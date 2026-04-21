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

/// Decision: explicit target or leave-alone. A `Set` decision that
/// matches `current_mode` is semantically a no-op but still returned so
/// that the outer `process()` can drive the TASS phase machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EddiModeDecision {
    Set(EddiMode),
    Leave,
}

/// Evaluate the desired Eddi mode target.
#[must_use]
pub fn evaluate_eddi_mode(input: &EddiModeInput, clock: &dyn Clock) -> EddiModeDecision {
    // Safety: SoC unknown or stale → target Stopped.
    if input.soc_freshness != Freshness::Fresh {
        return safe_decision(EddiMode::Stopped, input);
    }
    let Some(soc) = input.soc_value else {
        return safe_decision(EddiMode::Stopped, input);
    };

    // Clear cases — above enable threshold = Normal; at/below disable = Stopped.
    // Hysteresis band in between: hold current mode.
    let desired = if soc >= input.knobs.enable_soc {
        EddiMode::Normal
    } else if soc <= input.knobs.disable_soc {
        EddiMode::Stopped
    } else {
        // In hysteresis band — keep current mode.
        input.current_mode
    };

    apply_dwell(desired, input, clock)
}

/// Short-circuit that enforces the dwell timer for safety-direction
/// transitions too. The safety direction is `Stopped`, so we always
/// apply it even within dwell — only the *transition to Normal* is
/// gated by dwell.
fn safe_decision(target: EddiMode, input: &EddiModeInput) -> EddiModeDecision {
    if target == input.current_mode {
        EddiModeDecision::Leave
    } else {
        EddiModeDecision::Set(target)
    }
}

/// Gate a non-safety transition on the dwell timer.
fn apply_dwell(
    desired: EddiMode,
    input: &EddiModeInput,
    clock: &dyn Clock,
) -> EddiModeDecision {
    // No-op if we already match.
    if desired == input.current_mode {
        return EddiModeDecision::Leave;
    }

    // Safety direction (→ Stopped) is never dwell-gated.
    if desired == EddiMode::Stopped {
        return EddiModeDecision::Set(EddiMode::Stopped);
    }

    // Normal requires dwell satisfied or first transition.
    let dwell = Duration::from_secs(u64::from(input.knobs.dwell_s));
    let now = clock.monotonic();
    match input.last_transition_at {
        None => EddiModeDecision::Set(EddiMode::Normal),
        Some(prev) if now.saturating_duration_since(prev) >= dwell => {
            EddiModeDecision::Set(EddiMode::Normal)
        }
        Some(_) => EddiModeDecision::Leave,
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
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn stale_soc_forces_stopped() {
        // Even with a value present, Stale freshness → Stopped.
        let input = input_with(Some(99.0), Freshness::Stale, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn deprecated_soc_forces_stopped() {
        let input = input_with(Some(99.0), Freshness::Deprecated, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn stale_soc_when_already_stopped_is_leave() {
        let input = input_with(Some(99.0), Freshness::Stale, EddiMode::Stopped, None);
        assert_eq!(evaluate_eddi_mode(&input, &clock()), EddiModeDecision::Leave);
    }

    // ------------------------------------------------------------------
    // Clear thresholds
    // ------------------------------------------------------------------

    #[test]
    fn soc_at_enable_threshold_sets_normal() {
        let input = input_with(Some(96.0), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Normal)
        );
    }

    #[test]
    fn soc_above_enable_threshold_sets_normal() {
        let input = input_with(Some(99.5), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Normal)
        );
    }

    #[test]
    fn soc_at_disable_threshold_sets_stopped() {
        let input = input_with(Some(94.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn soc_below_disable_threshold_sets_stopped() {
        let input = input_with(Some(85.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }

    // ------------------------------------------------------------------
    // Hysteresis band
    // ------------------------------------------------------------------

    #[test]
    fn in_hysteresis_while_normal_stays_normal() {
        let input = input_with(Some(95.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(evaluate_eddi_mode(&input, &clock()), EddiModeDecision::Leave);
    }

    #[test]
    fn in_hysteresis_while_stopped_stays_stopped() {
        let input = input_with(Some(95.0), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(evaluate_eddi_mode(&input, &clock()), EddiModeDecision::Leave);
    }

    // ------------------------------------------------------------------
    // Dwell timer
    // ------------------------------------------------------------------

    #[test]
    fn first_transition_to_normal_is_immediate() {
        // No prior transition; dwell doesn't apply.
        let input = input_with(Some(99.0), Freshness::Fresh, EddiMode::Stopped, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Normal)
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
        assert_eq!(evaluate_eddi_mode(&input, &c), EddiModeDecision::Leave);
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
            evaluate_eddi_mode(&input, &c),
            EddiModeDecision::Set(EddiMode::Normal)
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
            evaluate_eddi_mode(&input, &c),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn no_change_when_already_in_desired_mode() {
        let input = input_with(Some(99.0), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(evaluate_eddi_mode(&input, &clock()), EddiModeDecision::Leave);
    }

    // ------------------------------------------------------------------
    // Boundary conditions
    // ------------------------------------------------------------------

    #[test]
    fn soc_between_thresholds_with_bookkeeping_change() {
        // A Normal-to-Stopped boundary: SoC was above, now just under disable.
        let input = input_with(Some(93.9), Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }

    #[test]
    fn fresh_some_value_missing_is_treated_as_unknown() {
        // Defensive: Fresh + value=None is inconsistent, but if the shell
        // ever constructs that (e.g. parse failure not reflected in
        // freshness), we fall back to Stopped.
        let input = input_with(None, Freshness::Fresh, EddiMode::Normal, None);
        assert_eq!(
            evaluate_eddi_mode(&input, &clock()),
            EddiModeDecision::Set(EddiMode::Stopped)
        );
    }
}
