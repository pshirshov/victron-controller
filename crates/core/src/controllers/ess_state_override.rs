//! Daytime ESS-state controller. Two-target policy:
//!
//!   * `9` (KeepBatteriesCharged) when ALL of:
//!     - operator knob `keep_batteries_charged_during_full_charge` is on,
//!     - `bookkeeping.charge_to_full_required` is set (today is a
//!       full-charge day),
//!     - `now ∈ [sunrise + offset, sunset - offset]` (with fresh
//!       sunrise/sunset).
//!   * `10` (Optimized w/o BatteryLife) otherwise.
//!
//! There is no third "leave alone" branch: every tick proposes a
//! definite target, so the dashboard's actuated row never reads as
//! "undefined". Bias-to-safety: stale sunrise/sunset, an empty window
//! from a pathological offset, or any failure to evaluate the override
//! conditions falls through to target = 10.
//!
//! Pure: emits a target value. The shell layer turns the target into a
//! `WriteDbus` effect when it differs from the live `ess_state` sensor
//! reading.

use std::time::{Duration, Instant};

use chrono::NaiveDateTime;

use crate::types::Decision;

/// Wire value for the `KeepBatteriesCharged` ESS state. Documented in
/// the Victron Hub-4 docs and matches the legacy NR flow's override
/// value.
pub const ESS_STATE_KEEP_BATTERIES_CHARGED: i32 = 9;
/// Wire value for `OptimizedWithoutBatteryLife` (Victron's stock daily
/// self-consumption mode). The default target whenever the override
/// is not active.
pub const ESS_STATE_OPTIMIZED: i32 = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EssStateOverrideInput {
    /// Knob: master enable for the override.
    pub knob_enabled: bool,
    /// Knob: symmetric offset (minutes) inset from sunrise / sunset.
    pub offset_min: u32,
    /// `bookkeeping.charge_to_full_required`. Drives the override only
    /// on full-charge days.
    pub charge_to_full_required: bool,
    /// Live `world.sunrise` (local time). `None` while no fresh
    /// observation is available.
    pub sunrise_local: Option<NaiveDateTime>,
    /// Live `world.sunset` (local time). `None` while no fresh
    /// observation is available.
    pub sunset_local: Option<NaiveDateTime>,
    /// Monotonic stamp of the most recent successful sunrise/sunset
    /// observation.
    pub sunrise_sunset_updated_at: Option<Instant>,
    /// Configured freshness window — values older than this fall
    /// through to the default target (10) regardless of the in-window
    /// computation, since the window can't be evaluated on stale data.
    pub freshness_threshold: Duration,
    /// Local-time clock reading. Sunrise/sunset are local-time, so the
    /// window comparison must be in local time too.
    pub now_local: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EssStateOverrideOutput {
    /// Target ESS state value. Always either `ESS_STATE_OPTIMIZED`
    /// (10) or `ESS_STATE_KEEP_BATTERIES_CHARGED` (9). The shell
    /// short-circuits the `WriteDbus` when the target matches the
    /// live readback.
    pub target: i32,
    /// "Why?" explanation. Always populated.
    pub decision: Decision,
}

/// Evaluate whether the override should be active. Pure.
#[must_use]
pub fn evaluate_ess_state_override(input: &EssStateOverrideInput) -> EssStateOverrideOutput {
    let base_decision = |target: i32, branch: &str, in_window: Option<bool>| -> Decision {
        let mut d = Decision::new(branch.to_string())
            .with_factor("knob_enabled", format!("{}", input.knob_enabled))
            .with_factor(
                "charge_to_full_required",
                format!("{}", input.charge_to_full_required),
            )
            .with_factor("offset_min", format!("{}", input.offset_min))
            .with_factor("target", target.to_string());
        if let Some(b) = in_window {
            d = d.with_factor("in_window", format!("{b}"));
        }
        d
    };

    if !input.knob_enabled {
        return EssStateOverrideOutput {
            target: ESS_STATE_OPTIMIZED,
            decision: base_decision(
                ESS_STATE_OPTIMIZED,
                "override knob disabled → ESS state 10 (Optimized)",
                None,
            ),
        };
    }

    if !input.charge_to_full_required {
        return EssStateOverrideOutput {
            target: ESS_STATE_OPTIMIZED,
            decision: base_decision(
                ESS_STATE_OPTIMIZED,
                "not a full-charge day → ESS state 10 (Optimized)",
                None,
            ),
        };
    }

    let fresh = match (input.sunrise_local, input.sunset_local, input.sunrise_sunset_updated_at) {
        (Some(_), Some(_), Some(at)) => match Instant::now().checked_duration_since(at) {
            Some(age) => age <= input.freshness_threshold,
            None => true,
        },
        _ => false,
    };

    if !fresh {
        return EssStateOverrideOutput {
            target: ESS_STATE_OPTIMIZED,
            decision: base_decision(
                ESS_STATE_OPTIMIZED,
                "sunrise/sunset stale or unknown → bias-to-safety, ESS state 10",
                None,
            )
            .with_factor("fresh", "false".to_string()),
        };
    }

    // Both Some by the freshness gate above.
    let sunrise = input.sunrise_local.expect("fresh implies Some");
    let sunset = input.sunset_local.expect("fresh implies Some");
    let offset = chrono::Duration::minutes(i64::from(input.offset_min));
    let window_start = sunrise + offset;
    let window_end = sunset - offset;
    if window_end <= window_start {
        return EssStateOverrideOutput {
            target: ESS_STATE_OPTIMIZED,
            decision: base_decision(
                ESS_STATE_OPTIMIZED,
                "sunrise+offset >= sunset-offset → empty window, ESS state 10",
                None,
            )
            .with_factor("window_start_local", window_start.to_string())
            .with_factor("window_end_local", window_end.to_string()),
        };
    }
    let in_window = input.now_local >= window_start && input.now_local < window_end;

    if in_window {
        EssStateOverrideOutput {
            target: ESS_STATE_KEEP_BATTERIES_CHARGED,
            decision: base_decision(
                ESS_STATE_KEEP_BATTERIES_CHARGED,
                "full-charge day + within daylight window → ESS state 9 (KeepBatteriesCharged)",
                Some(true),
            )
            .with_factor("window_start_local", window_start.to_string())
            .with_factor("window_end_local", window_end.to_string()),
        }
    } else {
        EssStateOverrideOutput {
            target: ESS_STATE_OPTIMIZED,
            decision: base_decision(
                ESS_STATE_OPTIMIZED,
                "full-charge day but outside daylight window → ESS state 10 (Optimized)",
                Some(false),
            )
            .with_factor("window_start_local", window_start.to_string())
            .with_factor("window_end_local", window_end.to_string()),
        }
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn local(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn input() -> EssStateOverrideInput {
        EssStateOverrideInput {
            knob_enabled: true,
            offset_min: 60,
            charge_to_full_required: true,
            sunrise_local: Some(local(5, 0)),
            sunset_local: Some(local(21, 0)),
            sunrise_sunset_updated_at: Some(Instant::now()),
            freshness_threshold: Duration::from_secs(3 * 3600),
            now_local: local(12, 0),
        }
    }

    #[test]
    fn knob_disabled_targets_10() {
        let mut i = input();
        i.knob_enabled = false;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_OPTIMIZED);
    }

    #[test]
    fn full_charge_inside_window_targets_9() {
        let i = input();
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_KEEP_BATTERIES_CHARGED);
    }

    #[test]
    fn full_charge_before_window_targets_10() {
        // 05:30 < sunrise(05:00) + offset(60min) = 06:00.
        let mut i = input();
        i.now_local = local(5, 30);
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_OPTIMIZED);
    }

    #[test]
    fn full_charge_after_window_targets_10() {
        // 20:30 > sunset(21:00) - offset(60min) = 20:00.
        let mut i = input();
        i.now_local = local(20, 30);
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_OPTIMIZED);
    }

    #[test]
    fn no_full_charge_targets_10() {
        let mut i = input();
        i.charge_to_full_required = false;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_OPTIMIZED);
    }

    #[test]
    fn missing_sunrise_targets_10() {
        let mut i = input();
        i.sunrise_local = None;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_OPTIMIZED);
    }

    #[test]
    fn stale_sunrise_targets_10() {
        let mut i = input();
        i.sunrise_sunset_updated_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(4 * 3600))
                .unwrap(),
        );
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_OPTIMIZED);
    }

    #[test]
    fn empty_window_via_huge_offset_targets_10() {
        // 600 min offset on a 16h day: 5:00 + 10h = 15:00, 21:00 - 10h
        // = 11:00 → window_end <= window_start.
        let mut i = input();
        i.offset_min = 600;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, ESS_STATE_OPTIMIZED);
    }
}
