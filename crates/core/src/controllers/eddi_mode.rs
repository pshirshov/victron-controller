//! Eddi mode controller — new logic, not a port (the existing HA
//! automation is being replaced). See SPEC §5.10.
//!
//! **Safety direction**: default target is `Stopped`. `Normal` is only
//! issued when battery SoC is *Fresh* AND ≥ `eddi_enable_soc`. Once
//! Normal, stays Normal until SoC ≤ `eddi_disable_soc` (or SoC becomes
//! stale / unknown).
//!
//! The hysteresis band is between `eddi_disable_soc` and `eddi_enable_soc`
//! (default 94–96 %). Above the band → Normal; below → Stopped; inside
//! the band → hold the current mode.
//!
//! A dwell timer (`eddi_dwell_s`, default 60) gates re-evaluation after
//! the last mode change to prevent flapping under noisy SoC readings.

use std::time::{Duration, Instant};

use crate::Clock;
use crate::myenergi::EddiMode;
use crate::tass::Freshness;
use crate::types::Decision;

/// Inputs — the SoC sensor half (value + freshness) and the three knobs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EddiModeInput {
    /// Most recent battery SoC reading (%). Not consulted when `soc_freshness` is `Unknown`.
    pub soc_value: Option<f64>,
    pub soc_freshness: Freshness,
    /// Current Eddi mode as last observed from myenergi.
    pub current_mode: EddiMode,
    /// When the Eddi mode target last changed, or `None` if never yet.
    pub last_transition_at: Option<Instant>,
    pub knobs: EddiModeKnobs,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EddiModeKnobs {
    pub enable_soc: f64,
    pub disable_soc: f64,
    pub dwell_s: u32,
}

/// Decision: explicit target with a flag for whether it requires an
/// actuation call. `Set` and `Leave` both carry the controller's
/// intended target (Stopped or Normal); `Leave` differs only in that
/// the actual mode already matches, so no `CallMyenergi` effect is
/// needed. The TASS target is still proposed in both cases so the
/// dashboard / HA see the controller's intent — pre-fix the `Leave`
/// path left `world.eddi_mode.target` stuck at `Unset` forever
/// when the device's first-tick assumed-Stopped happened to match
/// the controller's Stopped decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EddiModeAction {
    Set(EddiMode),
    Leave(EddiMode),
}

impl EddiModeAction {
    /// The controller's intended target mode, regardless of whether
    /// actuation is required this tick.
    #[must_use]
    pub fn target(self) -> EddiMode {
        match self {
            Self::Set(m) | Self::Leave(m) => m,
        }
    }

    /// True iff the controller wants `process()` to fire a
    /// `CallMyenergi(SetEddiMode)` effect this tick.
    #[must_use]
    pub fn should_actuate(self) -> bool {
        matches!(self, Self::Set(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EddiModeOutput {
    pub action: EddiModeAction,
    pub decision: Decision,
}

/// Tariff-window override (PR-eddi-tariff-windows). The four windows
/// match the legacy NR cron triggers — the Eddi follows the cheap-rate
/// tariff schedule rather than waiting for SoC to climb above the
/// hysteresis band:
///
/// - 02:00–05:00 → Normal  (cheap-rate Boost window)
/// - 05:00–07:00 → Stopped (rest)
/// - 07:00–08:00 → Normal  (last cheap-rate hour before day rate)
/// - 08:00–02:00 → no override; SoC-based hysteresis runs
///
/// Returns `None` when no schedule applies, so the existing SoC-based
/// hysteresis below takes over. Hardcoded for now — if other tariff
/// shapes appear later, lift to knob-driven schedule.
#[must_use]
pub fn eddi_schedule_override(now: chrono::NaiveDateTime) -> Option<EddiMode> {
    use chrono::Timelike;
    let h = now.hour();
    if (2..5).contains(&h) {
        Some(EddiMode::Normal)
    } else if (5..7).contains(&h) {
        Some(EddiMode::Stopped)
    } else if h == 7 {
        Some(EddiMode::Normal)
    } else {
        None
    }
}

/// Evaluate the desired Eddi mode target.
#[must_use]
pub fn evaluate_eddi_mode(input: &EddiModeInput, clock: &dyn Clock) -> EddiModeOutput {
    let factors = || Decision::new("placeholder")
        .with_factor("soc_freshness", format!("{:?}", input.soc_freshness))
        .with_factor("soc", match input.soc_value {
            Some(v) => format!("{v:.1}%"),
            None => "—".to_string(),
        })
        .with_factor("current_mode", format!("{:?}", input.current_mode))
        .with_factor("enable_soc", format!("{:.0}%", input.knobs.enable_soc))
        .with_factor("disable_soc", format!("{:.0}%", input.knobs.disable_soc))
        .with_factor("dwell_s", format!("{}", input.knobs.dwell_s))
        .factors;

    // Safety: SoC unknown or stale → target Stopped.
    if input.soc_freshness != Freshness::Fresh {
        let action = safe_action(EddiMode::Stopped, input);
        return EddiModeOutput {
            action,
            decision: Decision {
                summary: "SoC not Fresh — safety direction → Stopped".to_string(),
                factors: factors(),
            },
        };
    }
    let Some(soc) = input.soc_value else {
        let action = safe_action(EddiMode::Stopped, input);
        return EddiModeOutput {
            action,
            decision: Decision {
                summary: "SoC Fresh but value missing — safety direction → Stopped".to_string(),
                factors: factors(),
            },
        };
    };

    // PR-eddi-tariff-windows: clock-of-day override. When the operator
    // is on a multi-window cheap-rate tariff (legacy NR pattern), drive
    // Eddi by the schedule regardless of SoC. Outside the override
    // windows, the SoC-based hysteresis below runs as before.
    let now_naive = clock.naive();
    let (desired, band): (EddiMode, &'static str) =
        if let Some(forced) = eddi_schedule_override(now_naive) {
            match forced {
                EddiMode::Normal => (EddiMode::Normal, "tariff window → Normal"),
                EddiMode::Stopped => (EddiMode::Stopped, "tariff window → Stopped"),
            }
        } else if soc >= input.knobs.enable_soc {
            (EddiMode::Normal, "SoC ≥ enable threshold → Normal")
        } else if soc <= input.knobs.disable_soc {
            (EddiMode::Stopped, "SoC ≤ disable threshold → Stopped")
        } else {
            (input.current_mode, "SoC in hysteresis band → hold current mode")
        };

    let (action, dwell_note) = apply_dwell(desired, input, clock);
    let full_summary = if let Some(n) = dwell_note {
        format!("{band}; {n}")
    } else {
        band.to_string()
    };
    EddiModeOutput {
        action,
        decision: Decision {
            summary: full_summary,
            factors: factors(),
        },
    }
}

fn safe_action(target: EddiMode, input: &EddiModeInput) -> EddiModeAction {
    if target == input.current_mode { EddiModeAction::Leave(target) } else { EddiModeAction::Set(target) }
}

/// Gate a non-safety transition on the dwell timer. Returns the action
/// plus an optional note for the decision summary.
fn apply_dwell(
    desired: EddiMode,
    input: &EddiModeInput,
    clock: &dyn Clock,
) -> (EddiModeAction, Option<&'static str>) {
    if desired == input.current_mode {
        return (EddiModeAction::Leave(desired), None);
    }
    if desired == EddiMode::Stopped {
        return (EddiModeAction::Set(EddiMode::Stopped), Some("safety direction bypasses dwell"));
    }
    let dwell = Duration::from_secs(u64::from(input.knobs.dwell_s));
    let now = clock.monotonic();
    match input.last_transition_at {
        None => (EddiModeAction::Set(EddiMode::Normal), Some("first transition (no dwell)")),
        Some(prev) if now.saturating_duration_since(prev) >= dwell => {
            (EddiModeAction::Set(EddiMode::Normal), Some("dwell satisfied"))
        }
        // Dwell-not-satisfied: target intent is still `desired` (the
        // controller wants to transition); we just don't fire the
        // actuation yet. Keep target intent visible to the dashboard.
        Some(_) => (EddiModeAction::Leave(input.current_mode), Some("dwell not yet satisfied — holding")),
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use chrono::NaiveDate;

    fn clock() -> FixedClock {
        FixedClock::at(
            NaiveDate::from_ymd_opt(2026, 4, 21)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
        )
    }

    fn knobs() -> EddiModeKnobs {
        EddiModeKnobs {
            enable_soc: 96.0,
            disable_soc: 94.0,
            dwell_s: 60,
        }
    }

    fn input_with(
        soc_value: Option<f64>,
        soc_freshness: Freshness,
        current_mode: EddiMode,
        last_transition_at: Option<Instant>,
    ) -> EddiModeInput {
        EddiModeInput {
            soc_value,
            soc_freshness,
            current_mode,
            last_transition_at,
            knobs: knobs(),
        }
    }

    // ------------------------------------------------------------------
    // Safety direction: Unknown / Stale → Stopped
    // ------------------------------------------------------------------

    #[test]
    fn unknown_soc_forces_stopped() {
        let input = input_with(None, Freshness::Unknown, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn stale_soc_forces_stopped() {
        // Even with a value present, Stale freshness → Stopped.
        let input = input_with(Some(99.0), Freshness::Stale, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn deprecated_soc_forces_stopped() {
        let input = input_with(Some(99.0), Freshness::Deprecated, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn stale_soc_when_already_stopped_is_leave() {
        let input = input_with(Some(99.0), Freshness::Stale, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Leave(EddiMode::Stopped),
        );
    }

    // ------------------------------------------------------------------
    // Clear thresholds
    // ------------------------------------------------------------------

    #[test]
    fn soc_at_enable_threshold_sets_normal() {
        let input = input_with(Some(96.0), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Normal)
        );
    }

    #[test]
    fn soc_above_enable_threshold_sets_normal() {
        let input = input_with(Some(99.5), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Normal)
        );
    }

    #[test]
    fn soc_at_disable_threshold_sets_stopped() {
        let input = input_with(Some(94.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn soc_below_disable_threshold_sets_stopped() {
        let input = input_with(Some(85.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    // ------------------------------------------------------------------
    // Hysteresis band
    // ------------------------------------------------------------------

    #[test]
    fn in_hysteresis_while_normal_stays_normal() {
        let input = input_with(Some(95.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Leave(EddiMode::Normal),
        );
    }

    #[test]
    fn in_hysteresis_while_stopped_stays_stopped() {
        let input = input_with(Some(95.0), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Leave(EddiMode::Stopped),
        );
    }

    // ------------------------------------------------------------------
    // Dwell timer
    // ------------------------------------------------------------------

    #[test]
    fn first_transition_to_normal_is_immediate() {
        // No prior transition; dwell doesn't apply.
        let input = input_with(Some(99.0), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Normal)
        );
    }

    #[test]
    fn transition_to_normal_within_dwell_is_blocked() {
        let c = clock();
        let recently = c
            .monotonic()
            .checked_sub(Duration::from_secs(30))
            .unwrap(); // < 60 s dwell
        let input = input_with(
            Some(99.0),
            Freshness::Fresh,
            EddiMode::Stopped,
            Some(recently),
        );
        // Dwell-holding: target intent stays at the actual mode (the
        // controller wants Normal but is holding Stopped until the
        // dwell expires; TASS target reflects what's running, not what's
        // queued — Decision text describes the intent).
        assert_eq!(
            evaluate_eddi_mode(&input, &c).action,
            EddiModeAction::Leave(EddiMode::Stopped),
        );
    }

    #[test]
    fn transition_to_normal_after_dwell_is_allowed() {
        let c = clock();
        let long_ago = c
            .monotonic()
            .checked_sub(Duration::from_secs(120))
            .unwrap(); // > 60 s
        let input = input_with(
            Some(99.0),
            Freshness::Fresh,
            EddiMode::Stopped,
            Some(long_ago),
        );
        assert_eq!(
            evaluate_eddi_mode(&input, &c).action,
            EddiModeAction::Set(EddiMode::Normal)
        );
    }

    #[test]
    fn transition_to_stopped_bypasses_dwell() {
        // Safety direction — even within dwell, we stop.
        let c = clock();
        let recently = c
            .monotonic()
            .checked_sub(Duration::from_secs(5))
            .unwrap();
        let input = input_with(
            Some(85.0),
            Freshness::Fresh,
            EddiMode::Normal,
            Some(recently),
        );
        assert_eq!(
            evaluate_eddi_mode(&input, &c).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn no_change_when_already_in_desired_mode() {
        let input = input_with(Some(99.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Leave(EddiMode::Normal),
        );
    }

    // ------------------------------------------------------------------
    // Boundary conditions
    // ------------------------------------------------------------------

    #[test]
    fn soc_between_thresholds_with_bookkeeping_change() {
        // A Normal-to-Stopped boundary: SoC was above, now just under disable.
        let input = input_with(Some(93.9), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn fresh_some_value_missing_is_treated_as_unknown() {
        // Defensive: Fresh + value=None is inconsistent, but if the shell
        // ever constructs that (e.g. parse failure not reflected in
        // freshness), we fall back to Stopped.
        let input = input_with(None, Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()).action,
            EddiModeAction::Set(EddiMode::Stopped)
        );
    }

    // ------------------------------------------------------------------
    // PR-eddi-tariff-windows: clock-of-day overrides
    // ------------------------------------------------------------------

    fn clock_at(h: u32, m: u32) -> FixedClock {
        FixedClock::at(
            NaiveDate::from_ymd_opt(2026, 4, 26)
                .unwrap()
                .and_hms_opt(h, m, 0)
                .unwrap(),
        )
    }

    #[test]
    fn schedule_lookup_returns_correct_modes_per_window() {
        use chrono::NaiveDate;
        let at = |h: u32, m: u32| {
            NaiveDate::from_ymd_opt(2026, 4, 26)
                .unwrap()
                .and_hms_opt(h, m, 0)
                .unwrap()
        };
        // Boost cheap-rate window 02:00–05:00 → Normal.
        assert_eq!(eddi_schedule_override(at(2, 0)), Some(EddiMode::Normal));
        assert_eq!(eddi_schedule_override(at(3, 30)), Some(EddiMode::Normal));
        assert_eq!(eddi_schedule_override(at(4, 59)), Some(EddiMode::Normal));
        // Rest 05:00–07:00 → Stopped.
        assert_eq!(eddi_schedule_override(at(5, 0)), Some(EddiMode::Stopped));
        assert_eq!(eddi_schedule_override(at(6, 0)), Some(EddiMode::Stopped));
        assert_eq!(eddi_schedule_override(at(6, 59)), Some(EddiMode::Stopped));
        // Last cheap-rate hour 07:00–08:00 → Normal.
        assert_eq!(eddi_schedule_override(at(7, 0)), Some(EddiMode::Normal));
        assert_eq!(eddi_schedule_override(at(7, 59)), Some(EddiMode::Normal));
        // Outside windows → no override (SoC hysteresis rules).
        assert_eq!(eddi_schedule_override(at(8, 0)), None);
        assert_eq!(eddi_schedule_override(at(12, 0)), None);
        assert_eq!(eddi_schedule_override(at(23, 0)), None);
        assert_eq!(eddi_schedule_override(at(0, 30)), None);
        assert_eq!(eddi_schedule_override(at(1, 59)), None);
    }

    #[test]
    fn tariff_window_normal_overrides_low_soc() {
        // Below disable threshold but in the boost window — must Normal.
        let input = input_with(Some(50.0), Freshness::Fresh, EddiMode::Stopped, None);
        let out = evaluate_eddi_mode(&input, &clock_at(3, 0));
        assert_eq!(out.action, EddiModeAction::Set(EddiMode::Normal));
        assert!(out.decision.summary.contains("tariff window"));
    }

    #[test]
    fn tariff_window_stopped_overrides_high_soc() {
        // Above enable threshold but in the rest window — must Stopped.
        let input = input_with(Some(99.0), Freshness::Fresh, EddiMode::Normal, None);
        let out = evaluate_eddi_mode(&input, &clock_at(6, 0));
        assert_eq!(out.action, EddiModeAction::Set(EddiMode::Stopped));
        assert!(out.decision.summary.contains("tariff window"));
    }

    #[test]
    fn tariff_window_last_cheap_rate_hour_normal_at_high_soc() {
        // 07:00–08:00 forces Normal regardless of SoC.
        let input = input_with(Some(50.0), Freshness::Fresh, EddiMode::Stopped, None);
        let out = evaluate_eddi_mode(&input, &clock_at(7, 30));
        assert_eq!(out.action, EddiModeAction::Set(EddiMode::Normal));
    }

    #[test]
    fn outside_tariff_windows_falls_through_to_soc_hysteresis() {
        // Daytime, SoC well above enable → Normal via hysteresis (not
        // via tariff window). Decision summary must NOT mention tariff.
        let input = input_with(Some(98.0), Freshness::Fresh, EddiMode::Stopped, None);
        let out = evaluate_eddi_mode(&input, &clock_at(13, 0));
        assert_eq!(out.action, EddiModeAction::Set(EddiMode::Normal));
        assert!(!out.decision.summary.contains("tariff window"));
    }

    #[test]
    fn safety_unknown_soc_still_wins_over_tariff_window() {
        // Safety direction takes precedence over the tariff override.
        let input = input_with(None, Freshness::Unknown, EddiMode::Normal, None);
        let out = evaluate_eddi_mode(&input, &clock_at(3, 0));
        assert_eq!(out.action, EddiModeAction::Set(EddiMode::Stopped));
        assert!(out.decision.summary.contains("not Fresh"));
    }
}
