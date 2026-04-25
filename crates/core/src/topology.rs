//! Immutable structural metadata — loaded once at startup from config,
//! never mutated. See SPEC §2.3.6.

use std::time::Duration;

use crate::tz::TzHandle;

/// Tunables for the controller dispatch layer. These are separate from
/// the user-facing [`crate::knobs::Knobs`] — they don't belong on the
/// dashboard and rarely change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControllerParams {
    /// GridSetpoint: how close actual must be to target to confirm.
    pub setpoint_confirm_tolerance_w: i32,
    /// GridSetpoint: minimum delta that restarts the phase cycle.
    pub setpoint_retarget_deadband_w: i32,
    /// InputCurrentLimit: confirm tolerance.
    pub current_limit_confirm_tolerance_a: f64,
    /// InputCurrentLimit: retarget dead-band.
    pub current_limit_retarget_deadband_a: f64,

    // Freshness thresholds — see SPEC §5.3.
    //
    // D-Bus sensor and actuated readback freshness is per-id (see
    // `SensorId::freshness_threshold` / `ActuatedId::freshness_threshold`,
    // authoritative per
    // `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md`).
    // Only myenergi (single-cadence poller) is kept here as a scalar.
    pub freshness_myenergi: Duration,

    /// Per-provider forecast snapshot freshness. A snapshot older than
    /// this is excluded from the weather-SoC fusion (A-16). 12 h is
    /// generous — Solcast fetches hourly, Forecast.Solar and Open-Meteo
    /// every 30 min; 12 h is 24+ missed fetches. The goal is to catch
    /// long-running failures (API-key expiry, network partition) without
    /// tripping on routine intermittent outages.
    pub freshness_forecast: Duration,
}

impl ControllerParams {
    /// Defaults per SPEC §5.3 with the user's G3 overrides:
    /// - myenergi (Zappi/Eddi): 5 min
    /// - D-Bus sensors and readbacks use per-id thresholds instead of a
    ///   single scalar — see `SensorId::freshness_threshold` and
    ///   `ActuatedId::freshness_threshold`.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            setpoint_confirm_tolerance_w: 50,
            setpoint_retarget_deadband_w: 25,
            current_limit_confirm_tolerance_a: 0.5,
            current_limit_retarget_deadband_a: 0.5,
            freshness_myenergi: Duration::from_secs(300),
            freshness_forecast: Duration::from_secs(12 * 60 * 60),
        }
    }
}

impl Default for ControllerParams {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Deploy-time hardware constants. Promoted from per-controller
/// `const`s into config so a different physical install (different
/// inverter, breaker rating, grid voltage band, MPPT capability) can
/// override them without recompiling the core.
///
/// These are NOT user-tunable knobs (they don't appear on the
/// dashboard, are not subject to MQTT retain/replay, and should not
/// change at runtime). See `SPEC.md §7` for the runtime knob inventory.
///
/// Sign convention (matches the controller-side usage):
/// - `inverter_max_discharge_w` is stored as a **negative** f64 — that's
///   the floor used in `prepare_setpoint(max_discharge, …)`. The shell-
///   side `HardwareConfig` exposes a positive magnitude; the
///   `From<HardwareConfig>` impl flips the sign at the boundary.
/// - All other power values are positive magnitudes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HardwareParams {
    /// MultiPlus AC-export ceiling (negative; floor for setpoint).
    pub inverter_max_discharge_w: f64,
    /// Margin below the inverter "forced grid charge above ~4.8 kW"
    /// glitch — used by setpoint's `max_discharge` formula.
    pub inverter_safe_discharge_w: f64,
    /// Main breaker rating ceiling (A) for current-limit controller.
    pub max_grid_current_a: f64,
    /// Floor — keeps inverter aux fed.
    pub min_system_current_a: f64,
    /// Forced-import baseline (Soltaro 23:55 quirk).
    pub idle_setpoint_w: f64,
    /// Evening planner: `preserve_battery` baseload threshold.
    pub baseload_consumption_w: f64,
    /// Caps `grid_export_limit_w` knob (W).
    pub grid_export_knob_max_w: u32,
    /// Caps `grid_import_limit_w` knob (W).
    pub grid_import_knob_max_w: u32,
    /// Pylontech 48 V stack — capacity model nominal voltage.
    pub battery_nominal_voltage_v: f64,
    /// EN 50160 nominal grid voltage.
    pub grid_nominal_voltage_v: f64,
    /// EN 50160 -10% — sanity floor.
    pub grid_min_sensible_voltage_v: f64,
    /// EN 50160 +10% + ~7 V noise headroom — sanity ceiling.
    pub grid_max_sensible_voltage_v: f64,
}

impl HardwareParams {
    /// Defaults matching the legacy hard-coded values in
    /// `controllers::setpoint` and `controllers::current_limit`.
    /// `inverter_max_discharge_w` is stored as a **negative** number
    /// here (the value the controller subtracts from); the shell-side
    /// `HardwareConfig` carries the positive magnitude.
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            inverter_max_discharge_w: -5000.0,
            inverter_safe_discharge_w: 4020.0,
            max_grid_current_a: 65.0,
            min_system_current_a: 10.0,
            idle_setpoint_w: 10.0,
            baseload_consumption_w: 1200.0,
            grid_export_knob_max_w: 6000,
            grid_import_knob_max_w: 13000,
            battery_nominal_voltage_v: 48.0,
            grid_nominal_voltage_v: 230.0,
            grid_min_sensible_voltage_v: 207.0,
            grid_max_sensible_voltage_v: 260.0,
        }
    }
}

impl Default for HardwareParams {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Topology = structural configuration (service instance IDs, MQTT
/// broker, controller tunables). Built once at startup; immutable in
/// the sense that no controller mutates it — but the embedded
/// [`TzHandle`] is itself a shared atomic cell that the shell's D-Bus
/// timezone subscriber writes to.
///
/// PR-tz-from-victron: `tz_handle` was added; the struct is no longer
/// `Copy` because `TzHandle` holds an `Arc`. Pass by reference (already
/// the convention) or `clone()` at the boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct Topology {
    pub controller_params: ControllerParams,
    pub hardware: HardwareParams,
    /// PR-tz-from-victron: the live timezone fed from D-Bus
    /// `/Settings/System/TimeZone`. Cloned across threads cheaply via
    /// the inner Arc; `apply_event(Event::Timezone, ...)` writes to it.
    pub tz_handle: TzHandle,
}

impl Topology {
    /// Default topology with all-defaults plus a fresh UTC `TzHandle`.
    /// Not `const fn` because `TzHandle::new_utc` allocates an Arc.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            controller_params: ControllerParams::defaults(),
            hardware: HardwareParams::defaults(),
            tz_handle: TzHandle::new_utc(),
        }
    }

    /// Builder helper: replace only the hardware section, keep all
    /// other defaults.
    #[must_use]
    pub fn with_hardware(hardware: HardwareParams) -> Self {
        Self {
            controller_params: ControllerParams::defaults(),
            hardware,
            tz_handle: TzHandle::new_utc(),
        }
    }
}

impl Default for Topology {
    fn default() -> Self {
        Self::defaults()
    }
}
