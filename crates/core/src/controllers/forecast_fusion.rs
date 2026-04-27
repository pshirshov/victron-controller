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
///
/// PR-baseline-forecast: the locally-computed `Baseline` provider is a
/// last-resort fallback. It participates ONLY when zero of the three
/// cloud providers (Solcast / Forecast.Solar / Open-Meteo) supplied a
/// fresh snapshot. With at least one cloud snapshot fresh, baseline is
/// ignored entirely — even under `Mean` (which would otherwise drag the
/// fused estimate toward the very pessimistic baseline number) and even
/// under `Min` (which would otherwise nearly always pick baseline).
#[must_use]
pub fn fused_today_kwh(
    typed: &TypedSensors,
    strategy: ForecastDisagreementStrategy,
    mut is_fresh: impl FnMut(ForecastProvider, &ForecastSnapshot) -> bool,
) -> Option<f64> {
    let solcast = typed
        .forecast_solcast
        .as_ref()
        .filter(|s| is_fresh(ForecastProvider::Solcast, s));
    let fs = typed
        .forecast_forecast_solar
        .as_ref()
        .filter(|s| is_fresh(ForecastProvider::ForecastSolar, s));
    let om = typed
        .forecast_open_meteo
        .as_ref()
        .filter(|s| is_fresh(ForecastProvider::OpenMeteo, s));

    // A-41: filter non-finite values before reducing. A provider that
    // leaks NaN (Open-Meteo null → 0/0, schema drift, ring-arithmetic
    // bug) would otherwise contaminate Max/Min/Mean — f64::max(NaN, x)
    // = x partly hides it, but reduce(f64::max) isn't total on NaN and
    // the result is subtly non-deterministic.
    let mut fresh: Vec<f64> = [solcast, fs, om]
        .into_iter()
        .flatten()
        .map(|s| s.today_kwh)
        .filter(|v| v.is_finite())
        .collect();

    if fresh.is_empty() {
        // Last-resort fallback: consult the locally-computed baseline.
        return typed
            .forecast_baseline
            .as_ref()
            .filter(|s| is_fresh(ForecastProvider::Baseline, s))
            .map(|s| s.today_kwh)
            .filter(|v| v.is_finite());
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

/// PR-soc-chart-solar: fuse per-hour energy estimates across providers
/// into a single length-48 vector, hour-by-hour. Returns an empty `Vec`
/// when no fresh provider supplied any hourly data — callers must
/// distinguish that case from "all providers say 0 kWh this hour".
///
/// For each hour 0..48 we collect the contribution of every fresh
/// provider that has *non-empty* hourly data, then fold using the
/// configured strategy. Providers whose `hourly_kwh` is empty (legacy
/// fetch path / quota burnout) are silently omitted, mirroring the
/// missing-snapshot behaviour of [`fused_today_kwh`].
#[must_use]
pub fn fused_hourly_kwh(
    typed: &TypedSensors,
    strategy: ForecastDisagreementStrategy,
    mut is_fresh: impl FnMut(ForecastProvider, &ForecastSnapshot) -> bool,
) -> Vec<f64> {
    let solcast = typed
        .forecast_solcast
        .as_ref()
        .filter(|s| is_fresh(ForecastProvider::Solcast, s));
    let fs = typed
        .forecast_forecast_solar
        .as_ref()
        .filter(|s| is_fresh(ForecastProvider::ForecastSolar, s));
    let om = typed
        .forecast_open_meteo
        .as_ref()
        .filter(|s| is_fresh(ForecastProvider::OpenMeteo, s));

    // Only providers with non-empty hourly data participate.
    let solcast_h: Option<&[f64]> = solcast
        .map(|s| s.hourly_kwh.as_slice())
        .filter(|v| !v.is_empty());
    let fs_h: Option<&[f64]> = fs
        .map(|s| s.hourly_kwh.as_slice())
        .filter(|v| !v.is_empty());
    let om_h: Option<&[f64]> = om
        .map(|s| s.hourly_kwh.as_slice())
        .filter(|v| !v.is_empty());

    if solcast_h.is_none() && fs_h.is_none() && om_h.is_none() {
        // PR-baseline-forecast: last-resort fallback. Identical fallback
        // gate as `fused_today_kwh`: only consulted when no cloud
        // provider supplied fresh hourly data.
        return typed
            .forecast_baseline
            .as_ref()
            .filter(|s| is_fresh(ForecastProvider::Baseline, s))
            .map(|s| s.hourly_kwh.clone())
            .unwrap_or_default();
    }

    // Length is the max of all participating providers, capped at 48.
    // A short provider (e.g. Forecast.Solar returning only 24 entries
    // padded with 0) still contributes 0 for hours past its end.
    let max_len = [solcast_h, fs_h, om_h]
        .iter()
        .filter_map(|x| x.map(<[f64]>::len))
        .max()
        .unwrap_or(0)
        .min(48);

    let mut out = Vec::with_capacity(max_len);
    for h in 0..max_len {
        let mut samples: Vec<f64> = Vec::with_capacity(3);
        for src in [solcast_h, fs_h, om_h].iter().flatten() {
            if let Some(v) = src.get(h).copied() {
                if v.is_finite() {
                    samples.push(v);
                }
            }
        }
        if samples.is_empty() {
            // No participating provider supplied a finite value at this
            // hour — emit 0 (as opposed to None) since the chart caller
            // treats every entry as a numeric kWh estimate. Empty Vec
            // (the "all providers had no hourly data" signal) is handled
            // above by the early return.
            out.push(0.0);
            continue;
        }
        let fused = match strategy {
            ForecastDisagreementStrategy::Max => {
                samples.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            }
            ForecastDisagreementStrategy::Min => {
                samples.iter().copied().fold(f64::INFINITY, f64::min)
            }
            ForecastDisagreementStrategy::Mean => {
                #[allow(clippy::cast_precision_loss)]
                let n = samples.len() as f64;
                samples.iter().sum::<f64>() / n
            }
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean => {
                if let Some(s) = solcast_h.and_then(|s| s.get(h).copied()) {
                    if s.is_finite() {
                        s
                    } else {
                        #[allow(clippy::cast_precision_loss)]
                        let n = samples.len() as f64;
                        samples.iter().sum::<f64>() / n
                    }
                } else {
                    #[allow(clippy::cast_precision_loss)]
                    let n = samples.len() as f64;
                    samples.iter().sum::<f64>() / n
                }
            }
        };
        out.push(fused);
    }
    out
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
            hourly_kwh: Vec::new(),
        }
    }

    fn typed_with(s: Option<f64>, fs: Option<f64>, om: Option<f64>) -> TypedSensors {
        TypedSensors {
            zappi_state: crate::Actual::unknown(Instant::now()),
            eddi_mode: crate::Actual::unknown(Instant::now()),
            forecast_solcast: s.map(snap),
            forecast_forecast_solar: fs.map(snap),
            forecast_open_meteo: om.map(snap),
            forecast_baseline: None,
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

    // ------------------------------------------------------------------
    // PR-soc-chart-solar: hourly fusion
    // ------------------------------------------------------------------

    fn snap_hourly(today_kwh: f64, hourly: Vec<f64>) -> ForecastSnapshot {
        ForecastSnapshot {
            today_kwh,
            tomorrow_kwh: 0.0,
            fetched_at: Instant::now(),
            hourly_kwh: hourly,
        }
    }

    fn typed_hourly(
        s: Option<Vec<f64>>,
        fs: Option<Vec<f64>>,
        om: Option<Vec<f64>>,
    ) -> TypedSensors {
        TypedSensors {
            zappi_state: crate::Actual::unknown(Instant::now()),
            eddi_mode: crate::Actual::unknown(Instant::now()),
            forecast_solcast: s.map(|h| snap_hourly(0.0, h)),
            forecast_forecast_solar: fs.map(|h| snap_hourly(0.0, h)),
            forecast_open_meteo: om.map(|h| snap_hourly(0.0, h)),
            forecast_baseline: None,
        }
    }

    #[test]
    fn fused_hourly_mean_across_providers() {
        let s = vec![1.0, 2.0, 3.0, 4.0];
        let fs = vec![3.0, 4.0, 5.0, 6.0];
        let om = vec![5.0, 6.0, 7.0, 8.0];
        let t = typed_hourly(Some(s), Some(fs), Some(om));
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh);
        assert_eq!(out, vec![3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn fused_hourly_skips_stale() {
        let s = vec![10.0, 10.0];
        let fs = vec![20.0, 20.0];
        let om = vec![30.0, 30.0];
        let t = typed_hourly(Some(s), Some(fs), Some(om));
        // ForecastSolar stale.
        let f = |p: ForecastProvider, _: &ForecastSnapshot| p != ForecastProvider::ForecastSolar;
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, f);
        assert_eq!(out, vec![20.0, 20.0]); // mean(10, 30)
    }

    #[test]
    fn fused_hourly_empty_when_all_empty() {
        // All providers present but with empty hourly arrays → empty Vec
        // (signal: "no provider supplied hourly data at all").
        let t = typed_hourly(Some(vec![]), Some(vec![]), Some(vec![]));
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh);
        assert!(out.is_empty(), "expected empty Vec, got {out:?}");
    }

    #[test]
    fn fused_hourly_empty_when_no_providers() {
        let t = typed_hourly(None, None, None);
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh);
        assert!(out.is_empty());
    }

    #[test]
    fn fused_hourly_handles_uneven_lengths() {
        // Forecast.Solar returns only 24 entries; others return 4. Output
        // length is the max of participating providers (24); short
        // providers contribute 0 past their end.
        let s = vec![10.0, 10.0, 10.0, 10.0];
        let fs: Vec<f64> = (0..24).map(|_| 5.0).collect();
        let t = typed_hourly(Some(s), Some(fs), None);
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh);
        assert_eq!(out.len(), 24);
        // Hours 0..4: mean(10, 5) = 7.5
        for (h, &v) in out.iter().enumerate().take(4) {
            assert!((v - 7.5).abs() < 1e-9, "hour {h}: {v}");
        }
        // Hour 4..24: only fs contributes (5)
        for (h, &v) in out.iter().enumerate().take(24).skip(4) {
            assert!((v - 5.0).abs() < 1e-9, "hour {h}: {v}");
        }
    }

    // ------------------------------------------------------------------
    // PR-baseline-forecast: Baseline acts as a last-resort fallback.
    // ------------------------------------------------------------------

    fn typed_with_baseline(
        s: Option<f64>,
        fs: Option<f64>,
        om: Option<f64>,
        baseline: Option<f64>,
    ) -> TypedSensors {
        TypedSensors {
            zappi_state: crate::Actual::unknown(Instant::now()),
            eddi_mode: crate::Actual::unknown(Instant::now()),
            forecast_solcast: s.map(snap),
            forecast_forecast_solar: fs.map(snap),
            forecast_open_meteo: om.map(snap),
            forecast_baseline: baseline.map(snap),
        }
    }

    fn typed_hourly_with_baseline(
        s: Option<Vec<f64>>,
        fs: Option<Vec<f64>>,
        om: Option<Vec<f64>>,
        baseline: Option<Vec<f64>>,
    ) -> TypedSensors {
        TypedSensors {
            zappi_state: crate::Actual::unknown(Instant::now()),
            eddi_mode: crate::Actual::unknown(Instant::now()),
            forecast_solcast: s.map(|h| snap_hourly(0.0, h)),
            forecast_forecast_solar: fs.map(|h| snap_hourly(0.0, h)),
            forecast_open_meteo: om.map(|h| snap_hourly(0.0, h)),
            forecast_baseline: baseline.map(|h| snap_hourly(0.0, h)),
        }
    }

    #[test]
    fn baseline_ignored_when_any_cloud_provider_fresh() {
        // Even under Min (which would otherwise pick the very pessimistic
        // baseline) and Mean (which would drag the average down), baseline
        // must be ignored when at least one cloud provider is fresh.
        let t = typed_with_baseline(Some(40.0), None, None, Some(2.0));
        for s in [
            ForecastDisagreementStrategy::Max,
            ForecastDisagreementStrategy::Min,
            ForecastDisagreementStrategy::Mean,
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
        ] {
            assert_eq!(
                fused_today_kwh(&t, s, always_fresh),
                Some(40.0),
                "strategy {s:?}",
            );
        }
    }

    #[test]
    fn baseline_used_when_all_clouds_stale() {
        let t = typed_with_baseline(Some(40.0), Some(50.0), Some(60.0), Some(2.0));
        // Mark all three clouds stale, baseline fresh.
        let f = |p: ForecastProvider, _: &ForecastSnapshot| p == ForecastProvider::Baseline;
        for s in [
            ForecastDisagreementStrategy::Max,
            ForecastDisagreementStrategy::Min,
            ForecastDisagreementStrategy::Mean,
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
        ] {
            assert_eq!(fused_today_kwh(&t, s, f), Some(2.0), "strategy {s:?}");
        }
    }

    #[test]
    fn baseline_used_when_no_cloud_configured() {
        let t = typed_with_baseline(None, None, None, Some(3.0));
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh),
            Some(3.0),
        );
    }

    #[test]
    fn baseline_stale_returns_none() {
        let t = typed_with_baseline(None, None, None, Some(3.0));
        // Baseline present but stale.
        assert_eq!(
            fused_today_kwh(&t, ForecastDisagreementStrategy::Mean, never_fresh),
            None,
        );
    }

    #[test]
    fn fused_hourly_baseline_ignored_when_any_cloud_fresh() {
        let t = typed_hourly_with_baseline(
            Some(vec![10.0, 10.0]),
            None,
            None,
            Some(vec![1.0, 1.0]),
        );
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh);
        assert_eq!(out, vec![10.0, 10.0]);
    }

    #[test]
    fn fused_hourly_baseline_used_when_clouds_empty() {
        // All clouds present but with empty hourly arrays — fall back to
        // baseline.
        let t = typed_hourly_with_baseline(
            Some(vec![]),
            Some(vec![]),
            Some(vec![]),
            Some(vec![1.0, 2.0, 3.0]),
        );
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, always_fresh);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn fused_hourly_baseline_empty_when_baseline_stale() {
        let t = typed_hourly_with_baseline(None, None, None, Some(vec![1.0, 2.0]));
        let out = fused_hourly_kwh(&t, ForecastDisagreementStrategy::Mean, never_fresh);
        assert!(out.is_empty());
    }
}
