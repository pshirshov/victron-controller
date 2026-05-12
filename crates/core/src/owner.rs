//! Target ownership. See SPEC §5.4.
//!
//! Every target value (knob, actuated entity) carries an `Owner` recording
//! which subsystem last set it. Ownership drives conflict resolution (e.g.
//! dashboard beats HA within 1 s) and diagnostics.

/// The subsystem that set the current target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Owner {
    /// No target has been set.
    #[default]
    Unset,
    /// Service itself — safety fallback, hard-coded default, kill switch.
    System,
    /// Local web dashboard user action.
    Dashboard,
    /// Home Assistant MQTT command.
    HaMqtt,
    /// Nightly weather-SoC planner (01:55).
    WeatherSocPlanner,
    /// `evaluate_setpoint`.
    SetpointController,
    /// `evaluate_current_limit`.
    CurrentLimitController,
    /// `evaluate_schedules`.
    ScheduleController,
    /// `evaluate_zappi_mode`.
    ZappiController,
    /// `evaluate_eddi_mode`.
    EddiController,
    /// Sunday 17:00 full-charge rollover.
    FullChargeScheduler,
    /// PR-keep-batteries-charged: daytime ESS-state override
    /// (`evaluate_ess_state_override`).
    EssStateOverrideController,
    /// PR-LG-THINQ-B: HeatPumpControl core (evaluate_heat_pump).
    HeatPumpController,
}
