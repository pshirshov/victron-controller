//! Forecast fusion — combines today's kWh estimate from up to three
//! providers into a single number for [`super::weather_soc`] to consume.
//!
//! Per SPEC §5.7, providers are fetched directly from the Rust service
//! (not from HA). Each provider publishes its own `today_kwh` to
//! `victron-controller/forecast/<provider>/today_kwh` so the user can
//! inspect them independently; fusion produces the single value used for
//! control.
//!
//! The strategy is runtime-configurable via
//! [`crate::knobs::ForecastDisagreementStrategy`]:
//!
//! - `Max` — optimistic; use the highest estimate.
//! - `Mean` — arithmetic mean across available providers.
//! - `Min` — conservative; use the lowest.
//! - `SolcastIfAvailableElseMean` — trust Solcast when fresh, else mean
//!   of the others.
//!
//! Fusion *only considers fresh providers*. A caller-supplied `is_fresh`
//! predicate decides staleness — the pure core doesn't know wall-clock
//! elapsed; the shell knows `fetched_at` and passes a closure.

use crate::knobs::ForecastDisagreementStrategy;
use crate::types::ForecastProvider;
use crate::world::{ForecastSnapshot, TypedSensors};

/// Fuse today's kWh estimates into a single number.
///
/// Returns `None` when no provider is fresh.
#[must_use]
pub fn fused_today_kwh(
    typed: &TypedSensors,
    strategy: ForecastDisagreementStrategy,
    mut is_fresh: impl FnMut(ForecastProvider, &ForecastSnapshot) -> bool,
) -> Option<f64> {
    let solcast = typed
        .forecast_solcast
        .filter(|s| is_fresh(ForecastProvider::Solcast, s));
    let fs = typed
        .forecast_forecast_solar
        .filter(|s| is_fresh(ForecastProvider::ForecastSolar, s));
    let om = typed
        .forecast_open_meteo
        .filter(|s| is_fresh(ForecastProvider::OpenMeteo, s));

    let mut fresh: Vec<f64> = [solcast, fs, om]
        .into_iter()
        .flatten()
        .map(|s| s.today_kwh)
        .collect();

    if fresh.is_empty() {
        return None;
    }

    match strategy {
        ForecastDisagreementStrategy::Max => fresh.iter().copied().reduce(f64::max),
        ForecastDisagreementStrategy::Min => fresh.iter().copied().reduce(f64::min),
        ForecastDisagreementStrategy::Mean => {
            #[allow(clippy::cast_precision_loss)]
            let n = fresh.len() as f64;
            Some(fresh.iter().sum::<f64>() / n)
        }
        ForecastDisagreementStrategy::SolcastIfAvailableElseMean => {
            if let Some(s) = solcast {
                Some(s.today_kwh)
            } else if fresh.is_empty() {
                None
            } else {
                // No Solcast → mean of others
                fresh.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                #[allow(clippy::cast_precision_loss)]
                let n = fresh.len() as f64;
                Some(fresh.iter().sum::<f64>() / n)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn snap(today_kwh: f64) -> ForecastSnapshot {
        ForecastSnapshot {
            today_kwh,
            tomorrow_kwh: 0.0,
            fetched_at: Instant::now(),
        }
    }

    fn typed_with(s: Option<f64>, fs: Option<f64>, om: Option<f64>) -> TypedSensors {
        TypedSensors {
            zappi_state: crate::Actual::unknown(Instant::now()),
            eddi_mode: crate::Actual::unknown(Instant::now()),
            forecast_solcast: s.map(snap),
            forecast_forecast_solar: fs.map(snap),
            forecast_open_meteo: om.map(snap),
        }
    }

    fn always_fresh(_: ForecastProvider, _: &ForecastSnapshot) -> bool {
        true
    }

    fn never_fresh(_: ForecastProvider, _: &ForecastSnapshot) -> bool {
        false
    }

    // ------------------------------------------------------------------
    // Empty / single-provider cases
    // ------------------------------------------------------------------

    #[test]
    fn no_providers_returns_none() {
        let t = typed_with(None, None, None);
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh),
            None
        );
    }

    #[test]
    fn all_providers_stale_returns_none() {
        let t = typed_with(Some(30.0), Some(40.0), Some(50.0));
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Mean, never_fresh),
            None
        );
    }

    #[test]
    fn single_provider_regardless_of_strategy() {
        let t = typed_with(None, Some(42.0), None);
        for s in [
            ForecastDisagreementStrategy::Max,
            ForecastDisagreementStrategy::Min,
            ForecastDisagreementStrategy::Mean,
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
        ] {
            assert_eq!(fused_today_kwh(&t, s, always_fresh), Some(42.0));
        }
    }

    // ------------------------------------------------------------------
    // Disagreement strategies
    // ------------------------------------------------------------------

    #[test]
    fn max_picks_highest() {
        let t = typed_with(Some(30.0), Some(40.0), Some(50.0));
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Max, always_fresh),
            Some(50.0)
        );
    }

    #[test]
    fn min_picks_lowest() {
        let t = typed_with(Some(30.0), Some(40.0), Some(50.0));
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Min, always_fresh),
            Some(30.0)
        );
    }

    #[test]
    fn mean_averages_all_fresh() {
        let t = typed_with(Some(30.0), Some(40.0), Some(50.0));
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh),
            Some(40.0)
        );
    }

    // ------------------------------------------------------------------
    // Solcast-preferred strategy
    // ------------------------------------------------------------------

    #[test]
    fn solcast_preferred_uses_solcast_when_fresh() {
        let t = typed_with(Some(25.0), Some(40.0), Some(50.0));
        assert_eq!(
            fused_today_kwh(
                &t,
                ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
                always_fresh
            ),
            Some(25.0)
        );
    }

    #[test]
    fn solcast_preferred_falls_back_to_mean_when_solcast_stale() {
        let t = typed_with(Some(25.0), Some(40.0), Some(50.0));
        // Solcast stale, others fresh.
        let f = |p: ForecastProvider, _: &ForecastSnapshot| p != ForecastProvider::Solcast;
        assert_eq!(
            fused_today_kwh(
                &t,
                ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
                f
            ),
            Some(45.0) // mean(40, 50)
        );
    }

    #[test]
    fn solcast_preferred_with_only_open_meteo_returns_open_meteo() {
        let t = typed_with(None, None, Some(33.0));
        assert_eq!(
            fused_today_kwh(
                &t,
                ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
                always_fresh
            ),
            Some(33.0)
        );
    }

    // ------------------------------------------------------------------
    // Freshness filter
    // ------------------------------------------------------------------

    #[test]
    fn mean_ignores_stale_providers() {
        let t = typed_with(Some(30.0), Some(40.0), Some(50.0));
        // Mark ForecastSolar stale.
        let f = |p: ForecastProvider, _: &ForecastSnapshot| p != ForecastProvider::ForecastSolar;
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Mean, f),
            Some(40.0) // mean(30, 50)
        );
    }
}
