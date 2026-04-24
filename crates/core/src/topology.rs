//! Immutable structural metadata — loaded once at startup from config,
//! never mutated. See SPEC §2.3.6.

use std::time::Duration;

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
    pub freshness_local_dbus: Duration,
    pub freshness_myenergi: Duration,
    pub freshness_outdoor_temperature: Duration,
}

impl ControllerParams {
    /// Defaults per SPEC §5.3 with the user's G3 overrides:
    /// - local D-Bus values: 15 s (paired with 5 s `DBUS_POLL_PERIOD`
    ///   in the shell subscriber → three polls per deadline; PR-URGENT-20
    ///   widened the poll cadence to stop hammering the Venus D-Bus
    ///   broker, and the staleness window must stay strictly greater
    ///   than the poll cadence or sensors would be perpetually Stale).
    /// - myenergi (Zappi/Eddi): 5 min
    /// - outdoor temperature: 40 min (Open-Meteo fetched every 30 min;
    ///   temperature changes slowly, so a 10-min grace window keeps
    ///   `weather_soc` happy across a single missed fetch).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            setpoint_confirm_tolerance_w: 50,
            setpoint_retarget_deadband_w: 25,
            current_limit_confirm_tolerance_a: 0.5,
            current_limit_retarget_deadband_a: 0.5,
            freshness_local_dbus: Duration::from_secs(15),
            freshness_myenergi: Duration::from_secs(300),
            freshness_outdoor_temperature: Duration::from_secs(40 * 60),
        }
    }
}

impl Default for ControllerParams {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Topology = structural configuration (service instance IDs, MQTT
/// broker, controller tunables). Built once at startup; immutable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Topology {
    pub controller_params: ControllerParams,
}

impl Topology {
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            controller_params: ControllerParams::defaults(),
        }
    }
}

impl Default for Topology {
    fn default() -> Self {
        Self::defaults()
    }
}
