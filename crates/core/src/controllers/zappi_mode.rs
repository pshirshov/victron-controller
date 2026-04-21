//! Zappi charge-mode controller. Ports the three distinct Zappi-touching
//! rules in the legacy NR 'Zappi' tab into one controller:
//!
//! 1. **Boost window (02:00–05:00)**: mode = Fast if `charge_car_boost`,
//!    else Off. Drives from cronplus `5-59 2` + `* 3-4`.
//! 2. **NightExtended window (05:00–08:00)**: mode = Fast if
//!    `charge_car_extended`, else Off. Drives from cronplus `* 5-7`.
//! 3. **Night-time auto-stop**: when in any Night tariff band, the Zappi
//!    `zappi_limit` is ≤ 65 %, and the session `charged_pct` ≥ `zappi_limit`,
//!    force mode Off. Drives from the 15 s poll + `zappi charge limit`
//!    function (lines 503-545 of legacy/debug/…-functions.txt).
//!
//! Outside all three of those windows (daytime, no auto-stop), returns
//! `None` — leave the mode alone so the user's manual setting in the
//! myenergi app isn't overridden.

use crate::Clock;
use crate::controllers::tariff_band::{TariffBand, TariffBandKind, tariff_band};
use crate::myenergi::ZappiMode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiModeInput {
    pub globals: ZappiModeInputGlobals,
    /// Current mode as last observed from myenergi.
    pub current_mode: ZappiMode,
    /// Session-charged percentage of the configured limit, i.e.
    /// `min(100, round(session_kwh / limit_kwh * 100))`. Legacy NR reads
    /// `msg.payload.che` (session kWh) and compares to `limit`. In this
    /// controller we expect the shell to do the scale alignment.
    pub session_charged_pct: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiModeInputGlobals {
    pub charge_car_boost: bool,
    pub charge_car_extended: bool,
    /// User's target charge-ceiling (`zappi_limit`) as a percent 1..100.
    /// The 15-s auto-stop path only runs when `zappi_limit <= 65`.
    pub zappi_limit_pct: f64,
}

/// Decision returned by the controller: either "set the mode to X" or
/// "don't touch" (the latter when no rule applies, so the user's
/// manual choice in the myenergi app is preserved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZappiModeDecision {
    Set(ZappiMode),
    Leave,
}

/// Evaluate the desired Zappi mode target for the current moment.
#[must_use]
pub fn evaluate_zappi_mode(input: &ZappiModeInput, clock: &dyn Clock) -> ZappiModeDecision {
    let g = &input.globals;
    let now = clock.naive();
    let band = tariff_band(now);

    // 1. Boost window — flag-driven Fast/Off.
    if band == TariffBand::BOOST {
        return ZappiModeDecision::Set(if g.charge_car_boost {
            ZappiMode::Fast
        } else {
            ZappiMode::Off
        });
    }

    // 2. NightExtended — flag-driven Fast/Off.
    if band == TariffBand::NIGHT_EXTENDED {
        return ZappiModeDecision::Set(if g.charge_car_extended {
            ZappiMode::Fast
        } else {
            ZappiMode::Off
        });
    }

    // 3. Night-time auto-stop (covers NightStart 23-02 and any other Night-
    //    kind band not handled above, which in current set is just
    //    NightStart).
    let is_night = band.kind == TariffBandKind::Night;
    if is_night
        && g.zappi_limit_pct <= 65.0
        && input.session_charged_pct >= g.zappi_limit_pct
        && input.current_mode != ZappiMode::Off
    {
        return ZappiModeDecision::Set(ZappiMode::Off);
    }

    // Daytime + all other cases — don't touch.
    ZappiModeDecision::Leave
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
                zappi_limit_pct: 100.0,
            },
            current_mode: ZappiMode::Eco,
            session_charged_pct: 0.0,
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
        assert_eq!(d, ZappiModeDecision::Set(ZappiMode::Fast));
    }

    #[test]
    fn boost_window_without_flag_sets_off() {
        let input = base_input();
        let d = evaluate_zappi_mode(&input, &clock_at(3, 0));
        assert_eq!(d, ZappiModeDecision::Set(ZappiMode::Off));
    }

    // ------------------------------------------------------------------
    // NightExtended window
    // ------------------------------------------------------------------

    #[test]
    fn extended_window_with_charge_car_extended_sets_fast() {
        let mut input = base_input();
        input.globals.charge_car_extended = true;
        let d = evaluate_zappi_mode(&input, &clock_at(6, 30));
        assert_eq!(d, ZappiModeDecision::Set(ZappiMode::Fast));
    }

    #[test]
    fn extended_window_without_flag_sets_off() {
        let input = base_input();
        let d = evaluate_zappi_mode(&input, &clock_at(6, 30));
        assert_eq!(d, ZappiModeDecision::Set(ZappiMode::Off));
    }

    // ------------------------------------------------------------------
    // Night auto-stop
    // ------------------------------------------------------------------

    #[test]
    fn night_start_auto_stop_triggers_when_over_limit() {
        let mut input = base_input();
        input.globals.zappi_limit_pct = 50.0;
        input.session_charged_pct = 60.0;
        input.current_mode = ZappiMode::Eco;
        // NightStart (23:30)
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d, ZappiModeDecision::Set(ZappiMode::Off));
    }

    #[test]
    fn night_auto_stop_noop_when_already_off() {
        let mut input = base_input();
        input.globals.zappi_limit_pct = 50.0;
        input.session_charged_pct = 60.0;
        input.current_mode = ZappiMode::Off;
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d, ZappiModeDecision::Leave);
    }

    #[test]
    fn night_auto_stop_skipped_when_limit_above_65() {
        let mut input = base_input();
        input.globals.zappi_limit_pct = 90.0;
        input.session_charged_pct = 95.0;
        input.current_mode = ZappiMode::Eco;
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d, ZappiModeDecision::Leave);
    }

    #[test]
    fn night_auto_stop_skipped_when_under_limit() {
        let mut input = base_input();
        input.globals.zappi_limit_pct = 50.0;
        input.session_charged_pct = 30.0;
        input.current_mode = ZappiMode::Eco;
        let d = evaluate_zappi_mode(&input, &clock_at(23, 30));
        assert_eq!(d, ZappiModeDecision::Leave);
    }

    // ------------------------------------------------------------------
    // Daytime
    // ------------------------------------------------------------------

    #[test]
    fn daytime_always_leaves_mode_alone() {
        let mut input = base_input();
        input.current_mode = ZappiMode::Eco;
        input.globals.zappi_limit_pct = 50.0;
        input.session_charged_pct = 80.0;
        // Daytime — even with all auto-stop conditions met, we don't touch.
        let d = evaluate_zappi_mode(&input, &clock_at(12, 0));
        assert_eq!(d, ZappiModeDecision::Leave);
    }

    #[test]
    fn peak_window_leaves_mode_alone() {
        let input = base_input();
        let d = evaluate_zappi_mode(&input, &clock_at(18, 0));
        assert_eq!(d, ZappiModeDecision::Leave);
    }

    // ------------------------------------------------------------------
    // Window precedence
    // ------------------------------------------------------------------

    #[test]
    fn boost_window_with_auto_stop_conditions_still_uses_boost_rule() {
        let mut input = base_input();
        input.globals.charge_car_boost = true;
        input.globals.zappi_limit_pct = 50.0;
        input.session_charged_pct = 80.0;
        input.current_mode = ZappiMode::Eco;
        // Boost rule wins — mode becomes Fast regardless of over-limit.
        let d = evaluate_zappi_mode(&input, &clock_at(3, 0));
        assert_eq!(d, ZappiModeDecision::Set(ZappiMode::Fast));
    }
}
