//! Zappi charge-mode controller. Ports the three distinct Zappi-touching
//! rules in the legacy NR 'Zappi' tab into one controller:
//!
//! 1. **Boost window (02:00–05:00)**: mode = Fast if `charge_car_boost`,
//!    else Off. Drives from cronplus `5-59 2` + `* 3-4`.
//! 2. **NightExtended window (05:00–08:00)**: mode = Fast if
//!    `charge_car_extended`, else Off. Drives from cronplus `* 5-7`.
//! 3. **Night-time auto-stop**: when in any Night tariff band, the user's
//!    `zappi_limit` is ≤ 65 kWh, and the session `session_kwh` has
//!    reached `zappi_limit`, force mode Off. Drives from the 15 s poll
//!    plus the legacy NR `zappi charge limit` function (A-13 + A-14).
//!
//! Outside all three of those windows (daytime, no auto-stop), returns
//! `None` — leave the mode alone so the user's manual setting in the
//! myenergi app isn't overridden.

use crate::Clock;
use crate::controllers::tariff_band::{TariffBand, TariffBandKind, tariff_band};
use crate::myenergi::ZappiMode;
use crate::types::Decision;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiModeInput {
    pub globals: ZappiModeInputGlobals,
    /// Current mode as last observed from myenergi.
    pub current_mode: ZappiMode,
    /// Session energy delivered so far (kWh), straight from myenergi's
    /// `che` field. Compared directly against `zappi_limit_kwh` — no
    /// percent-of-limit rescaling.
    pub session_kwh: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiModeInputGlobals {
    pub charge_car_boost: bool,
    pub charge_car_extended: bool,
    /// User's target per-session charge ceiling (`zappi_limit`) in kWh.
    /// The 15-s auto-stop path only runs when `zappi_limit_kwh <= 65`,
    /// which matches the legacy NR gate: only arm the stop when the user
    /// configured a sub-full-charge cap.
    pub zappi_limit_kwh: f64,
}

/// Decision returned by the controller: either "set the mode to X" or
/// "don't touch" (the latter when no rule applies, so the user's
/// manual choice in the myenergi app is preserved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZappiModeAction {
    Set(ZappiMode),
    Leave,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZappiModeOutput {
    pub action: ZappiModeAction,
    pub decision: Decision,
}

/// Evaluate the desired Zappi mode target for the current moment.
#[must_use]
pub fn evaluate_zappi_mode(input: &ZappiModeInput, clock: &dyn Clock) -> ZappiModeOutput {
    let g = &input.globals;
    let now = clock.naive();
    let band = tariff_band(now);
    let common = Decision::new("placeholder")
        .with_factor("tariff", format!("{band:?}"))
        .with_factor("charge_car_boost", format!("{}", g.charge_car_boost))
        .with_factor("charge_car_extended", format!("{}", g.charge_car_extended))
        .with_factor("current_mode", format!("{:?}", input.current_mode))
        .with_factor("zappi_limit_kwh", format!("{:.2} kWh", g.zappi_limit_kwh))
        .with_factor("session_kwh", format!("{:.2} kWh", input.session_kwh));

    // 1. Boost window — flag-driven Fast/Off.
    if band == TariffBand::BOOST {
        let mode = if g.charge_car_boost { ZappiMode::Fast } else { ZappiMode::Off };
        return ZappiModeOutput {
            action: ZappiModeAction::Set(mode),
            decision: Decision {
                summary: format!("Boost window (02:00–05:00) → mode={mode:?} (driven by charge_car_boost)"),
                factors: common.factors,
            },
        };
    }

    // 2. NightExtended — flag-driven Fast/Off.
    if band == TariffBand::NIGHT_EXTENDED {
        let mode = if g.charge_car_extended { ZappiMode::Fast } else { ZappiMode::Off };
        return ZappiModeOutput {
            action: ZappiModeAction::Set(mode),
            decision: Decision {
                summary: format!(
                    "NightExtended window (05:00–08:00) → mode={mode:?} (driven by charge_car_extended)"
                ),
                factors: common.factors,
            },
        };
    }

    // 3. Night-time auto-stop. The `<= 65 kWh` gate mirrors the legacy
    // NR behaviour: auto-stop only fires when the user has configured a
    // sub-full-charge session cap; above 65 kWh (typical big-battery
    // full-charge figure) we assume the user wants the Zappi to run to
    // completion on its own schedule.
    let is_night = band.kind == TariffBandKind::Night;
    if is_night
        && g.zappi_limit_kwh <= 65.0
        && input.session_kwh >= g.zappi_limit_kwh
        && input.current_mode != ZappiMode::Off
    {
        return ZappiModeOutput {
            action: ZappiModeAction::Set(ZappiMode::Off),
            decision: Decision {
                summary: format!(
                    "Night auto-stop — session {:.2} kWh ≥ zappi_limit {:.2} kWh → Off",
                    input.session_kwh, g.zappi_limit_kwh
                ),
                factors: common.factors,
            },
        };
    }

    // Daytime + all other cases — don't touch.
    ZappiModeOutput {
        action: ZappiModeAction::Leave,
        decision: Decision {
            summary: "No rule applies — leaving Zappi mode unchanged".to_string(),
            factors: common.factors,
        },
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

    fn clock_at(h: u32, m: u32) -> FixedClock {
        let nt = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap();
        FixedClock::at(nt)
    }

    fn base_input() -> ZappiModeInput {
        ZappiModeInput {
            globals: ZappiModeInputGlobals {
                charge_car_boost: false,
                charge_car_extended: false,
                // 100 kWh = auto-stop disarmed (above the 65 kWh gate).
                zappi_limit_kwh: 100.0,
            },
            current_mode: ZappiMode::Eco,
            session_kwh: 0.0,
        }
    }

    // ------------------------------------------------------------------
    // Boost window
    // ------------------------------------------------------------------

    #[test]
    fn boost_window_with_charge_car_boost_sets_fast() {
        let mut input = base_input();
        input.globals.charge_car_boost = true;
        let d = evaluate_zappi_mode(&input, &clock_at(3, 0));
        assert_eq!(d.action, ZappiModeAction::Set(ZappiMode::Fast));
    }

    #[test]
    fn boost_window_without_flag_sets_off() {
        let input = base_input();
        let d = evaluate_zappi_mode(&input, &clock_at(3, 0));
        assert_eq!(d.action, ZappiModeAction::Set(ZappiMode::Off));
    }

    // ------------------------------------------------------------------
    // NightExtended window
    // ------------------------------------------------------------------

    #[test]
    fn extended_window_with_charge_car_extended_sets_fast() {
        let mut input = base_input();
        input.globals.charge_car_extended = true;
        let d = evaluate_zappi_mode(&input, &clock_at(6, 30));
        assert_eq!(d.action, ZappiModeAction::Set(ZappiMode::Fast));
    }

    #[test]
    fn extended_window_without_flag_sets_off() {
        let input = base_input();
        let d = evaluate_zappi_mode(&input, &clock_at(6, 30));
        assert_eq!(d.action, ZappiModeAction::Set(ZappiMode::Off));
    }

    // ------------------------------------------------------------------
    // Night auto-stop
    // ------------------------------------------------------------------

    #[test]
    fn night_start_auto_stop_triggers_when_over_limit() {
        // User set 50 kWh cap; car already pulled 60 kWh → stop.
        let mut input = base_input();
        input.globals.zappi_limit_kwh = 50.0;
        input.session_kwh = 60.0;
        input.current_mode = ZappiMode::Eco;
        // NightStart (23:30)
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d.action, ZappiModeAction::Set(ZappiMode::Off));
    }

    #[test]
    fn zappi_mode_stops_when_session_kwh_meets_kwh_limit() {
        // A-13 / A-14 fresh coverage: compare kWh-to-kWh with a small
        // overshoot (session = 20.5 kWh, cap = 20 kWh) and confirm the
        // controller forces the Zappi Off.
        let mut input = base_input();
        input.globals.zappi_limit_kwh = 20.0;
        input.session_kwh = 20.5;
        input.current_mode = ZappiMode::Eco;
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d.action, ZappiModeAction::Set(ZappiMode::Off));
        // Honesty invariant: the Decision must surface both operands in
        // the new kWh semantic.
        let factor_names: Vec<_> = d.decision.factors.iter().map(|f| f.name.as_str()).collect();
        assert!(factor_names.contains(&"zappi_limit_kwh"));
        assert!(factor_names.contains(&"session_kwh"));
    }

    #[test]
    fn night_auto_stop_noop_when_already_off() {
        let mut input = base_input();
        input.globals.zappi_limit_kwh = 50.0;
        input.session_kwh = 60.0;
        input.current_mode = ZappiMode::Off;
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d.action, ZappiModeAction::Leave);
    }

    #[test]
    fn night_auto_stop_skipped_when_limit_above_65() {
        // 90 kWh cap means "charge whatever" — auto-stop disarmed even
        // if session would technically be ≥ limit.
        let mut input = base_input();
        input.globals.zappi_limit_kwh = 90.0;
        input.session_kwh = 95.0;
        input.current_mode = ZappiMode::Eco;
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d.action, ZappiModeAction::Leave);
    }

    #[test]
    fn night_auto_stop_skipped_when_under_limit() {
        let mut input = base_input();
        input.globals.zappi_limit_kwh = 50.0;
        input.session_kwh = 30.0;
        input.current_mode = ZappiMode::Eco;
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d.action, ZappiModeAction::Leave);
    }

    // ------------------------------------------------------------------
    // Daytime
    // ------------------------------------------------------------------

    #[test]
    fn daytime_always_leaves_mode_alone() {
        let mut input = base_input();
        input.current_mode = ZappiMode::Eco;
        input.globals.zappi_limit_kwh = 50.0;
        input.session_kwh = 80.0;
        // Daytime — even with all auto-stop conditions met, we don't touch.
        let d = evaluate_zappi_mode(&input, &clock_at(12, 0));
        assert_eq!(d.action, ZappiModeAction::Leave);
    }

    #[test]
    fn peak_window_leaves_mode_alone() {
        let input = base_input();
        let d = evaluate_zappi_mode(&input, &clock_at(18, 0));
        assert_eq!(d.action, ZappiModeAction::Leave);
    }

    // ------------------------------------------------------------------
    // Window precedence
    // ------------------------------------------------------------------

    #[test]
    fn boost_window_with_auto_stop_conditions_still_uses_boost_rule() {
        let mut input = base_input();
        input.globals.charge_car_boost = true;
        input.globals.zappi_limit_kwh = 50.0;
        input.session_kwh = 80.0;
        input.current_mode = ZappiMode::Eco;
        // Boost rule wins — mode becomes Fast regardless of over-limit.
        let d = evaluate_zappi_mode(&input, &clock_at(3, 0));
        assert_eq!(d.action, ZappiModeAction::Set(ZappiMode::Fast));
    }
}
