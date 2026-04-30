//! User-tunable knobs. See SPEC §7 for the safe-defaults baseline.
//!
//! On cold start the service actuates with [`Knobs::safe_defaults`]; retained
//! MQTT values then overwrite fields as they arrive (with the explicit
//! exception of `allow_battery_to_car`, which *always* boots `false`).

/// End-of-evening discharge target time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DischargeTime {
    /// Discharge until 02:00 (Night-Start continues through 02:00).
    #[default]
    At0200,
    /// Discharge until 23:00 (truncate early; used on tariffs with a 23:00
    /// peak-to-off-peak transition).
    At2300,
}

/// Manual override for the weekly Sunday-17:00 full-charge rollover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugFullCharge {
    /// Do not schedule a full charge, regardless of `next_full_charge`.
    Forbid,
    /// Force a full charge on the next evaluation cycle.
    Force,
    /// Follow the `next_full_charge` schedule (default).
    #[default]
    Auto,
}

/// Fusion strategy when forecast providers disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForecastDisagreementStrategy {
    Max,
    Mean,
    Min,
    #[default]
    SolcastIfAvailableElseMean,
}

/// PR-gamma-hold-redesign. Per-knob source selector for the four
/// weather_soc-driven outputs: `Weather` reads the matching
/// `bookkeeping.weather_soc_*` slot (default — preserves prior implicit
/// behaviour where the planner drove these knobs); `Forced` reads the
/// user-owned knob value directly. There is no priority queue and no
/// γ-hold — the user picks the source explicitly per knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Weather,
    Forced,
}

/// Override for the `charge_battery_extended` bit that the schedules
/// controller consults. Legacy derivation is
/// `!disable_night_grid_discharge || charge_to_full_required`, but the
/// user sometimes wants to pin it on or off regardless.
///
///   * `Auto` — use the legacy-derived value (default).
///   * `Forced` — always `true`, even when nothing else would set it.
///   * `Disabled` — always `false`, even when derivation says yes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChargeBatteryExtendedMode {
    #[default]
    Auto,
    Forced,
    Disabled,
}

/// PR-auto-extended-charge: replaces the legacy `charge_car_extended`
/// boolean. Controls whether the NightExtended (05:00–08:00) Zappi
/// window pulls cheap-rate grid power into the EV.
///
///   * `Auto` — daily 04:30 evaluation: enable for tonight when
///     `ev_soc < 40` OR `ev_charge_target > 80`. Stale `ev_soc`
///     defensively disables.
///   * `Forced` — always `true`, regardless of EV state.
///   * `Disabled` — always `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtendedChargeMode {
    #[default]
    Auto,
    Forced,
    Disabled,
}

/// User-controlled knobs. One struct, one source of truth.
///
/// Defaults come from [`Knobs::safe_defaults`]; see SPEC §7.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Knobs {
    // --- Export / discharge policy ---
    pub force_disable_export: bool,
    /// Export when battery SoC (%) is at or above this threshold.
    pub export_soc_threshold: f64,
    /// Evening controller targets this SoC (%) at end-of-day.
    pub discharge_soc_target: f64,
    /// Night-time scheduled charge target (%).
    pub battery_soc_target: f64,
    /// Evening target (%) during weekly full-charge.
    pub full_charge_discharge_soc_target: f64,
    /// Export threshold (%) during weekly full-charge.
    pub full_charge_export_soc_threshold: f64,
    pub discharge_time: DischargeTime,
    pub debug_full_charge: DebugFullCharge,
    pub pessimism_multiplier_modifier: f64,
    pub disable_night_grid_discharge: bool,

    // --- Zappi ---
    pub charge_car_boost: bool,
    /// PR-auto-extended-charge: tri-state knob replacing the legacy
    /// `charge_car_extended: bool`. The effective per-tick boolean used
    /// by controllers comes from `process::effective_charge_car_extended`
    /// (consults bookkeeping when `Auto`).
    pub charge_car_extended_mode: ExtendedChargeMode,
    pub zappi_current_target: f64,
    /// Per-session EV charge ceiling in **kWh** (A-14: was `%` in earlier
    /// revisions; now matches the legacy NR semantic). The Zappi-mode
    /// controller compares `ZappiState::session_kwh` against this value
    /// during Night tariff bands and forces the mode Off once the car
    /// has drawn ≥ `zappi_limit` kWh — but only when `zappi_limit ≤ 65`
    /// (the legacy "only arm when user configured a sub-full-charge
    /// cap" gate).
    pub zappi_limit: f64,
    pub zappi_emergency_margin: f64,

    // --- New knobs (SPEC §2.10a) ---
    /// Hard cap on negative setpoint magnitude (grid-side export limit, W).
    pub grid_export_limit_w: u32,
    /// Hard cap on positive setpoint magnitude (grid-side import limit, W).
    pub grid_import_limit_w: u32,
    /// Optionally allow discharging DC battery into the EV during Zappi-active
    /// windows. Always boots `false` regardless of retained value.
    pub allow_battery_to_car: bool,
    /// Eddi target becomes Normal when battery SoC ≥ this (%).
    pub eddi_enable_soc: f64,
    /// Eddi target becomes Stopped when battery SoC ≤ this (%).
    pub eddi_disable_soc: f64,
    /// Minimum dwell time (s) at the current Eddi state before re-evaluation.
    pub eddi_dwell_s: u32,

    // --- Weather-SoC planner thresholds ---
    pub weathersoc_winter_temperature_threshold: f64,
    pub weathersoc_low_energy_threshold: f64,
    pub weathersoc_ok_energy_threshold: f64,
    pub weathersoc_high_energy_threshold: f64,
    pub weathersoc_too_much_energy_threshold: f64,

    // --- Ops ---
    pub writes_enabled: bool,
    pub forecast_disagreement_strategy: ForecastDisagreementStrategy,
    /// Manual override for the legacy `charge_battery_extended`
    /// derivation. Default `Auto` → use the derived value.
    pub charge_battery_extended_mode: ChargeBatteryExtendedMode,

    // --- PR-gamma-hold-redesign: per-knob source selectors ---
    /// `Weather` (default): controllers read
    /// `bookkeeping.weather_soc_export_soc_threshold`. `Forced`:
    /// controllers read `knobs.export_soc_threshold` directly.
    pub export_soc_threshold_mode: Mode,
    pub discharge_soc_target_mode: Mode,
    pub battery_soc_target_mode: Mode,
    pub disable_night_grid_discharge_mode: Mode,

    /// PR-safe-discharge-enable: gates the legacy 4020 W "inverter
    /// safe discharge" margin in the setpoint controller's
    /// `max_discharge` formula. When `false` (default) the safety
    /// margin is OFF — setpoint discharges at full
    /// `inverter_max_discharge_w`. When `true`, the legacy margin
    /// applies (was empirically calibrated against an observed
    /// "forced grid charge during 4.8k+ discharge" inverter glitch
    /// on some MultiPlus firmware).
    pub inverter_safe_discharge_enable: bool,

    // --- PR-baseline-forecast: 4 runtime knobs steering the local
    // last-resort forecast. Dates are encoded as `MMDD` (e.g. 1101 for
    // November 1, 301 for March 1) to keep one knob per conceptual
    // setting; the baseline scheduler validates and falls back to the
    // (1101, 301) defaults if a value is malformed at runtime.
    /// Inclusive winter MM-DD start, encoded as `month*100 + day`.
    pub baseline_winter_start_mm_dd: u32,
    /// Inclusive winter MM-DD end, encoded as `month*100 + day`.
    pub baseline_winter_end_mm_dd: u32,
    /// Average per-hour Wh produced during winter daylight hours.
    pub baseline_wh_per_hour_winter: f64,
    /// Average per-hour Wh produced during summer daylight hours.
    pub baseline_wh_per_hour_summer: f64,

    // --- PR-keep-batteries-charged ---
    /// Gate the daytime ESS-state override on full-charge days. When
    /// `true` AND `bookkeeping.charge_to_full_required` AND `now ∈
    /// [sunrise + offset, sunset - offset]`, the controller writes
    /// ESS state 9 (KeepBatteriesCharged); otherwise it writes 10
    /// (Optimized). The knob being `false` is equivalent to "always
    /// write 10".
    pub keep_batteries_charged_during_full_charge: bool,
    /// Inset (minutes) applied symmetrically to local sunrise and
    /// sunset to delimit the override window. Default 60 keeps the
    /// override well clear of dawn/dusk shoulder periods.
    pub sunrise_sunset_offset_min: u32,

    /// When true, the SoC ≥ 99.99 weekly rollover always lands on the
    /// Sunday at-or-after `now + 7d`, never snapping back to the
    /// current week's Sunday. Default `false` preserves legacy
    /// behaviour (Mon/Tue/Wed snap back to this week's Sunday).
    pub full_charge_defer_to_next_sunday: bool,

    /// Inclusive weekday cap for the snap-back branch of the SoC ≥
    /// 99.99 rollover. With `num_days_from_sunday` encoding (Sun=0,
    /// Mon=1, ..., Sat=6), snap-back fires when the resulting `dow
    /// <= cap`; otherwise the date is pushed forward to the next
    /// Sunday. Range 1..=5; default 3 preserves legacy (Mon/Tue/Wed
    /// snap back; Thu/Fri/Sat push forward). Ignored when
    /// `full_charge_defer_to_next_sunday` is on.
    pub full_charge_snap_back_max_weekday: u32,

    // --- PR-ZD-2: compensated battery-drain feedback loop knobs ---
    /// Compensated drain threshold (W). When
    /// `compensated_drain = max(0, -battery_dc_power - heat_pump - cooker)`
    /// exceeds this value while Zappi is active and
    /// `allow_battery_to_car=false`, the controller tightens the
    /// grid setpoint to halt battery discharge into the EV.
    pub zappi_battery_drain_threshold_w: u32,
    /// Setpoint-relax step (W per controller tick). When compensated
    /// drain is below the threshold, the controller relaxes the grid
    /// setpoint toward `-solar_export` at this step size per tick.
    pub zappi_battery_drain_relax_step_w: u32,
    /// Proportional gain on the compensated-drain controller.
    pub zappi_battery_drain_kp: f64,
    /// Reference for the compensated-drain controller (W). Reserved
    /// for a future PI extension; inert in the current soft loop.
    /// Routes via `KnobValue::Float` because no `Int32` variant exists.
    pub zappi_battery_drain_target_w: i32,
    /// Fast-mode hard-clamp threshold (W). When Zappi is in Fast mode
    /// and `allow_battery_to_car=false`, if compensated drain exceeds
    /// this value, the controller raises the proposed setpoint by the
    /// excess as a belt-and-suspenders safety net.
    pub zappi_battery_drain_hard_clamp_w: u32,
    /// PR-ZDP-1: MPPT probe offset (W). When at least one MPPT reports
    /// voltage/current limited (mode 1 — curtailed by the inverter),
    /// the relax target is pushed deeper than observed `-solar_export`
    /// by this amount. Set to 0 to disable probing entirely.
    pub zappi_battery_drain_mppt_probe_w: u32,
}

impl Knobs {
    /// Cold-start safe defaults. Chosen per SPEC §7 to match the user's
    /// "keep battery around 80, schedule-only grid charging, cap grid export
    /// at 5000 W, don't discharge battery" policy.
    #[must_use]
    pub fn safe_defaults() -> Self {
        Self {
            force_disable_export: false,
            export_soc_threshold: 80.0,
            discharge_soc_target: 80.0,
            battery_soc_target: 80.0,
            full_charge_discharge_soc_target: 57.0,
            full_charge_export_soc_threshold: 100.0,
            discharge_time: DischargeTime::At0200,
            debug_full_charge: DebugFullCharge::Auto,
            pessimism_multiplier_modifier: 1.0,
            disable_night_grid_discharge: false,
            // Default true: during the cheap-tariff Boost window
            // (02:00–05:00) we want the Zappi to draw `Fast` unless the
            // operator explicitly disables it. The user-facing dashboard
            // / HA toggle stays as the override.
            charge_car_boost: true,
            // PR-auto-extended-charge: default `Auto`. The 04:30 daily
            // evaluation enables NightExtended for tonight only when the
            // EV's reported SoC / target says it actually needs the
            // cheap-rate window. Operator can flip to `Forced`/`Disabled`
            // from the dashboard / HA to override.
            charge_car_extended_mode: ExtendedChargeMode::Auto,
            zappi_current_target: 9.5,
            // A-14: kWh, not %. 65 kWh covers a Tesla Model 3 LR full
            // charge and sits on the `<= 65` gate boundary in the
            // zappi-mode controller, so auto-stop is armed by default
            // for typical EV sessions. Tune per vehicle.
            zappi_limit: 65.0,
            zappi_emergency_margin: 5.0,
            grid_export_limit_w: 5000,
            grid_import_limit_w: 10,
            allow_battery_to_car: false,
            eddi_enable_soc: 96.0,
            eddi_disable_soc: 94.0,
            eddi_dwell_s: 60,
            weathersoc_winter_temperature_threshold: 12.0,
            // Energy-band thresholds calibrated for the user's
            // installation (peak ~50 kWh good-day forecast). The Node-RED
            // legacy values (12/20/80/80) were tuned for a much larger
            // installation; defaulting to those left every day below the
            // 80 kWh "high" rung, which fires `disable_export` and pins
            // `export_soc_threshold = 100` — the controller would then
            // hold at idle 10 W on a sunny 52 kWh forecast day. Updated
            // 2026-04-25 after live observation; SPEC §3.4 documents the
            // sizing rationale.
            weathersoc_low_energy_threshold: 8.0,
            weathersoc_ok_energy_threshold: 15.0,
            weathersoc_high_energy_threshold: 30.0,
            weathersoc_too_much_energy_threshold: 45.0,
            // Safe cold-start: no actuation effects until either a
            // retained MQTT `<root>/writes_enabled/state = true` seeds us
            // or a user flips the kill switch explicitly from the
            // dashboard. Bias-to-safety per SPEC §7; observer-mode is
            // the default, opt-in to act.
            writes_enabled: false,
            forecast_disagreement_strategy: ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
            charge_battery_extended_mode: ChargeBatteryExtendedMode::Auto,
            // PR-gamma-hold-redesign: planner-driven defaults so prior
            // behaviour (weather_soc owns these knobs) is preserved out
            // of the box. Flip to `Forced` from the dashboard / HA to
            // pin the user-set knob value through.
            export_soc_threshold_mode: Mode::Weather,
            discharge_soc_target_mode: Mode::Weather,
            battery_soc_target_mode: Mode::Weather,
            disable_night_grid_discharge_mode: Mode::Weather,
            // PR-safe-discharge-enable: default OFF — full inverter
            // discharge rate. User explicitly chose this default after
            // confirming their MultiPlus firmware doesn't reproduce
            // the legacy "forced grid charge during 4.8k+" glitch.
            inverter_safe_discharge_enable: false,
            // PR-baseline-forecast: defaults are the canonical NH
            // winter (Nov 1 .. Mar 1) and rough order-of-magnitude
            // Wh-per-daylight-hour figures. Operator overrides via the
            // dashboard / HA / retained MQTT.
            baseline_winter_start_mm_dd: 1101,
            baseline_winter_end_mm_dd: 301,
            baseline_wh_per_hour_winter: 100.0,
            baseline_wh_per_hour_summer: 1000.0,
            // PR-keep-batteries-charged: opt-in. The override is only
            // useful when the operator's tariff makes daytime
            // self-consumption from the grid expensive and the topology
            // can absorb a forced "stay full" branch — defaults to
            // disabled so a fresh deployment keeps the legacy behaviour.
            keep_batteries_charged_during_full_charge: false,
            // 60 min keeps the override clear of the shoulder hour
            // around sunrise/sunset where the sun crate's accuracy
            // matters less — and matches the user-stated default.
            sunrise_sunset_offset_min: 60,
            // Default off — preserve the legacy Mon/Tue/Wed snap-back
            // to the current week's Sunday.
            full_charge_defer_to_next_sunday: false,
            // Default 3 (Wednesday) preserves legacy behaviour: dow ≤ 3
            // → snap back; dow > 3 → push forward.
            full_charge_snap_back_max_weekday: 3,
            // PR-ZD-2: compensated battery-drain feedback loop knobs.
            // All are install-time config; operators reach them via
            // [knobs] in config.toml or the HA entity inspector.
            zappi_battery_drain_threshold_w: 1000,
            zappi_battery_drain_relax_step_w: 100,
            zappi_battery_drain_kp: 1.0,
            zappi_battery_drain_target_w: 0,
            zappi_battery_drain_hard_clamp_w: 200,
            // PR-ZDP-1: MPPT curtailment probe offset. Default 500 W —
            // enough to push the inverter toward MPP without overshoot.
            // Set to 0 to disable probing (reverts to PR-ZD-3 behaviour).
            zappi_battery_drain_mppt_probe_w: 500,
        }
    }
}

impl Default for Knobs {
    fn default() -> Self {
        Self::safe_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_defaults_match_spec_7() {
        let k = Knobs::safe_defaults();
        // Spot-check the non-trivial values from SPEC §7.
        assert!(!k.force_disable_export);
        assert!((k.export_soc_threshold - 80.0).abs() < f64::EPSILON);
        assert!((k.discharge_soc_target - 80.0).abs() < f64::EPSILON);
        assert!((k.battery_soc_target - 80.0).abs() < f64::EPSILON);
        assert!((k.full_charge_discharge_soc_target - 57.0).abs() < f64::EPSILON);
        assert!((k.full_charge_export_soc_threshold - 100.0).abs() < f64::EPSILON);
        assert_eq!(k.discharge_time, DischargeTime::At0200);
        assert_eq!(k.debug_full_charge, DebugFullCharge::Auto);
        assert_eq!(k.grid_export_limit_w, 5000);
        assert!(!k.allow_battery_to_car);
        assert!((k.eddi_enable_soc - 96.0).abs() < f64::EPSILON);
        assert!((k.eddi_disable_soc - 94.0).abs() < f64::EPSILON);
        assert_eq!(k.eddi_dwell_s, 60);
        // Cold-start safety: observer-mode by default; user/MQTT must
        // explicitly enable writes.
        assert!(!k.writes_enabled);
        // PR-gamma-hold-redesign: planner-driven by default.
        assert_eq!(k.export_soc_threshold_mode, Mode::Weather);
        assert_eq!(k.discharge_soc_target_mode, Mode::Weather);
        assert_eq!(k.battery_soc_target_mode, Mode::Weather);
        assert_eq!(k.disable_night_grid_discharge_mode, Mode::Weather);
        // PR-safe-discharge-enable: legacy 4020 W margin OFF by default
        // (full `inverter_max_discharge_w` discharge); user can flip on
        // affected MultiPlus firmware.
        assert!(!k.inverter_safe_discharge_enable);
        // PR-ZD-2: compensated battery-drain feedback loop knob defaults.
        assert_eq!(k.zappi_battery_drain_threshold_w, 1000);
        assert_eq!(k.zappi_battery_drain_relax_step_w, 100);
        assert!((k.zappi_battery_drain_kp - 1.0).abs() < f64::EPSILON);
        assert_eq!(k.zappi_battery_drain_target_w, 0);
        assert_eq!(k.zappi_battery_drain_hard_clamp_w, 200);
    }

    #[test]
    fn default_equals_safe_defaults() {
        assert_eq!(Knobs::default(), Knobs::safe_defaults());
    }

    /// PR-auto-extended-charge: the cold-start mode is `Auto`. A regression
    /// that flips this to `Disabled` would silently kill the nightly
    /// extended-charge cycle for users who never touch the knob.
    #[test]
    fn auto_extended_default_mode_is_auto() {
        let k = Knobs::safe_defaults();
        assert_eq!(k.charge_car_extended_mode, ExtendedChargeMode::Auto);
    }

    /// PR-ZD-2: dedicated test for the five compensated-drain defaults so
    /// the reviewer can confirm each value without inspecting the larger
    /// `safe_defaults_match_spec_7` test.
    #[test]
    fn safe_defaults_match_spec_zappi_drain() {
        let k = Knobs::safe_defaults();
        assert_eq!(k.zappi_battery_drain_threshold_w, 1000, "threshold_w");
        assert_eq!(k.zappi_battery_drain_relax_step_w, 100, "relax_step_w");
        assert!((k.zappi_battery_drain_kp - 1.0).abs() < f64::EPSILON, "kp");
        assert_eq!(k.zappi_battery_drain_target_w, 0, "target_w");
        assert_eq!(k.zappi_battery_drain_hard_clamp_w, 200, "hard_clamp_w");
        // PR-ZDP-1: probe offset default.
        assert_eq!(k.zappi_battery_drain_mppt_probe_w, 500, "mppt_probe_w");
    }
}
