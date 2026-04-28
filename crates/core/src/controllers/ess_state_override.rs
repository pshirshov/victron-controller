//! PR-keep-batteries-charged. Daytime ESS-state override controller.
//!
//! Decision: when the operator has enabled
//! `keep_batteries_charged_during_full_charge` AND today is a
//! charge-to-full day AND `now ∈ [sunrise + offset, sunset - offset]`,
//! the Victron `/Settings/CGwacs/BatteryLife/State` should be set to
//! `9` (KeepBatteriesCharged). When the override drops away (window
//! exit, knob flip, full-charge latch clear), the same path is restored
//! to `bookkeeping.prev_ess_state` — captured by the existing
//! current-limit `prev_ess_state` machinery (see
//! `current_limit.rs:165` for the `state != 9 && changed` capture
//! rule).
//!
//! Bias-to-safety: if `world.sunrise` / `world.sunset` are missing or
//! older than [`crate::world::SUNRISE_SUNSET_FRESHNESS`], the
//! controller returns `None` (no write). A stale clock must never pin
//! the system in `KeepBatteriesCharged`.
//!
//! Pure: emits a target value or `None`. The shell layer turns the
//! target into a `WriteDbus` effect when it differs from the live
//! `ess_state` sensor reading.

use std::time::{Duration, Instant};

use chrono::NaiveDateTime;

use crate::types::Decision;

/// Wire value for the `KeepBatteriesCharged` ESS state. Documented in
/// the Victron Hub-4 docs and matches the legacy NR flow's override
/// value (see `controllers::current_limit` line 162 comment).
pub const ESS_STATE_KEEP_BATTERIES_CHARGED: i32 = 9;

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
    /// Configured freshness window — values older than this revert to
    /// the bias-to-safety branch ("no fresh signal → don't override").
    pub freshness_threshold: Duration,
    /// Captured pre-override ESS state. `None` until the
    /// current-limit core has observed at least one non-9 value.
    pub prev_ess_state: Option<i32>,
    /// Live `ess_state` sensor reading (the actuated value the shell
    /// last read back from D-Bus). `None` when the sensor has never
    /// been seen as Fresh.
    pub current_ess_state: Option<i32>,
    /// Local-time clock reading. Sunrise/sunset are local-time, so the
    /// window comparison must be in local time too.
    pub now_local: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EssStateOverrideOutput {
    /// Target ESS state value to write. `None` means "don't write this
    /// tick". A `Some(target)` matching `current_ess_state` is also a
    /// no-op at the shell — the shell only emits a `WriteDbus` when
    /// the target differs from the live readback.
    pub target: Option<i32>,
    /// "Why?" explanation. Always populated.
    pub decision: Decision,
}

/// Evaluate whether the override should be active. Pure.
#[must_use]
pub fn evaluate_ess_state_override(input: &EssStateOverrideInput) -> EssStateOverrideOutput {
    let factors = |target: Option<i32>, branch: &str, in_window: Option<bool>| -> Decision {
        let mut d = Decision::new(branch.to_string())
            .with_factor("knob_enabled", format!("{}", input.knob_enabled))
            .with_factor(
                "charge_to_full_required",
                format!("{}", input.charge_to_full_required),
            )
            .with_factor("offset_min", format!("{}", input.offset_min))
            .with_factor(
                "current_ess_state",
                input
                    .current_ess_state
                    .map_or_else(|| "?".to_string(), |v| v.to_string()),
            )
            .with_factor(
                "prev_ess_state",
                input
                    .prev_ess_state
                    .map_or_else(|| "?".to_string(), |v| v.to_string()),
            )
            .with_factor(
                "target",
                target.map_or_else(|| "no-write".to_string(), |v| v.to_string()),
            );
        if let Some(b) = in_window {
            d = d.with_factor("in_window", format!("{b}"));
        }
        d
    };

    if !input.knob_enabled {
        return EssStateOverrideOutput {
            target: None,
            decision: factors(None, "override knob disabled → no write", None),
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
            target: None,
            decision: factors(
                None,
                "sunrise/sunset stale or unknown → bias-to-safety, no write",
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
    // Pathological config: offset so large that the window collapses or
    // inverts. Treat as "no window" — bias-to-safety.
    if window_end <= window_start {
        return EssStateOverrideOutput {
            target: None,
            decision: factors(
                None,
                "sunrise+offset >= sunset-offset → empty window, no write",
                None,
            )
            .with_factor("window_start_local", window_start.to_string())
            .with_factor("window_end_local", window_end.to_string()),
        };
    }
    let in_window = input.now_local >= window_start && input.now_local < window_end;
    let active = input.charge_to_full_required && in_window;

    if active {
        let target = ESS_STATE_KEEP_BATTERIES_CHARGED;
        return EssStateOverrideOutput {
            target: Some(target),
            decision: factors(
                Some(target),
                "full-charge day + within daylight window → force ESS state 9 (KeepBatteriesCharged)",
                Some(in_window),
            )
            .with_factor("window_start_local", window_start.to_string())
            .with_factor("window_end_local", window_end.to_string()),
        };
    }

    // Override is *not* desired. If we're currently sitting in state 9
    // (i.e. we previously wrote it), restore the pre-override value.
    // `prev_ess_state` is updated by the current-limit core only when
    // `ess_state != 9`; that's exactly the value we want here.
    if input.current_ess_state == Some(ESS_STATE_KEEP_BATTERIES_CHARGED) {
        match input.prev_ess_state {
            Some(prev) if prev != ESS_STATE_KEEP_BATTERIES_CHARGED => {
                return EssStateOverrideOutput {
                    target: Some(prev),
                    decision: factors(
                        Some(prev),
                        "override no longer desired but state==9 → restore prev_ess_state",
                        Some(in_window),
                    )
                    .with_factor("window_start_local", window_start.to_string())
                    .with_factor("window_end_local", window_end.to_string()),
                };
            }
            _ => {
                // No safe restore target. Leave state 9 in place rather
                // than writing a guess — operator can manually override
                // via the Victron UI.
                return EssStateOverrideOutput {
                    target: None,
                    decision: factors(
                        None,
                        "override no longer desired but no prev_ess_state recorded → leave alone",
                        Some(in_window),
                    ),
                };
            }
        }
    }

    EssStateOverrideOutput {
        target: None,
        decision: factors(
            None,
            "override inactive (no full-charge or outside window); ess_state already non-9",
            Some(in_window),
        )
        .with_factor("window_start_local", window_start.to_string())
        .with_factor("window_end_local", window_end.to_string()),
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
            prev_ess_state: Some(10),
            current_ess_state: Some(10),
            now_local: local(12, 0),
        }
    }

    #[test]
    fn knob_disabled_no_write() {
        let mut i = input();
        i.knob_enabled = false;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn full_charge_inside_window_writes_9() {
        let i = input();
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, Some(9));
    }

    #[test]
    fn full_charge_before_window_no_write_when_state_already_non_9() {
        // 05:30 < sunrise(05:00) + offset(60min) = 06:00.
        let mut i = input();
        i.now_local = local(5, 30);
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn full_charge_after_window_no_write_when_state_already_non_9() {
        // 20:30 > sunset(21:00) - offset(60min) = 20:00.
        let mut i = input();
        i.now_local = local(20, 30);
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn no_full_charge_no_write() {
        let mut i = input();
        i.charge_to_full_required = false;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn restore_prev_ess_state_when_currently_9() {
        // Override no longer desired (charge_to_full off) but live state
        // is 9 — should restore prev_ess_state.
        let mut i = input();
        i.charge_to_full_required = false;
        i.current_ess_state = Some(9);
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, Some(10));
    }

    #[test]
    fn restore_skipped_when_no_prev_recorded() {
        let mut i = input();
        i.charge_to_full_required = false;
        i.current_ess_state = Some(9);
        i.prev_ess_state = None;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn restore_skipped_when_prev_is_also_9() {
        let mut i = input();
        i.charge_to_full_required = false;
        i.current_ess_state = Some(9);
        i.prev_ess_state = Some(9);
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn missing_sunrise_no_write() {
        let mut i = input();
        i.sunrise_local = None;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn stale_sunrise_no_write() {
        let mut i = input();
        i.sunrise_sunset_updated_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(4 * 3600))
                .unwrap(),
        );
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }

    #[test]
    fn empty_window_via_huge_offset_no_write() {
        // 600 min offset on a 16h day: 5:00 + 10h = 15:00, 21:00 - 10h
        // = 11:00 → window_end <= window_start.
        let mut i = input();
        i.offset_min = 600;
        let out = evaluate_ess_state_override(&i);
        assert_eq!(out.target, None);
    }
}
