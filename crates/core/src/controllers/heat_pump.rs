//! Pure heat-pump controller — `evaluate_heat_pump`.
//!
//! Produces proposals for three of the four LG ThinQ actuated entities:
//! - `lg_dhw_power` (time-window schedule)
//! - `lg_dhw_target_c` (constant 60 °C)
//! - `lg_heating_water_target_c` (outdoor-temperature curve, gated on
//!   freshness of `world.sensors.outdoor_temperature`)
//!
//! The fourth entity (`lg_heat_pump_power`) is NOT proposed here; it is
//! operator-only and mirrored by `apply_knob` in `process.rs`.

use chrono::{NaiveDateTime, NaiveTime};

use crate::tass::Actual;
use crate::types::Decision;

/// Output of `evaluate_heat_pump`.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatPumpOutput {
    /// Whether DHW (hot-water) mode should be on. Always `Some`.
    pub dhw_power: Option<bool>,
    /// DHW temperature target (°C). Always `Some(60)`.
    pub dhw_target_c: Option<i32>,
    /// Heating-water temperature target (°C). `None` when outdoor
    /// temperature is not Fresh (controller skips proposal).
    pub heating_water_target_c: Option<i32>,
    /// Human-readable decision factors for the dashboard.
    pub decision: Decision,
}

/// DHW heating windows: [02:00, 05:00) ∪ [07:00, 08:00).
fn in_dhw_window(t: NaiveTime) -> bool {
    let w1_start = NaiveTime::from_hms_opt(2, 0, 0).expect("valid");
    let w1_end = NaiveTime::from_hms_opt(5, 0, 0).expect("valid");
    let w2_start = NaiveTime::from_hms_opt(7, 0, 0).expect("valid");
    let w2_end = NaiveTime::from_hms_opt(8, 0, 0).expect("valid");
    (t >= w1_start && t < w1_end) || (t >= w2_start && t < w2_end)
}

/// Heating-water target temperature from outdoor temperature bucket (°C).
/// Strict `t ≤ threshold` buckets per plan §3 D07.
fn heating_target_from_outdoor_c(outdoor_c: f64) -> i32 {
    if outdoor_c <= 2.0 {
        48
    } else if outdoor_c <= 5.0 {
        46
    } else if outdoor_c <= 8.0 {
        44
    } else if outdoor_c <= 10.0 {
        43
    } else {
        42
    }
}

/// Evaluate heat-pump proposals for this tick.
///
/// `now_local` is the current local `NaiveDateTime` from
/// `topology.tz_handle` (same as `clock.naive()`).
/// `outdoor_temp` is `world.sensors.outdoor_temperature`.
/// `master_on` is `world.lg_heat_pump_power.actual` — readback of the
/// heat-pump master power, driven externally by the heat-demand relay.
///
/// The heating-water target is gated on `master_on` being Fresh + true.
/// LG silently drops setpoint writes while the unit reports
/// `boilerOperationMode == POWER_OFF` (the cloud returns 2xx but the
/// unit's reported `targetTemperature` never changes), which would
/// otherwise leave the actuator stuck in Commanded+Deprecated and burn
/// LG API retries every tick. DHW power and DHW target are still
/// proposed unconditionally (DHW writes succeed independently of master
/// power in practice).
pub fn evaluate_heat_pump(
    now_local: NaiveDateTime,
    outdoor_temp: Actual<f64>,
    master_on: Actual<bool>,
) -> HeatPumpOutput {
    let t = now_local.time();
    let dhw_power = in_dhw_window(t);
    let dhw_target_c = 60;

    let master_demanding =
        master_on.freshness == crate::Freshness::Fresh && master_on.value == Some(true);

    let mut decision = Decision::new("heat-pump controller")
        .with_factor("time_local", now_local.time().format("%H:%M:%S").to_string())
        .with_factor("dhw_window", dhw_power.to_string())
        .with_factor("outdoor_freshness", format!("{:?}", outdoor_temp.freshness))
        .with_factor("master_demanding", master_demanding.to_string());

    let heating_water_target_c = if !master_demanding {
        decision = decision.with_factor(
            "heating_target_c",
            "skipped (master not demanding heat)".to_string(),
        );
        None
    } else if outdoor_temp.freshness == crate::Freshness::Fresh {
        let t_c = outdoor_temp.value.unwrap_or(f64::NAN);
        let target = heating_target_from_outdoor_c(t_c);
        decision = decision
            .with_factor("outdoor_temp_c", format!("{t_c:.1}"))
            .with_factor("heating_target_c", target.to_string());
        Some(target)
    } else {
        decision = decision.with_factor(
            "heating_target_c",
            "skipped (outdoor temperature not fresh)".to_string(),
        );
        None
    };

    HeatPumpOutput {
        dhw_power: Some(dhw_power),
        dhw_target_c: Some(dhw_target_c),
        heating_water_target_c,
        decision,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::time::Instant;

    use crate::tass::Actual;
    use crate::Freshness;

    fn local(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 5, 12)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn fresh_outdoor(c: f64) -> Actual<f64> {
        Actual {
            value: Some(c),
            freshness: Freshness::Fresh,
            since: Instant::now(),
        }
    }

    fn stale_outdoor() -> Actual<f64> {
        Actual {
            value: Some(10.0),
            freshness: Freshness::Stale,
            since: Instant::now(),
        }
    }

    fn unknown_outdoor() -> Actual<f64> {
        Actual::unknown(Instant::now())
    }

    fn master_on(on: bool) -> Actual<bool> {
        Actual {
            value: Some(on),
            freshness: Freshness::Fresh,
            since: Instant::now(),
        }
    }

    fn master_stale(on: bool) -> Actual<bool> {
        Actual {
            value: Some(on),
            freshness: Freshness::Stale,
            since: Instant::now(),
        }
    }

    fn master_unknown() -> Actual<bool> {
        Actual::unknown(Instant::now())
    }

    // --- outdoor-temp bucket tests ---

    #[test]
    fn bucket_t1_yields_48() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(1.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(48));
        assert_eq!(out.dhw_target_c, Some(60));
    }

    #[test]
    fn bucket_t4_yields_46() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(4.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(46));
    }

    #[test]
    fn bucket_t6_yields_44() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(6.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(44));
    }

    #[test]
    fn bucket_t9_yields_43() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(9.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(43));
    }

    #[test]
    fn bucket_t15_yields_42() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(42));
    }

    // --- boundary tests (t <= threshold is inclusive) ---

    #[test]
    fn boundary_t2_inclusive_yields_48() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(2.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(48));
    }

    #[test]
    fn boundary_t5_inclusive_yields_46() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(5.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(46));
    }

    #[test]
    fn boundary_t8_inclusive_yields_44() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(8.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(44));
    }

    #[test]
    fn boundary_t10_inclusive_yields_43() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(10.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(43));
    }

    // --- DHW window tests ---

    #[test]
    fn dhw_window_0230_is_on() {
        let out = evaluate_heat_pump(local(2, 30), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.dhw_power, Some(true));
    }

    #[test]
    fn dhw_window_0730_is_on() {
        let out = evaluate_heat_pump(local(7, 30), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.dhw_power, Some(true));
    }

    #[test]
    fn dhw_window_1200_is_off() {
        let out = evaluate_heat_pump(local(12, 0), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.dhw_power, Some(false));
    }

    // --- window edge tests (inclusive at start, exclusive at end) ---

    #[test]
    fn dhw_window_edge_0200_is_in() {
        let out = evaluate_heat_pump(local(2, 0), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.dhw_power, Some(true));
    }

    #[test]
    fn dhw_window_edge_0500_is_out() {
        let out = evaluate_heat_pump(local(5, 0), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.dhw_power, Some(false));
    }

    #[test]
    fn dhw_window_edge_0700_is_in() {
        let out = evaluate_heat_pump(local(7, 0), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.dhw_power, Some(true));
    }

    #[test]
    fn dhw_window_edge_0800_is_out() {
        let out = evaluate_heat_pump(local(8, 0), fresh_outdoor(15.0), master_on(true));
        assert_eq!(out.dhw_power, Some(false));
    }

    // --- freshness gate tests ---

    #[test]
    fn stale_outdoor_temperature_skips_heating_target() {
        let out = evaluate_heat_pump(local(3, 0), stale_outdoor(), master_on(true));
        assert_eq!(out.heating_water_target_c, None, "stale → no proposal");
        // DHW is unaffected by outdoor-temp freshness.
        assert_eq!(out.dhw_power, Some(true));
        assert_eq!(out.dhw_target_c, Some(60));
    }

    #[test]
    fn unknown_outdoor_temperature_skips_heating_target() {
        let out = evaluate_heat_pump(local(3, 0), unknown_outdoor(), master_on(true));
        assert_eq!(out.heating_water_target_c, None, "unknown → no proposal");
    }

    // --- DHW constant invariant ---

    #[test]
    fn dhw_target_constant_60_across_all_buckets() {
        for &t in &[1.0_f64, 4.0, 6.0, 9.0, 15.0] {
            let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(t), master_on(true));
            assert_eq!(out.dhw_target_c, Some(60), "t={t} → dhw_target_c must always be 60");
        }
    }

    // --- master-demand gate tests -------------------------------------

    #[test]
    fn master_off_skips_heating_target_even_with_fresh_outdoor() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(1.0), master_on(false));
        assert_eq!(
            out.heating_water_target_c, None,
            "master off → no heating-water proposal regardless of outdoor temp"
        );
        // DHW still proposed.
        assert_eq!(out.dhw_power, Some(true));
        assert_eq!(out.dhw_target_c, Some(60));
    }

    #[test]
    fn master_unknown_skips_heating_target() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(1.0), master_unknown());
        assert_eq!(out.heating_water_target_c, None);
    }

    #[test]
    fn master_stale_true_still_skips_heating_target() {
        // Stale readback isn't authoritative enough to drive a write —
        // require Fresh so we only act when the readback is current.
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(1.0), master_stale(true));
        assert_eq!(out.heating_water_target_c, None);
    }

    #[test]
    fn master_on_fresh_with_fresh_outdoor_proposes_target() {
        let out = evaluate_heat_pump(local(3, 0), fresh_outdoor(1.0), master_on(true));
        assert_eq!(out.heating_water_target_c, Some(48));
    }
}
