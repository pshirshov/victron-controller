//! Top-level typed Events and Effects the pure `process()` consumes and
//! produces. See SPEC §5.5.
//!
//! All IDs are closed enums — the shell is responsible for parsing wire
//! formats (D-Bus / MQTT / HTTP) into these typed variants before calling
//! `process`. Correspondingly, all `Effect`s are typed; the shell
//! serialises them back to wire format when executing.

use crate::Freshness;
use crate::controllers::schedules::ScheduleSpec;
use crate::knobs::{
    ChargeBatteryExtendedMode, DebugFullCharge, DischargeTime, ExtendedChargeMode,
    ForecastDisagreementStrategy, Mode,
};
use crate::myenergi::{EddiMode, ZappiMode, ZappiState};
use crate::owner::Owner;
use std::time::Instant;

/// Human-readable explanation of a controller's decision — one-line
/// summary plus the key factors that drove it. Every controller
/// produces one of these every time it evaluates, even when the
/// output is "no change". Published in the world snapshot so the
/// dashboard/HA surface can always show WHY a target has its current
/// value, not just WHAT the value is.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub summary: String,
    pub factors: Vec<DecisionFactor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionFactor {
    pub name: String,
    pub value: String,
}

impl Decision {
    #[must_use]
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            factors: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_factor(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.factors.push(DecisionFactor {
            name: name.into(),
            value: value.into(),
        });
        self
    }
}

// =============================================================================
// IDs
// =============================================================================

/// Scalar sensor identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorId {
    BatterySoc,
    BatterySoh,
    BatteryInstalledCapacity,
    BatteryDcPower,
    MpptPower0,
    MpptPower1,
    SoltaroPower,
    PowerConsumption,
    GridPower,
    GridVoltage,
    GridCurrent,
    ConsumptionCurrent,
    OffgridPower,
    OffgridCurrent,
    VebusInputCurrent,
    EvchargerAcPower,
    EvchargerAcCurrent,
    EssState,
    OutdoorTemperature,
    /// Cumulative energy delivered to the EV in the current Zappi
    /// session (kWh). Sourced from the myenergi cloud poll's `che`
    /// field; resets when a session ends. Reseed-driven (cadence
    /// 300 s, owned by the myenergi poller, not the D-Bus subscriber).
    SessionKwh,
    // PR-actuated-as-sensors (PR-AS-A): D-Bus paths that mirror an
    // actuated entity. Treated as scalar sensors for cadence /
    // freshness purposes. Storage of truth lives on
    // `world.<entity>.actual` (driven via the `actuated_id()` post-
    // hook in `apply_sensor_reading`); these variants exist so the
    // D-Bus subscriber can route their paths through the unified
    // sensor pipeline.
    GridSetpointActual,
    InputCurrentLimitActual,
    Schedule0StartActual,
    Schedule0DurationActual,
    Schedule0SocActual,
    Schedule0DaysActual,
    Schedule0AllowDischargeActual,
    Schedule1StartActual,
    Schedule1DurationActual,
    Schedule1SocActual,
    Schedule1DaysActual,
    Schedule1AllowDischargeActual,
    /// PR-ev-soc-sensor: EV state-of-charge percentage sourced from an
    /// external MQTT publisher (saic-python-mqtt-gateway today). Pushed
    /// in by the MQTT subscriber after parsing the publisher's HA-
    /// discovery `state_topic`; the value is opaque to the controllers
    /// — surfaced on the dashboard sensor table only.
    EvSoc,
    /// PR-auto-extended-charge: EV configured charge-target percentage
    /// sourced from the same external publisher as `EvSoc`. Read by the
    /// 04:30 auto-extended-charge evaluation in `Auto` mode.
    EvChargeTarget,
    /// PR-ZD-1: AC power draw of the metered heat pump (W). Sourced from
    /// zigbee2mqtt push (nodon-mtr-heat-pump), JSON `.power` field.
    /// Observability + compensated-drain feedback (PR-ZD-3).
    HeatPumpPower,
    /// PR-ZD-1: AC power draw of the metered cooker/stove (W). Sourced from
    /// zigbee2mqtt push (nodon-mtr-stove), JSON `.power` field. Same shape
    /// as `HeatPumpPower`.
    CookerPower,
    /// PR-ZD-1: Operation mode of MPPT charger 0 (ttyUSB1, DI 289).
    /// 0=Off, 1=Voltage-or-current-limited, 2=MPPT-tracking.
    /// Observability only — not coupled into the control loop.
    Mppt0OperationMode,
    /// PR-ZD-1: Operation mode of MPPT charger 1 (ttyS2, DI 274).
    /// 0=Off, 1=Voltage-or-current-limited, 2=MPPT-tracking.
    /// Observability only — not coupled into the control loop.
    Mppt1OperationMode,
    // PR-LG-THINQ-B: actuated-mirror sensors for the 4 LG heat-pump slots,
    // plus 2 read-only temperature sensors. Storage of truth lives on
    // `world.lg_*.actual`; freshness regime = ReseedDriven at 60 s cadence;
    // staleness = 180 s (>= 2× cadence + headroom). NOT external-polled.
    /// Bool readback mirror of `ActuatedId::LgHeatPumpPower`.
    LgHeatPumpPowerActual,
    /// Bool readback mirror of `ActuatedId::LgDhwPower`.
    LgDhwPowerActual,
    /// i32 readback mirror of `ActuatedId::LgHeatingWaterTarget`.
    LgHeatingWaterTargetActual,
    /// i32 readback mirror of `ActuatedId::LgDhwTarget`.
    LgDhwTargetActual,
    /// Current DHW water temperature (°C). Read-only; sourced from
    /// the LG ThinQ cloud poller's `HeatPumpState.dhw_current_c`.
    LgDhwCurrentTemperatureC,
    /// Current heating water temperature (°C). Read-only; sourced from
    /// the LG ThinQ cloud poller's `HeatPumpState.heating_water_current_c`.
    LgHeatingWaterCurrentTemperatureC,
}

impl SensorId {
    /// Per-sensor Fresh→Stale threshold.
    ///
    /// Values are authoritative per
    /// `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md` and
    /// must only change via PR review. Universal invariant
    /// (PR-cadence-per-sensor): `staleness >= 2 * reseed_cadence` for
    /// every non-external-polled sensor; external-polled sensors
    /// (Open-Meteo, myenergi cloud) use the `cadence + slack` grace-
    /// window model — see [`is_external_polled`].
    #[must_use]
    pub const fn freshness_threshold(self) -> std::time::Duration {
        use std::time::Duration;
        match self {
            // Fast-organic sensors: 5 s reseed → 15 s staleness (=2×).
            // MPPTs join this group per user observation: PV power is
            // sub-second when sun is up, silent at night.
            Self::PowerConsumption
            | Self::ConsumptionCurrent
            | Self::GridPower
            | Self::GridVoltage
            | Self::GridCurrent
            | Self::BatteryDcPower
            | Self::SoltaroPower
            | Self::OffgridPower
            | Self::OffgridCurrent
            | Self::VebusInputCurrent
            | Self::EvchargerAcPower
            | Self::EvchargerAcCurrent
            | Self::MpptPower0
            | Self::MpptPower1 => Duration::from_secs(15),
            // Slow-signalled: Pylontech emits SoC at ~1 Hz while changing,
            // seconds-to-minutes idle. 60 s reseed → 120 s staleness.
            Self::BatterySoc => Duration::from_secs(120),
            // Reseed-driven slow metrics: value only moves on minutes-
            // to-hours timescales; staleness = 2× reseed cadence.
            Self::BatterySoh | Self::EssState => Duration::from_secs(900),
            // Essentially static — reseed every minute (alongside the
            // rest of the battery service); staleness 2× plus headroom.
            Self::BatteryInstalledCapacity => Duration::from_secs(3600),
            // Outdoor temperature comes from Open-Meteo (30 min cadence);
            // give a 10 min grace window on top.
            Self::OutdoorTemperature => Duration::from_secs(40 * 60),
            // Zappi session kWh comes from the myenergi cloud poll
            // (default 300 s); 600 s = 2 × cadence per the reseed-driven
            // staleness rule.
            Self::SessionKwh => Duration::from_secs(600),
            // PR-actuated-as-sensors (PR-AS-A): grid setpoint &
            // current-limit readback paths reseed every 5 s alongside
            // their fast-organic neighbours on the same service.
            Self::GridSetpointActual | Self::InputCurrentLimitActual => {
                Duration::from_secs(15)
            }
            // PR-actuated-as-sensors (PR-AS-A): schedule leaf paths
            // reseed at 60 s; staleness 180 s satisfies 2× cadence.
            // PR-LG-THINQ-B: LG sensors polled at 60 s → same 180 s window.
            Self::Schedule0StartActual
            | Self::Schedule0DurationActual
            | Self::Schedule0SocActual
            | Self::Schedule0DaysActual
            | Self::Schedule0AllowDischargeActual
            | Self::Schedule1StartActual
            | Self::Schedule1DurationActual
            | Self::Schedule1SocActual
            | Self::Schedule1DaysActual
            | Self::Schedule1AllowDischargeActual
            | Self::LgHeatPumpPowerActual
            | Self::LgDhwPowerActual
            | Self::LgHeatingWaterTargetActual
            | Self::LgDhwTargetActual
            | Self::LgDhwCurrentTemperatureC
            | Self::LgHeatingWaterCurrentTemperatureC => Duration::from_secs(180),
            // PR-ev-soc-sensor: external push from saic-python-mqtt-gateway.
            // The car can sleep for hours between reports; 12 h is a
            // generous Fresh window that still flips Stale if the gateway
            // dies. Marked as external-polled so the staleness invariant
            // uses the grace-window rule (cadence + slack).
            //
            // PR-auto-extended-charge: `EvChargeTarget` shares the same
            // gateway and the same 60 min cadence; identical 12 h window.
            Self::EvSoc | Self::EvChargeTarget => Duration::from_secs(12 * 3600),
            // PR-ZD-1: 30 s freshness for all four new sensors.
            // HP/cooker are zigbee2mqtt push (15 s reseed → 2× = 30 s).
            // MPPT op-modes are D-Bus reseed-driven (15 s reseed → 30 s).
            Self::HeatPumpPower
            | Self::CookerPower
            | Self::Mppt0OperationMode
            | Self::Mppt1OperationMode => Duration::from_secs(30),
        }
    }
}

/// Classification of a sensor's freshness regime.
///
/// Drives the staleness invariant — see
/// `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md` and
/// PR-cadence-per-sensor: every non-external-polled sensor satisfies
/// `staleness ≥ 2 × reseed_cadence`, regardless of regime. Regime is
/// retained as a documentation aid (which mechanism — organic signals
/// or reseed — is the *primary* freshness driver) but no longer carves
/// out a different staleness rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessRegime {
    /// Organic signals fire on change. Either at ≥ 1 Hz when the value
    /// is moving (former `Fast` sensors, now reseeded at 5 s so the
    /// universal rule yields a 15 s staleness floor) or with multi-
    /// second/minute inter-change gaps. Either way: organic plus reseed.
    SlowSignalled,
    /// Organic signals essentially never fire; reseed IS the freshness
    /// source.
    ReseedDriven,
}

impl SensorId {
    /// Every variant of this enum, for invariant checks.
    ///
    /// Updated by hand when a new sensor lands. The matching
    /// `regime` / `reseed_cadence` arms in the same impl will refuse to
    /// compile if a variant is added without classifying it (explicit
    /// `match` in those methods, no `_ =>` arm).
    pub const ALL: &'static [SensorId] = &[
        SensorId::BatterySoc,
        SensorId::BatterySoh,
        SensorId::BatteryInstalledCapacity,
        SensorId::BatteryDcPower,
        SensorId::MpptPower0,
        SensorId::MpptPower1,
        SensorId::SoltaroPower,
        SensorId::PowerConsumption,
        SensorId::GridPower,
        SensorId::GridVoltage,
        SensorId::GridCurrent,
        SensorId::ConsumptionCurrent,
        SensorId::OffgridPower,
        SensorId::OffgridCurrent,
        SensorId::VebusInputCurrent,
        SensorId::EvchargerAcPower,
        SensorId::EvchargerAcCurrent,
        SensorId::EssState,
        SensorId::OutdoorTemperature,
        SensorId::SessionKwh,
        // PR-actuated-as-sensors (PR-AS-A).
        SensorId::GridSetpointActual,
        SensorId::InputCurrentLimitActual,
        SensorId::Schedule0StartActual,
        SensorId::Schedule0DurationActual,
        SensorId::Schedule0SocActual,
        SensorId::Schedule0DaysActual,
        SensorId::Schedule0AllowDischargeActual,
        SensorId::Schedule1StartActual,
        SensorId::Schedule1DurationActual,
        SensorId::Schedule1SocActual,
        SensorId::Schedule1DaysActual,
        SensorId::Schedule1AllowDischargeActual,
        // PR-ev-soc-sensor.
        SensorId::EvSoc,
        // PR-auto-extended-charge.
        SensorId::EvChargeTarget,
        // PR-ZD-1.
        SensorId::HeatPumpPower,
        SensorId::CookerPower,
        SensorId::Mppt0OperationMode,
        SensorId::Mppt1OperationMode,
        // PR-LG-THINQ-B: 4 actuated-mirror + 2 plain temperature sensors.
        SensorId::LgHeatPumpPowerActual,
        SensorId::LgDhwPowerActual,
        SensorId::LgHeatingWaterTargetActual,
        SensorId::LgDhwTargetActual,
        SensorId::LgDhwCurrentTemperatureC,
        SensorId::LgHeatingWaterCurrentTemperatureC,
    ];

    /// Freshness regime for this sensor — see [`FreshnessRegime`].
    ///
    /// Authority: the audit table in
    /// `docs/drafts/20260425-1103-pr-cadence-per-sensor-plan.md` §3.
    /// Explicit per-variant match (no `_` arm) so adding a new variant
    /// forces an explicit classification call at compile time.
    ///
    /// PR-cadence-per-sensor: `FreshnessRegime::Fast` was deleted; the
    /// universal `staleness ≥ 2 × reseed_cadence` rule covers what the
    /// Fast carve-out used to handle, now that fast-organic services
    /// reseed at 5 s.
    #[must_use]
    pub const fn regime(self) -> FreshnessRegime {
        match self {
            // Slow-signalled — organic signals plus a meaningful reseed.
            // Includes the previously-Fast sub-second sensors (now 5 s
            // reseed → 15 s staleness via the universal rule) as well as
            // BatterySoc (~1 Hz changing, idle gaps in the minutes) and
            // the MPPTs (sub-second sunny, silent at night when PV=0).
            Self::PowerConsumption
            | Self::ConsumptionCurrent
            | Self::GridPower
            | Self::GridCurrent
            | Self::GridVoltage
            | Self::BatteryDcPower
            | Self::SoltaroPower
            | Self::MpptPower0
            | Self::MpptPower1
            | Self::OffgridPower
            | Self::OffgridCurrent
            | Self::VebusInputCurrent
            | Self::EvchargerAcPower
            | Self::EvchargerAcCurrent
            | Self::BatterySoc
            // PR-actuated-as-sensors (PR-AS-A): grid setpoint and
            // current-limit readback paths follow their fast-organic
            // neighbours on the same service.
            | Self::GridSetpointActual
            | Self::InputCurrentLimitActual
            // PR-ZD-1: HP/cooker are zigbee2mqtt push-on-change (organic
            // signal from the appliance meter plus 15 s reseed safety net).
            | Self::HeatPumpPower
            | Self::CookerPower => FreshnessRegime::SlowSignalled,
            // Reseed-driven — value moves on minutes-to-hours timescales,
            // organic signals essentially never fire.
            // `SessionKwh` is sourced from the myenergi cloud poll, not
            // the D-Bus subscriber; the regime is the same — reseed IS
            // the freshness source — but the cadence comes from a
            // separate constant (see `reseed_cadence`).
            Self::BatterySoh
            | Self::BatteryInstalledCapacity
            | Self::EssState
            | Self::OutdoorTemperature
            | Self::SessionKwh
            // PR-actuated-as-sensors (PR-AS-A): schedule leaf paths —
            // value moves only on a settings write (reseed-driven).
            | Self::Schedule0StartActual
            | Self::Schedule0DurationActual
            | Self::Schedule0SocActual
            | Self::Schedule0DaysActual
            | Self::Schedule0AllowDischargeActual
            | Self::Schedule1StartActual
            | Self::Schedule1DurationActual
            | Self::Schedule1SocActual
            | Self::Schedule1DaysActual
            | Self::Schedule1AllowDischargeActual
            // PR-ev-soc-sensor: external MQTT push (saic-python-mqtt-
            // gateway). No organic D-Bus signal; the gateway pushes when
            // the car reports a new SoC.
            | Self::EvSoc
            // PR-auto-extended-charge: same gateway as `EvSoc`.
            | Self::EvChargeTarget
            // PR-ZD-1: MPPT op-modes are reseed-driven (value changes
            // only on inverter mode transitions; no organic signal).
            | Self::Mppt0OperationMode
            | Self::Mppt1OperationMode
            // PR-LG-THINQ-B: all 6 new LG sensors are reseed-driven (60 s
            // LG cloud poll is the sole freshness source).
            | Self::LgHeatPumpPowerActual
            | Self::LgDhwPowerActual
            | Self::LgHeatingWaterTargetActual
            | Self::LgDhwTargetActual
            | Self::LgDhwCurrentTemperatureC
            | Self::LgHeatingWaterCurrentTemperatureC => FreshnessRegime::ReseedDriven,
        }
    }

    /// Reseed cadence for this sensor.
    ///
    /// Authoritative per the audit table in
    /// `docs/drafts/20260425-1103-pr-cadence-per-sensor-plan.md` §3.
    /// Per-sensor (not per-service) so the shell can compute each
    /// service's poll cadence as `min(reseed_cadence)` over its sensors.
    ///
    /// Hard-coded here so the core crate doesn't pull a dependency on
    /// the shell crate just to validate its own invariants.
    // The arms below intentionally pair semantically distinct
    // cadences (D-Bus settings reseed vs myenergi cloud poll) that
    // happen to share a numeric value today. Keep them separate so a
    // future change to either source doesn't accidentally rewrite the
    // other.
    #[allow(clippy::match_same_arms)]
    #[must_use]
    pub const fn reseed_cadence(self) -> std::time::Duration {
        use std::time::Duration;
        match self {
            // Fast-organic sensors: 5 s reseed safety net under the
            // universal rule. Drives the per-service min cadence in the
            // D-Bus subscriber for every service that hosts one of them.
            Self::BatteryDcPower
            | Self::SoltaroPower
            | Self::PowerConsumption
            | Self::ConsumptionCurrent
            | Self::GridPower
            | Self::GridVoltage
            | Self::GridCurrent
            | Self::OffgridPower
            | Self::OffgridCurrent
            | Self::VebusInputCurrent
            | Self::EvchargerAcPower
            | Self::EvchargerAcCurrent
            // MPPTs: sub-second when PV is flowing, silent at night.
            // 5 s reseed amortises across the silent service alongside
            // the other fast-organic sensors.
            | Self::MpptPower0
            | Self::MpptPower1
            // PR-actuated-as-sensors (PR-AS-A): grid setpoint &
            // current-limit readback paths share the fast-organic
            // 5 s reseed cadence on their respective services.
            | Self::GridSetpointActual
            | Self::InputCurrentLimitActual => Duration::from_secs(5),
            // Slow-signalled / reseed-driven on the battery service: 60 s.
            Self::BatterySoc
            | Self::BatterySoh
            | Self::BatteryInstalledCapacity => Duration::from_secs(60),
            // Settings service — reseed-driven, very rare changes.
            Self::EssState => Duration::from_secs(300),
            // Open-Meteo poll cadence (30 min).
            Self::OutdoorTemperature => Duration::from_secs(30 * 60),
            // myenergi default poll cadence (5 min). Owned by
            // `crates/shell/src/myenergi/mod.rs`'s Poller, not the
            // D-Bus subscriber; the constant is duplicated here only
            // so the core can validate its own staleness invariant
            // without taking a shell-crate dependency.
            Self::SessionKwh => Duration::from_secs(300),
            // PR-actuated-as-sensors (PR-AS-A): schedule leaf paths
            // reseed every 60 s on the settings service.
            Self::Schedule0StartActual
            | Self::Schedule0DurationActual
            | Self::Schedule0SocActual
            | Self::Schedule0DaysActual
            | Self::Schedule0AllowDischargeActual
            | Self::Schedule1StartActual
            | Self::Schedule1DurationActual
            | Self::Schedule1SocActual
            | Self::Schedule1DaysActual
            | Self::Schedule1AllowDischargeActual => Duration::from_secs(60),
            // PR-ev-soc-sensor / PR-auto-extended-charge: 10 min cadence.
            // Reading from MQTT is cheap (no API call on our side; the
            // gateway publishes when it polls). Earlier this was 60 min
            // — operator preference is a tighter "expected freshness"
            // signal so the dashboard shows Stale promptly when the
            // gateway stops publishing. The 12 h staleness threshold
            // (below) stays generous because real gateway gaps (car
            // asleep) span hours.
            Self::EvSoc | Self::EvChargeTarget => Duration::from_secs(10 * 60),
            // PR-ZD-1: 15 s for all four new sensors.
            // HP/cooker: zigbee2mqtt push — 15 s reseed gives a 30 s
            // staleness floor (satisfies 2× cadence).
            // MPPT op-modes: D-Bus reseed — 15 s cadence on the same
            // solarcharger service, no impact on per-service min (5 s
            // already wins via MpptPower0/1).
            Self::HeatPumpPower
            | Self::CookerPower
            | Self::Mppt0OperationMode
            | Self::Mppt1OperationMode => Duration::from_secs(15),
            // PR-LG-THINQ-B: 60 s LG cloud poll cadence (2× = 120 s ≤ 180 s threshold).
            Self::LgHeatPumpPowerActual
            | Self::LgDhwPowerActual
            | Self::LgHeatingWaterTargetActual
            | Self::LgDhwTargetActual
            | Self::LgDhwCurrentTemperatureC
            | Self::LgHeatingWaterCurrentTemperatureC => Duration::from_secs(60),
        }
    }

    /// PR-actuated-as-sensors: the actuated entity this sensor
    /// reading mirrors, if any. Returns `None` for plain (non-mirror)
    /// sensors.
    ///
    /// Used in two places:
    /// 1. The post-update hook in `apply_sensor_reading` drives
    ///    `confirm_if(...)` on the matching `world.<entity>.actual`
    ///    slot for scalar mirrors (`GridSetpoint`/`InputCurrentLimit`).
    ///    Schedule leaves are handled separately by
    ///    `Event::ScheduleReadback` (the rolled-up accumulator emits
    ///    a complete `ScheduleSpec`); the post-hook short-circuits
    ///    them via `debug_assert!`.
    /// 2. PR-AS-C: `SensorBroadcastCore` filters out actuated-mirror
    ///    variants from the sensor-publish iteration via
    ///    `actuated_id().is_some()` — their values are surfaced via
    ///    the dedicated `Actuated` table instead.
    ///
    /// Explicit per-variant match (no `_ =>` arm) so a future
    /// `SensorId` addition forces classification.
    #[must_use]
    pub const fn actuated_id(self) -> Option<ActuatedId> {
        match self {
            Self::GridSetpointActual => Some(ActuatedId::GridSetpoint),
            Self::InputCurrentLimitActual => Some(ActuatedId::InputCurrentLimit),
            Self::Schedule0StartActual
            | Self::Schedule0DurationActual
            | Self::Schedule0SocActual
            | Self::Schedule0DaysActual
            | Self::Schedule0AllowDischargeActual => Some(ActuatedId::Schedule0),
            Self::Schedule1StartActual
            | Self::Schedule1DurationActual
            | Self::Schedule1SocActual
            | Self::Schedule1DaysActual
            | Self::Schedule1AllowDischargeActual => Some(ActuatedId::Schedule1),
            // PR-keep-batteries-charged: `EssState` keeps the primary-
            // sensor classification (sensor table + HA sensor entity);
            // `apply_sensor_reading` *also* feeds it into
            // `world.ess_state_target.actual` directly so the daytime
            // override has TASS phase tracking. Returning `None` here
            // avoids the `actuated_id().is_some()` filter in
            // `SensorBroadcastCore` swallowing the sensor publish.
            Self::EssState
            | Self::BatterySoc
            | Self::BatterySoh
            | Self::BatteryInstalledCapacity
            | Self::BatteryDcPower
            | Self::MpptPower0
            | Self::MpptPower1
            | Self::SoltaroPower
            | Self::PowerConsumption
            | Self::GridPower
            | Self::GridVoltage
            | Self::GridCurrent
            | Self::ConsumptionCurrent
            | Self::OffgridPower
            | Self::OffgridCurrent
            | Self::VebusInputCurrent
            | Self::EvchargerAcPower
            | Self::EvchargerAcCurrent
            | Self::OutdoorTemperature
            | Self::SessionKwh
            // PR-ev-soc-sensor.
            | Self::EvSoc
            // PR-auto-extended-charge.
            | Self::EvChargeTarget
            // PR-ZD-1: plain sensors, not actuated mirrors.
            | Self::HeatPumpPower
            | Self::CookerPower
            | Self::Mppt0OperationMode
            | Self::Mppt1OperationMode
            // PR-LG-THINQ-B: 2 plain temperature sensors (not actuated mirrors).
            | Self::LgDhwCurrentTemperatureC
            | Self::LgHeatingWaterCurrentTemperatureC => None,
            // PR-LG-THINQ-B: 4 actuated-mirror sensors route to their ActuatedId.
            Self::LgHeatPumpPowerActual => Some(ActuatedId::LgHeatPumpPower),
            Self::LgDhwPowerActual => Some(ActuatedId::LgDhwPower),
            Self::LgHeatingWaterTargetActual => Some(ActuatedId::LgHeatingWaterTarget),
            Self::LgDhwTargetActual => Some(ActuatedId::LgDhwTarget),
        }
    }
}

/// Sensors whose cadence is NOT governed by the D-Bus subscriber
/// reseed schedule and which therefore use a grace-window freshness
/// model (`staleness = cadence + slack`) rather than the strict
/// `2× cadence` headroom. Open-Meteo (30 min) and myenergi (5 min)
/// polls are owned by their respective external services.
#[must_use]
const fn is_external_polled(id: SensorId) -> bool {
    matches!(
        id,
        SensorId::OutdoorTemperature
            | SensorId::SessionKwh
            // PR-ev-soc-sensor: external MQTT push, gateway-paced.
            | SensorId::EvSoc
            // PR-auto-extended-charge: same gateway as `EvSoc`.
            | SensorId::EvChargeTarget
    )
}

/// Verify the per-sensor staleness invariant for one sensor. Returns
/// `Err(message)` describing the violation if the invariant fails.
///
/// Universal rule (PR-cadence-per-sensor): every non-external-polled
/// sensor must satisfy `staleness ≥ 2 × reseed_cadence`. External-
/// polled sensors use the grace-window model `staleness ≥ cadence + 1 s`.
///
/// Used by both the `freshness_threshold_invariant_holds_for_every_sensor`
/// unit test and by `Runtime::new` as a startup belt-and-braces check.
#[allow(clippy::missing_errors_doc)]
pub fn check_staleness_invariant(id: SensorId) -> Result<(), String> {
    let staleness = id.freshness_threshold();
    let cadence = id.reseed_cadence();
    let required = if is_external_polled(id) {
        cadence + std::time::Duration::from_secs(1)
    } else {
        2 * cadence
    };
    if staleness < required {
        return Err(format!(
            "staleness invariant violated: SensorId::{id:?} \
             staleness={}s < required={}s; \
             fix freshness_threshold or reseed cadence",
            staleness.as_secs(),
            required.as_secs()
        ));
    }
    Ok(())
}

// =============================================================================
// PR-pinned-registers — typed value + per-register state
// =============================================================================

/// Typed value for a pinned-register comparison. Mirrors
/// `shell::config::PinnedValue` exactly; duplicated here so the core
/// crate stays free of a shell dependency. The shell-side validated
/// value is converted into this shape once at startup when seeding
/// `world.pinned_registers`, and again when emitting
/// `Event::PinnedRegisterReading`s and `Effect::WriteDbusPinned`s.
#[derive(Debug, Clone, PartialEq)]
pub enum PinnedValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl PinnedValue {
    /// Tolerant equality used to decide whether the bus-observed value
    /// matches the configured target.
    ///
    /// - Floats: equal iff both finite and within
    ///   `max(1e-6, 1e-6 * max(|a|, |b|))`. NaN never equals anything.
    /// - Bool ↔ Int(0/1): Victron's settings service returns ints over
    ///   D-Bus where the user wrote a Python boolean; treat 0/1 as
    ///   equivalent to `false`/`true`.
    /// - Otherwise: same-variant strict equality.
    #[must_use]
    pub fn approx_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => float_close(*a, *b),
            (Self::String(a), Self::String(b)) => a == b,
            // Bool ↔ Int(0/1) — Victron returns int over the wire.
            (Self::Bool(b), Self::Int(n)) | (Self::Int(n), Self::Bool(b)) => {
                (*b && *n == 1) || (!*b && *n == 0)
            }
            _ => false,
        }
    }
}

#[must_use]
fn float_close(a: f64, b: f64) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= 1e-6 * scale
}

impl std::fmt::Display for PinnedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(x) => write!(f, "{x}"),
            Self::String(s) => write!(f, "\"{s}\""),
        }
    }
}

/// Confirmation status for a pinned register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinnedStatus {
    /// No reading has landed since boot.
    Unknown,
    /// Most recent reading matched the configured target.
    Confirmed,
    /// Most recent reading drifted from the target. After a corrective
    /// `WriteDbus` lands, the next reseed will flip this back to
    /// `Confirmed` if the write succeeded.
    Drifted,
}

impl PinnedStatus {
    /// Lowercase wire name for the dashboard table.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Confirmed => "confirmed",
            Self::Drifted => "drifted",
        }
    }
}

/// Per-pinned-register state held in `World::pinned_registers`. One
/// entry per row in `[[dbus_pinned_registers]]`. Keyed by the joined
/// `service:dbus_path` so the shell-side reader can look an entry up
/// in O(log n) without re-splitting the path on every reading.
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedRegisterEntity {
    pub path: std::sync::Arc<str>,
    pub target: PinnedValue,
    pub actual: Option<PinnedValue>,
    pub last_check: Option<chrono::NaiveDateTime>,
    pub drift_count: u32,
    pub last_drift_at: Option<chrono::NaiveDateTime>,
    pub status: PinnedStatus,
}

impl PinnedRegisterEntity {
    /// Build a fresh entity from a `(path, target)` pair seeded by the
    /// shell at startup. All counters start at zero / `None`.
    #[must_use]
    pub fn new(path: std::sync::Arc<str>, target: PinnedValue) -> Self {
        Self {
            path,
            target,
            actual: None,
            last_check: None,
            drift_count: 0,
            last_drift_at: None,
            status: PinnedStatus::Unknown,
        }
    }
}

/// Actuated-entity identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActuatedId {
    GridSetpoint,
    InputCurrentLimit,
    ZappiMode,
    EddiMode,
    Schedule0,
    Schedule1,
    /// PR-keep-batteries-charged: target ESS state
    /// (`/Settings/CGwacs/BatteryLife/State` on
    /// `com.victronenergy.settings`). Mirrored on `SensorId::EssState`
    /// for readback / confirm.
    EssStateTarget,
    // PR-LG-THINQ-B: four LG heat-pump actuated entities.
    /// Master power on/off for the heat pump.
    LgHeatPumpPower,
    /// DHW (hot water) power on/off.
    LgDhwPower,
    /// Heating-water temperature target (°C, i32).
    LgHeatingWaterTarget,
    /// DHW temperature target (°C, i32).
    LgDhwTarget,
}

/// Knob identifiers — one per user-controllable setting in [`crate::knobs::Knobs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnobId {
    ForceDisableExport,
    ExportSocThreshold,
    DischargeSocTarget,
    BatterySocTarget,
    FullChargeDischargeSocTarget,
    FullChargeExportSocThreshold,
    DischargeTime,
    DebugFullCharge,
    PessimismMultiplierModifier,
    DisableNightGridDischarge,
    ChargeCarBoost,
    /// PR-auto-extended-charge: tri-state mode replacing the legacy
    /// boolean `ChargeCarExtended` knob.
    ChargeCarExtendedMode,
    ZappiCurrentTarget,
    ZappiLimit,
    ZappiEmergencyMargin,
    GridExportLimitW,
    GridImportLimitW,
    AllowBatteryToCar,
    EddiEnableSoc,
    EddiDisableSoc,
    EddiDwellS,
    WeathersocWinterTemperatureThreshold,
    WeathersocLowEnergyThreshold,
    WeathersocOkEnergyThreshold,
    WeathersocHighEnergyThreshold,
    WeathersocTooMuchEnergyThreshold,
    /// PR-WSOC-TABLE-1: bucket-boundary kWh knob for the new weather-SoC
    /// 6×2 lookup table. Replaces the hard-coded `1.5 × too_much`
    /// multiplier; default 67.5 kWh.
    WeathersocVerySunnyThreshold,
    ForecastDisagreementStrategy,
    ChargeBatteryExtendedMode,
    // PR-gamma-hold-redesign — four mode selectors.
    ExportSocThresholdMode,
    DischargeSocTargetMode,
    BatterySocTargetMode,
    DisableNightGridDischargeMode,
    // PR-inverter-safe-discharge-knob — gates the legacy 4020 W
    // safety margin in the setpoint controller's `max_discharge`.
    InverterSafeDischargeEnable,
    // PR-baseline-forecast: 4 runtime knobs. Dates are MMDD-encoded
    // u32 (e.g. 1101 = Nov 1, 301 = Mar 1).
    BaselineWinterStartMmDd,
    BaselineWinterEndMmDd,
    BaselineWhPerHourWinter,
    BaselineWhPerHourSummer,
    // PR-keep-batteries-charged: gate + window-offset for the
    // daytime ESS-state override (state 9, KeepBatteriesCharged).
    KeepBatteriesChargedDuringFullCharge,
    SunriseSunsetOffsetMin,
    /// When true, SoC ≥ 99.99 rollover always lands on the Sunday
    /// at-or-after `now + 7d`. Steers `get_next_charge_date_to_sunday_5pm`.
    FullChargeDeferToNextSunday,
    /// Inclusive max weekday for snap-back in the SoC ≥ 99.99 rollover.
    /// Range 1..=5; default 3.
    FullChargeSnapBackMaxWeekday,
    // PR-ZD-2: compensated battery-drain feedback loop knobs.
    /// Compensated drain threshold (W). Default 1000.
    ZappiBatteryDrainThresholdW,
    /// Setpoint-relax step (W/tick). Default 100.
    ZappiBatteryDrainRelaxStepW,
    /// Proportional gain on the drain controller. Default 1.0.
    ZappiBatteryDrainKp,
    /// Reserved reference for a future PI extension (W). Routes via
    /// `KnobValue::Float` (no Int32 variant). Default 0.
    ZappiBatteryDrainTargetW,
    /// Fast-mode hard-clamp threshold (W). Default 200.
    ZappiBatteryDrainHardClampW,
    /// PR-ZDP-1: MPPT probe offset (W). When at least one MPPT reports
    /// voltage/current limited (mode 1), the relax target is pushed
    /// deeper than observed `-solar_export` by this amount. Default 500.
    ZappiBatteryDrainMpptProbeW,
    /// PR-ACT-RETRY-1: universal actuator retry threshold (s). Default 60.
    ActuatorRetryS,

    // PR-LG-THINQ-B: four heat-pump knobs. Bool variants use KnobValue::Bool;
    // °C targets use KnobValue::Uint32 (cast to i64 at the writer boundary).
    /// Master power switch for the LG heat pump.
    LgHeatPumpPower,
    /// DHW (domestic hot water) power switch for the LG heat pump.
    LgDhwPower,
    /// Heating-water temperature target (°C). Range from config.
    LgHeatingWaterTargetC,
    /// DHW temperature target (°C). Range from config.
    LgDhwTargetC,

    /// PR-WSOC-EDIT-1: per-cell knob in the 6×2 weather-SoC lookup
    /// table. Externally surfaces as 48 distinct addressable knobs
    /// (12 cells × 4 fields), each with its own MQTT topic / HA entity
    /// / KNOB_SPEC entry; internally one variant carries the address
    /// triple so every plumbing layer (knob_name, knob_id_from_name,
    /// knob_range, parse_knob_value, apply_knob, all_knob_publish_payloads,
    /// HA discovery) is one programmatic match arm rather than 48
    /// hand-rolled lines.
    WeathersocTableCell {
        bucket: crate::weather_soc_addr::EnergyBucket,
        temp: crate::weather_soc_addr::TempCol,
        field: crate::weather_soc_addr::CellField,
    },

    /// PR-HEATING-CURVE-1: per-cell knob in the 5×2 heating-water
    /// curve. Surfaces as 10 distinct addressable knobs (5 rows × 2
    /// fields). Same parametric pattern as `WeathersocTableCell` —
    /// every plumbing layer matches on the variant once and demuxes
    /// programmatically off the address pair.
    HeatingCurveCell {
        row: crate::heating_curve_addr::RowIndex,
        field: crate::heating_curve_addr::CellField,
    },
}

/// Which branch of the compensated-drain controller fired this tick.
/// Mirrors the `if/else if` ladder in `evaluate_setpoint`'s Zappi
/// branch. Used purely for observability — never feeds back into the
/// controller. LOCKSTEP: `classify_zappi_drain_branch` in
/// `crates/core/src/process.rs` must stay in sync with
/// `evaluate_setpoint`'s branch ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZappiDrainBranch {
    /// Drain > threshold; controller raising setpoint to halt drain.
    Tighten,
    /// Drain ≤ threshold; controller stepping toward `-solar_export`.
    Relax,
    /// `allow_battery_to_car=true` OR `force_disable_export=true` —
    /// the Zappi-active branch was bypassed entirely.
    Bypass,
    /// `world.derived.zappi_active=false` — Zappi not pulling, drain
    /// branch inactive. Reached only when `force_disable_export=false`;
    /// `force_disable_export=true` short-circuits to `Bypass` regardless
    /// of `zappi_active`.
    Disabled,
}

impl ZappiDrainBranch {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Tighten => "Tighten",
            Self::Relax => "Relax",
            Self::Bypass => "Bypass",
            Self::Disabled => "Disabled",
        }
    }
}

impl std::fmt::Display for ZappiDrainBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Forecast providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForecastProvider {
    Solcast,
    ForecastSolar,
    OpenMeteo,
    /// PR-baseline-forecast: locally-computed pessimistic baseline. Used
    /// only as a last-resort fallback when every cloud provider is stale
    /// or unconfigured — see the fallback gate in
    /// `forecast_fusion::fused_today_kwh`.
    Baseline,
}

// =============================================================================
// Timers
// =============================================================================

/// Identifier for every timer-driven action the shell owns. PR-timers-section.
///
/// Each variant is a distinct task / loop in the shell that fires on a
/// fixed cadence (or once at startup); the shell emits an
/// `Event::TimerState` after each fire so the world snapshot can surface
/// last-fire / next-fire / status to the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimerId {
    /// Solcast HTTP poller (forecast scheduler).
    ForecastSolcast,
    /// Forecast.Solar HTTP poller (forecast scheduler).
    ForecastSolar,
    /// Open-Meteo HTTP poller (forecast scheduler).
    OpenMeteo,
    /// PR-baseline-forecast: locally-computed baseline scheduler. No
    /// network I/O; ticks on its cadence to recompute sunrise/sunset and
    /// the flat-during-daylight kWh estimate.
    ForecastBaseline,
    /// Open-Meteo current-weather poller — separate task from the
    /// forecast scheduler.
    OpenMeteoCurrent,
    /// myenergi cloud poller.
    MyenergiPoller,
    /// D-Bus reseed timer for the battery service.
    DbusReseedBattery,
    /// D-Bus reseed timer for the system service.
    DbusReseedSystem,
    /// D-Bus reseed timer for the grid service.
    DbusReseedGrid,
    /// D-Bus reseed timer for the vebus service.
    DbusReseedVebus,
    /// D-Bus reseed timer for the Soltaro pvinverter service.
    DbusReseedPvinverterSoltaro,
    /// D-Bus reseed timer for the evcharger service.
    DbusReseedEvcharger,
    /// D-Bus reseed timer for MPPT (S2) service.
    DbusReseedMpptS2,
    /// D-Bus reseed timer for MPPT (USB1) service.
    DbusReseedMpptUsb1,
    /// D-Bus reseed timer for the settings service.
    DbusReseedSettings,
    /// One-shot MQTT bootstrap (retained-state restore + initial publish).
    MqttBootstrap,
    /// One-shot initial knob publish at startup (PR-2).
    InitialKnobPublish,
    /// PR-LG-THINQ-B: LG ThinQ heat-pump cloud poller (state + temperature readbacks).
    LgThinqPoller,
}

impl TimerId {
    /// All variants — used by the dashboard converter to enumerate the
    /// expected timer set (so the table renders even before the first
    /// fire of a given timer).
    pub const ALL: &'static [TimerId] = &[
        TimerId::ForecastSolcast,
        TimerId::ForecastSolar,
        TimerId::OpenMeteo,
        TimerId::ForecastBaseline,
        TimerId::OpenMeteoCurrent,
        TimerId::MyenergiPoller,
        TimerId::DbusReseedBattery,
        TimerId::DbusReseedSystem,
        TimerId::DbusReseedGrid,
        TimerId::DbusReseedVebus,
        TimerId::DbusReseedPvinverterSoltaro,
        TimerId::DbusReseedEvcharger,
        TimerId::DbusReseedMpptS2,
        TimerId::DbusReseedMpptUsb1,
        TimerId::DbusReseedSettings,
        TimerId::MqttBootstrap,
        TimerId::InitialKnobPublish,
        TimerId::LgThinqPoller,
    ];

    /// Stable dotted identifier — what the wire `Timer.id` field carries
    /// and the user-facing identifier shown on the dashboard.
    /// PR-rename-entities.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ForecastSolcast => "timer.forecast.solcast",
            Self::ForecastSolar => "timer.forecast.solar",
            Self::OpenMeteo => "timer.forecast.open-meteo",
            Self::ForecastBaseline => "timer.forecast.baseline",
            Self::OpenMeteoCurrent => "timer.weather.current",
            Self::MyenergiPoller => "timer.myenergi.poll",
            Self::DbusReseedBattery => "timer.dbus.reseed.battery",
            Self::DbusReseedSystem => "timer.dbus.reseed.system",
            Self::DbusReseedGrid => "timer.dbus.reseed.grid",
            Self::DbusReseedVebus => "timer.dbus.reseed.vebus",
            Self::DbusReseedPvinverterSoltaro => "timer.dbus.reseed.soltaro",
            Self::DbusReseedEvcharger => "timer.dbus.reseed.evcharger",
            Self::DbusReseedMpptS2 => "timer.dbus.reseed.mppt-s2",
            Self::DbusReseedMpptUsb1 => "timer.dbus.reseed.mppt-usb1",
            Self::DbusReseedSettings => "timer.dbus.reseed.settings",
            Self::MqttBootstrap => "timer.mqtt.bootstrap",
            Self::InitialKnobPublish => "timer.knob.initial-publish",
            Self::LgThinqPoller => "timer.lg-thinq.poll",
        }
    }

    /// Human-readable description, surfaced as the dashboard tooltip /
    /// row description column.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::ForecastSolcast => "Solcast HTTP forecast poller",
            Self::ForecastSolar => "Forecast.Solar HTTP forecast poller",
            Self::OpenMeteo => "Open-Meteo HTTP forecast poller",
            Self::ForecastBaseline => "Local baseline forecast (sunrise/sunset × Wh-per-hour)",
            Self::OpenMeteoCurrent => "Open-Meteo current-weather poller",
            Self::MyenergiPoller => "myenergi cloud poller (Zappi/Eddi state + session kWh)",
            Self::DbusReseedBattery => "D-Bus reseed timer for the battery service",
            Self::DbusReseedSystem => "D-Bus reseed timer for the system service",
            Self::DbusReseedGrid => "D-Bus reseed timer for the grid service",
            Self::DbusReseedVebus => "D-Bus reseed timer for the vebus service",
            Self::DbusReseedPvinverterSoltaro => {
                "D-Bus reseed timer for the Soltaro pvinverter service"
            }
            Self::DbusReseedEvcharger => "D-Bus reseed timer for the evcharger service",
            Self::DbusReseedMpptS2 => "D-Bus reseed timer for the MPPT (S2) service",
            Self::DbusReseedMpptUsb1 => "D-Bus reseed timer for the MPPT (USB1) service",
            Self::DbusReseedSettings => "D-Bus reseed timer for the settings service",
            Self::MqttBootstrap => "One-shot MQTT bootstrap (retained-state restore + initial publish)",
            Self::InitialKnobPublish => "One-shot initial knob publish at startup",
            Self::LgThinqPoller => "LG ThinQ heat-pump cloud poller (state + temperature readbacks)",
        }
    }
}

/// Status of a timer-driven action's most recent invocation.
/// Stringified to `snake_case` over the wire (`Timer.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimerStatus {
    /// Idle between fires (the common case).
    Idle,
    /// Currently executing.
    Running,
    /// Last run failed; the timer continues firing on cadence.
    FailedLastRun,
    /// In retry-backoff after a failure.
    RetryBackoff,
}

impl TimerStatus {
    /// `snake_case` wire encoding.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::FailedLastRun => "failed_last_run",
            Self::RetryBackoff => "retry_backoff",
        }
    }
}

/// Which BatteryLife schedule field this target write addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleField {
    Start,
    Duration,
    Soc,
    Days,
    AllowDischarge,
}

// =============================================================================
// Knob values
// =============================================================================

/// A typed knob value. One variant per knob shape (float / int / bool /
/// enum). Type-safe representation; the MQTT serialiser stringifies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KnobValue {
    Bool(bool),
    Float(f64),
    Uint32(u32),
    DischargeTime(DischargeTime),
    DebugFullCharge(DebugFullCharge),
    ForecastDisagreementStrategy(ForecastDisagreementStrategy),
    ChargeBatteryExtendedMode(ChargeBatteryExtendedMode),
    /// PR-auto-extended-charge: tri-state EV-side extended-charge mode.
    ExtendedChargeMode(ExtendedChargeMode),
    // PR-gamma-hold-redesign.
    Mode(Mode),
}

// =============================================================================
// Events — parsed inputs from the outside world
// =============================================================================

/// Scalar sensor reading (D-Bus or MQTT-sourced).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorReading {
    pub id: SensorId,
    pub value: f64,
    pub at: Instant,
}

/// Typed (non-scalar) sensor updates.
///
/// PR-soc-chart-solar: `Forecast::hourly_kwh` carries the per-hour
/// energy estimates the SoC-chart projection consumes. May be empty
/// when the upstream provider didn't return hourly data.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedReading {
    /// PR-EDDI-SENSORS-1: `raw_json` is the pretty-printed body the
    /// poller saw on this poll cycle. Carried through to the dashboard
    /// for the entity inspector raw-response panel; `None` when the
    /// caller didn't capture or couldn't serialize it.
    Zappi { state: ZappiState, at: Instant, raw_json: Option<String> },
    Eddi { mode: EddiMode, at: Instant, raw_json: Option<String> },
    Forecast {
        provider: ForecastProvider,
        today_kwh: f64,
        tomorrow_kwh: f64,
        /// Length-48 vector starting at midnight LOCAL today. Empty when
        /// the provider didn't supply hourly data.
        hourly_kwh: Vec<f64>,
        /// Length-48 vector starting at midnight LOCAL today, °C. Empty
        /// for providers that don't supply temperature (Solcast,
        /// Forecast.Solar, baseline). Open-Meteo populates this from
        /// its `temperature_2m` hourly field.
        hourly_temperature_c: Vec<f64>,
        /// Length-48 vector of cloud-cover percentage in [0, 100] at
        /// the same indexing convention as `hourly_kwh`. Empty for
        /// providers without cloud data. Populated by Open-Meteo
        /// (from `cloud_cover`) and by the baseline forecaster (echo
        /// of the cloud array it consulted for modulation).
        hourly_cloud_cover_pct: Vec<f64>,
        at: Instant,
    },
}

/// Commands originating from dashboard / HA / explicit user action.
///
/// The `Bookkeeping` variant is used only during the MQTT bootstrap
/// phase to seed `World.bookkeeping` from retained state. There is no
/// external source that should issue it at runtime; the controllers
/// themselves own bookkeeping updates via effects.
///
/// `SetBookkeeping` is the user-driven sibling — issued by the dashboard
/// to shift one of a small allowlist of bookkeeping fields. Unlike
/// `Bookkeeping`, the apply path validates `(key, value)` against the
/// allowlist; unsupported combinations emit a Warn log and are dropped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Knob { id: KnobId, value: KnobValue },
    KillSwitch(bool),
    Bookkeeping { key: BookkeepingKey, value: BookkeepingValue },
    SetBookkeeping { key: BookkeepingKey, value: BookkeepingValue },
}

/// Everything the pure core consumes.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Sensor(SensorReading),
    TypedSensor(TypedReading),
    /// PR-actuated-as-sensors (PR-AS-A): rolled-up schedule readback.
    /// The 5 leaf D-Bus fields (`Start`/`Duration`/`Soc`/`Day`/
    /// `AllowDischarge`) per slot land as `Event::Sensor` per leaf;
    /// the subscriber-side accumulator emits this event after a
    /// complete fresh re-observation of all 5 fields. Drives
    /// `world.schedule_<n>.on_reading` + `confirm_if`.
    ScheduleReadback {
        /// `0` or `1`.
        index: u8,
        value: ScheduleSpec,
        at: Instant,
    },
    Command {
        command: Command,
        owner: Owner,
        at: Instant,
    },
    /// Periodic heartbeat. Drives freshness decay and gives controllers
    /// a chance to re-propose in the absence of input events.
    Tick {
        at: Instant,
    },
    /// PR-timers-section: a timer-driven shell task fired (or just
    /// updated its status). Pure observability — `apply_event` upserts
    /// the entry in `world.timers`; no controllers consume this.
    TimerState {
        id: TimerId,
        last_fire_epoch_ms: i64,
        next_fire_epoch_ms: Option<i64>,
        status: TimerStatus,
        at: Instant,
    },
    /// PR-tz-from-victron: Victron-supplied display timezone string
    /// (read from `com.victronenergy.settings`
    /// `/Settings/System/TimeZone`). `apply_event` validates the
    /// string with `chrono_tz::Tz::from_str`; on success it updates
    /// `world.timezone` + bumps `world.timezone_updated_at` and stores
    /// the parsed Tz into `topology.tz_handle` so subsequent
    /// `RealClock::naive()` calls use the operator-configured zone.
    /// Invalid strings emit an `Effect::Log(Warn)` and are dropped —
    /// the controller continues with the previously-loaded Tz (or
    /// UTC at boot). The variant carries `String` so `Event` is no
    /// longer `Copy`.
    Timezone {
        value: String,
        at: Instant,
    },
    /// PR-baseline-forecast: today's sunrise/sunset surfaced as a pair of
    /// non-numeric "sensors". Both are local-time `NaiveDateTime` values
    /// in the **`[forecast].timezone`** TZ — same TZ the cloud forecast
    /// providers use, NOT the (separately-tracked) Victron-display
    /// `world.timezone`. Mismatch is possible if the two are configured
    /// differently; the dashboard renders sunrise/sunset using
    /// `world.timezone` per the existing convention, so on a mismatch the
    /// rendered local time will drift by the offset between the two TZs.
    /// The shell-side baseline scheduler recomputes them once per cadence
    /// using the `sunrise` crate. Polar latitudes can yield no sunrise/
    /// sunset on a given day — the shell drops the event in that case
    /// rather than fabricating placeholder values.
    SunriseSunset {
        sunrise: chrono::NaiveDateTime,
        sunset: chrono::NaiveDateTime,
        at: Instant,
    },
    /// PR-pinned-registers: a fresh reading of a pinned D-Bus register
    /// from the shell-side hourly reader. `path` is the joined
    /// `service:dbus_path` (matches the configured key); `value` is the
    /// typed bus reading the shell extracted from `Get`. `at` is the
    /// wall-clock timestamp the dashboard surfaces. Drives the
    /// `world.pinned_registers` entry's `actual` / `last_check` /
    /// `drift_count` / `last_drift_at` and (on drift) emits an
    /// `Effect::WriteDbusPinned`.
    PinnedRegisterReading {
        path: String,
        value: PinnedValue,
        at: chrono::NaiveDateTime,
    },
}

// =============================================================================
// Effects — produced by `process()` for the shell to execute
// =============================================================================

/// A D-Bus write target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DbusTarget {
    GridSetpoint,
    InputCurrentLimit,
    Schedule {
        index: u8,
        field: ScheduleField,
    },
    /// `/Settings/CGwacs/BatteryLife/State` on
    /// `com.victronenergy.settings`. Written by the ESS-state
    /// controller as an `i32`: 9 (KeepBatteriesCharged) inside the
    /// override window on a full-charge day, 10 (Optimized) otherwise.
    EssState,
}

/// Value written to a D-Bus path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DbusValue {
    Int(i32),
    Float(f64),
}

/// A myenergi API call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MyenergiAction {
    SetZappiMode(ZappiMode),
    SetEddiMode(EddiMode),
}

/// PR-LG-THINQ-B: an LG ThinQ heat-pump API action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LgThinqAction {
    SetHeatPumpPower(bool),
    SetDhwPower(bool),
    SetHeatingWaterTargetC(i64),
    SetDhwTargetC(i64),
}

/// Publishable snapshots (retained MQTT state).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PublishPayload {
    Knob { id: KnobId, value: KnobValue },
    ActuatedPhase {
        id: ActuatedId,
        phase: crate::tass::TargetPhase,
    },
    KillSwitch(bool),
    Bookkeeping(BookkeepingKey, BookkeepingValue),
    /// PR-ha-discovery-expand: scalar sensor publish for HA / dashboard
    /// consumption. Stale and Unknown freshness are encoded as
    /// `"unavailable"` on the wire (HA convention) — see
    /// `mqtt::serialize::encode_publish_payload`. Dedup'd by
    /// `World::last_published_sensors` so quiet sensors don't republish
    /// every tick.
    Sensor {
        id: SensorId,
        value: Option<f64>,
        freshness: crate::tass::Freshness,
    },
    /// PR-ha-discovery-expand: numeric bookkeeping publish (always
    /// meaningful — no Stale handling).
    BookkeepingNumeric {
        id: BookkeepingId,
        value: f64,
    },
    /// PR-ha-discovery-expand: boolean bookkeeping publish.
    BookkeepingBool {
        id: BookkeepingId,
        value: bool,
    },
    /// PR-ZDO-2: numeric controller-derived observable. Uses
    /// `encode_sensor_body` so stale values render as `"unavailable"` on
    /// the HA side.
    ControllerNumeric {
        id: ControllerObservableId,
        value: f64,
        freshness: crate::tass::Freshness,
    },
    /// PR-ZDO-2: boolean controller-derived observable. Always-meaningful
    /// — `false` is honest pre-first-tick output. No freshness gating.
    ControllerBool {
        id: ControllerObservableId,
        value: bool,
    },
    /// String-valued controller observable (text `sensor` in HA).
    /// `value: None` encodes as `"unavailable"` on the wire (HA convention).
    /// Carries a static-string token so `PublishPayload` stays `Copy`.
    ControllerEnumName {
        id: ControllerObservableId,
        value: Option<&'static str>,
    },
}

/// Identifier for a controller-derived broadcast observable.
/// Distinct from `SensorId` (raw sensor reads) and `BookkeepingId`
/// (controller bookkeeping fields). Topic root: `controller/<name>/state`.
///
/// PR-ZDO-2: first three entries surface the M-ZAPPI-DRAIN compensated-drain
/// loop's per-tick state to HA for recording. Future controller-derived
/// observables (setpoint decision tag, schedule activation flags) ride this
/// same prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerObservableId {
    ZappiDrainCompensatedW,
    ZappiDrainTightenActive,
    ZappiDrainHardClampActive,
    /// Process uptime in seconds. Republished on a fixed cadence so HA
    /// can use `expire_after` as a liveness check — when the controller
    /// stops, the entity goes `unavailable` once the expiry elapses.
    AppUptimeS,
    /// PR-DIAG-1: process + host memory diagnostics. Sampled by
    /// `shell::diagnostics` once a minute. Topics live under
    /// `diagnostics.*` so HA groups them separately from the
    /// load-bearing `controller.uptime-s` liveness sensor.
    DiagProcessRssBytes,
    DiagProcessVmHwmBytes,
    DiagProcessVmSizeBytes,
    DiagJemallocAllocatedBytes,
    DiagJemallocResidentBytes,
    DiagHostMemTotalBytes,
    DiagHostMemAvailableBytes,
    DiagHostSwapUsedBytes,
    /// Seconds since host (GX device) boot. Sourced from `/proc/uptime`
    /// by `shell::diagnostics`; published on the diagnostics 60s cadence.
    /// Sits in the `diagnostics.*` namespace alongside the host memory
    /// observables — distinct from `AppUptimeS` ("process running"
    /// elapsed time, the load-bearing liveness heartbeat at 30s).
    DiagHostUptimeS,
    /// Active weather-SoC table cell as a kebab token pair
    /// (`<bucket>.<temp>`, e.g. `"sunny.warm"`). Mirrors
    /// `world.weather_soc_active` for HA. Encoded as a text `sensor`
    /// (no `device_class`); body is `"unavailable"` when the planner
    /// hasn't run yet or skipped (no fresh temp / forecast).
    WeathersocActiveCell,
}

impl ControllerObservableId {
    /// Dotted name used in MQTT topic and HA discovery — without the
    /// `controller/` prefix or `/state` suffix.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ZappiDrainCompensatedW => "zappi-drain.compensated-w",
            Self::ZappiDrainTightenActive => "zappi-drain.tighten-active",
            Self::ZappiDrainHardClampActive => "zappi-drain.hard-clamp-active",
            Self::AppUptimeS => "diagnostics.uptime-s",
            Self::DiagProcessRssBytes => "diagnostics.process-rss-bytes",
            Self::DiagProcessVmHwmBytes => "diagnostics.process-vm-hwm-bytes",
            Self::DiagProcessVmSizeBytes => "diagnostics.process-vm-size-bytes",
            Self::DiagJemallocAllocatedBytes => "diagnostics.jemalloc-allocated-bytes",
            Self::DiagJemallocResidentBytes => "diagnostics.jemalloc-resident-bytes",
            Self::DiagHostMemTotalBytes => "diagnostics.host-mem-total-bytes",
            Self::DiagHostMemAvailableBytes => "diagnostics.host-mem-available-bytes",
            Self::DiagHostSwapUsedBytes => "diagnostics.host-swap-used-bytes",
            Self::DiagHostUptimeS => "diagnostics.host-uptime-s",
            Self::WeathersocActiveCell => "weathersoc.active.cell",
        }
    }

    /// All variants, for iteration in tests and broadcast loops.
    pub const ALL: &'static [Self] = &[
        Self::ZappiDrainCompensatedW,
        Self::ZappiDrainTightenActive,
        Self::ZappiDrainHardClampActive,
        Self::AppUptimeS,
        Self::DiagProcessRssBytes,
        Self::DiagProcessVmHwmBytes,
        Self::DiagProcessVmSizeBytes,
        Self::DiagJemallocAllocatedBytes,
        Self::DiagJemallocResidentBytes,
        Self::DiagHostMemTotalBytes,
        Self::DiagHostMemAvailableBytes,
        Self::DiagHostSwapUsedBytes,
        Self::DiagHostUptimeS,
        Self::WeathersocActiveCell,
    ];
}

/// Identifiers for the controller-relevant bookkeeping fields surfaced
/// on MQTT for HA discovery (PR-ha-discovery-expand).
///
/// Distinct from `BookkeepingKey`: that enum identifies the persisted
/// retained state restored at boot (date-shaped fields included). This
/// enum identifies the live observability slice published to HA every
/// tick — only the seven fields the audit deemed worth exposing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookkeepingId {
    /// Bool. Mirrors `world.derived.zappi_active`.
    ZappiActive,
    /// Bool. `world.bookkeeping.charge_to_full_required`.
    ChargeToFullRequired,
    /// Bool. `world.bookkeeping.charge_battery_extended_today`.
    ChargeBatteryExtendedToday,
    /// %. `world.bookkeeping.soc_end_of_day_target`.
    SocEndOfDayTarget,
    /// %. `world.bookkeeping.effective_export_soc_threshold`.
    EffectiveExportSocThreshold,
    /// %. `world.bookkeeping.battery_selected_soc_target`.
    BatterySelectedSocTarget,
}

impl BookkeepingId {
    /// Stable dotted topic-tail name. PR-rename-entities. Mirrors the
    /// wire taxonomy at the top of `crates/shell/src/mqtt/discovery.rs`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ZappiActive => "evcharger.active",
            Self::ChargeToFullRequired => "schedule.full-charge.required",
            Self::ChargeBatteryExtendedToday => "schedule.extended.charge.today",
            Self::SocEndOfDayTarget => "battery.soc.target.end-of-day",
            Self::EffectiveExportSocThreshold => "battery.soc.threshold.export.effective",
            Self::BatterySelectedSocTarget => "battery.soc.target.selected",
        }
    }
}

/// Encode a sensor's wire body for the HA `state` topic. Single source
/// of truth for both the publish encoder (shell) and the
/// SensorBroadcastCore dedup cache (core). When `freshness` is anything
/// other than `Fresh`, OR `value` is `None` / non-finite, the body is
/// the literal `"unavailable"` (HA convention). Otherwise the value is
/// rounded to 3 decimals and formatted via `f64::Display`, which drops
/// pointless trailing zeros (`42.0` → `"42"`, `42.5` → `"42.5"`).
///
/// PR-ha-discovery-D03/D04: dedup on the encoded body avoids
/// re-publishing identical wire content when raw `f64::to_bits` differs
/// (sub-millisecond noise that rounds away) or when freshness flips
/// among states that all encode to `"unavailable"`.
#[must_use]
pub fn encode_sensor_body(value: Option<f64>, freshness: Freshness) -> String {
    match (freshness, value) {
        (Freshness::Fresh, Some(v)) if v.is_finite() => {
            let rounded = (v * 1000.0).round() / 1000.0;
            format!("{rounded}")
        }
        _ => "unavailable".to_string(),
    }
}

/// Keys for persistent bookkeeping state. Published to retained MQTT so
/// a restart can seed from these topics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookkeepingKey {
    NextFullCharge,
    AboveSocDate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BookkeepingValue {
    NaiveDateTime(chrono::NaiveDateTime),
    NaiveDate(chrono::NaiveDate),
    Cleared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Everything `process()` can produce.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    WriteDbus { target: DbusTarget, value: DbusValue },
    /// PR-pinned-registers: drift-correction write produced by
    /// `run_pinned_registers`. Carries the raw `(service, path, value)`
    /// triplet rather than a `DbusTarget` enum variant — the set of
    /// pinned paths is config-driven and not part of the closed
    /// actuator catalogue. The shell-side dispatch routes through the
    /// same `Writer` and therefore the same `[dbus] writes_enabled`
    /// chokepoint as the regular `WriteDbus`.
    WriteDbusPinned {
        service: String,
        path: String,
        value: PinnedValue,
    },
    CallMyenergi(MyenergiAction),
    /// PR-LG-THINQ-B: an LG ThinQ API call.
    CallLgThinq(LgThinqAction),
    Publish(PublishPayload),
    Log {
        level: LogLevel,
        source: &'static str,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Per PR-cadence-per-sensor: every `SensorId` must satisfy the
    /// universal `staleness ≥ 2 × reseed_cadence` rule (or the
    /// `cadence + 1 s` grace-window rule for external-polled sensors).
    ///
    /// Drives the `regime()` / `reseed_cadence()` directly — no parallel
    /// match table — and cross-checks each variant's `reseed_cadence()`
    /// against the audit table so a single-sensor regression on cadence
    /// fails this test.
    #[test]
    fn freshness_threshold_invariant_holds_for_every_sensor() {
        // Per-variant cadence assertions — mirror of the audit table in
        // `docs/drafts/20260425-1103-pr-cadence-per-sensor-plan.md` §3.
        // A regression that changes a single sensor's cadence in
        // `reseed_cadence()` without updating this test fails loud.
        // EssState (D-Bus settings) and SessionKwh (myenergi cloud)
        // share the 300 s value but come from semantically distinct
        // sources — keep the arms separate.
        #[allow(clippy::match_same_arms)]
        for &id in SensorId::ALL {
            let expected_cadence = match id {
                // Fast-organic — 5 s.
                SensorId::PowerConsumption
                | SensorId::ConsumptionCurrent
                | SensorId::GridPower
                | SensorId::GridVoltage
                | SensorId::GridCurrent
                | SensorId::BatteryDcPower
                | SensorId::SoltaroPower
                | SensorId::OffgridPower
                | SensorId::OffgridCurrent
                | SensorId::VebusInputCurrent
                | SensorId::EvchargerAcPower
                | SensorId::EvchargerAcCurrent
                // MPPTs — also fast-organic at 5 s.
                | SensorId::MpptPower0
                | SensorId::MpptPower1 => Duration::from_secs(5),
                // Battery service — 60 s.
                SensorId::BatterySoc
                | SensorId::BatterySoh
                | SensorId::BatteryInstalledCapacity => Duration::from_secs(60),
                // Settings (ESS) — 300 s.
                SensorId::EssState => Duration::from_secs(300),
                // Open-Meteo — 30 min.
                SensorId::OutdoorTemperature => Duration::from_secs(30 * 60),
                // myenergi cloud — 5 min.
                SensorId::SessionKwh => Duration::from_secs(300),
                // PR-actuated-as-sensors (PR-AS-A): grid-setpoint &
                // current-limit readback paths reseed at 5 s.
                SensorId::GridSetpointActual
                | SensorId::InputCurrentLimitActual => Duration::from_secs(5),
                // PR-actuated-as-sensors (PR-AS-A): schedule leaf
                // paths reseed at 60 s.
                SensorId::Schedule0StartActual
                | SensorId::Schedule0DurationActual
                | SensorId::Schedule0SocActual
                | SensorId::Schedule0DaysActual
                | SensorId::Schedule0AllowDischargeActual
                | SensorId::Schedule1StartActual
                | SensorId::Schedule1DurationActual
                | SensorId::Schedule1SocActual
                | SensorId::Schedule1DaysActual
                | SensorId::Schedule1AllowDischargeActual => Duration::from_secs(60),
                // PR-ev-soc-sensor / PR-auto-extended-charge: tightened
                // to 10 min — MQTT receive is cheap and we want a
                // prompt Stale signal when the gateway stops publishing.
                SensorId::EvSoc | SensorId::EvChargeTarget => Duration::from_secs(10 * 60),
                // PR-ZD-1: 15 s for all four new sensors.
                SensorId::HeatPumpPower
                | SensorId::CookerPower
                | SensorId::Mppt0OperationMode
                | SensorId::Mppt1OperationMode => Duration::from_secs(15),
                // PR-LG-THINQ-B: 60 s LG cloud poll cadence.
                SensorId::LgHeatPumpPowerActual
                | SensorId::LgDhwPowerActual
                | SensorId::LgHeatingWaterTargetActual
                | SensorId::LgDhwTargetActual
                | SensorId::LgDhwCurrentTemperatureC
                | SensorId::LgHeatingWaterCurrentTemperatureC => Duration::from_secs(60),
            };
            assert_eq!(
                id.reseed_cadence(),
                expected_cadence,
                "SensorId::{id:?} cadence mismatch with audit table"
            );

            // PR-cadence-per-sensor-D04: cross-check `regime()`. The
            // universal staleness rule no longer branches on regime, but
            // the regime is a documentation aid — if it silently rots
            // (e.g. someone reclassifies BatterySoc → ReseedDriven),
            // future readers will be misled. Pin every variant.
            let expected_regime = match id {
                SensorId::PowerConsumption
                | SensorId::ConsumptionCurrent
                | SensorId::GridPower
                | SensorId::GridVoltage
                | SensorId::GridCurrent
                | SensorId::BatteryDcPower
                | SensorId::BatterySoc
                | SensorId::SoltaroPower
                | SensorId::OffgridPower
                | SensorId::OffgridCurrent
                | SensorId::VebusInputCurrent
                | SensorId::EvchargerAcPower
                | SensorId::EvchargerAcCurrent
                | SensorId::MpptPower0
                | SensorId::MpptPower1
                // PR-actuated-as-sensors (PR-AS-A).
                | SensorId::GridSetpointActual
                | SensorId::InputCurrentLimitActual => FreshnessRegime::SlowSignalled,
                SensorId::BatterySoh
                | SensorId::BatteryInstalledCapacity
                | SensorId::EssState
                | SensorId::OutdoorTemperature
                | SensorId::SessionKwh
                // PR-actuated-as-sensors (PR-AS-A).
                | SensorId::Schedule0StartActual
                | SensorId::Schedule0DurationActual
                | SensorId::Schedule0SocActual
                | SensorId::Schedule0DaysActual
                | SensorId::Schedule0AllowDischargeActual
                | SensorId::Schedule1StartActual
                | SensorId::Schedule1DurationActual
                | SensorId::Schedule1SocActual
                | SensorId::Schedule1DaysActual
                | SensorId::Schedule1AllowDischargeActual
                // PR-ev-soc-sensor.
                | SensorId::EvSoc
                // PR-auto-extended-charge.
                | SensorId::EvChargeTarget
                // PR-ZD-1: MPPT op-modes are reseed-driven.
                | SensorId::Mppt0OperationMode
                | SensorId::Mppt1OperationMode
                // PR-LG-THINQ-B: all 6 LG sensors are reseed-driven.
                | SensorId::LgHeatPumpPowerActual
                | SensorId::LgDhwPowerActual
                | SensorId::LgHeatingWaterTargetActual
                | SensorId::LgDhwTargetActual
                | SensorId::LgDhwCurrentTemperatureC
                | SensorId::LgHeatingWaterCurrentTemperatureC => FreshnessRegime::ReseedDriven,
                // PR-ZD-1: HP/cooker are SlowSignalled (z2m push on change).
                SensorId::HeatPumpPower | SensorId::CookerPower => FreshnessRegime::SlowSignalled,
            };
            assert_eq!(
                id.regime(),
                expected_regime,
                "SensorId::{id:?} regime mismatch with audit table"
            );

            let staleness = id.freshness_threshold();
            let cadence = id.reseed_cadence();
            // Universal rule. External-polled sensors (Open-Meteo, the
            // myenergi-sourced `SessionKwh`) use the grace-window
            // model — see `is_external_polled` doc.
            let required = if matches!(
                id,
                SensorId::OutdoorTemperature
                    | SensorId::SessionKwh
                    // PR-ev-soc-sensor.
                    | SensorId::EvSoc
                    // PR-auto-extended-charge.
                    | SensorId::EvChargeTarget,
            ) {
                cadence + Duration::from_secs(1)
            } else {
                2 * cadence
            };
            assert!(
                staleness >= required,
                "SensorId::{id:?} staleness {staleness:?} < required {required:?}",
            );

            // Belt-and-braces: the shared helper agrees.
            check_staleness_invariant(id).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    /// PR-actuated-as-sensors (PR-AS-A): explicit per-variant assertion
    /// of `SensorId::actuated_id()`. Mirrors the impl's match shape so a
    /// new `SensorId` variant fails compile here until classified.
    #[test]
    #[allow(clippy::match_same_arms)]
    fn sensor_id_actuated_id_mapping() {
        for &id in SensorId::ALL {
            let expected: Option<ActuatedId> = match id {
                SensorId::GridSetpointActual => Some(ActuatedId::GridSetpoint),
                SensorId::InputCurrentLimitActual => Some(ActuatedId::InputCurrentLimit),
                SensorId::Schedule0StartActual
                | SensorId::Schedule0DurationActual
                | SensorId::Schedule0SocActual
                | SensorId::Schedule0DaysActual
                | SensorId::Schedule0AllowDischargeActual => Some(ActuatedId::Schedule0),
                SensorId::Schedule1StartActual
                | SensorId::Schedule1DurationActual
                | SensorId::Schedule1SocActual
                | SensorId::Schedule1DaysActual
                | SensorId::Schedule1AllowDischargeActual => Some(ActuatedId::Schedule1),
                SensorId::BatterySoc
                | SensorId::BatterySoh
                | SensorId::BatteryInstalledCapacity
                | SensorId::BatteryDcPower
                | SensorId::MpptPower0
                | SensorId::MpptPower1
                | SensorId::SoltaroPower
                | SensorId::PowerConsumption
                | SensorId::GridPower
                | SensorId::GridVoltage
                | SensorId::GridCurrent
                | SensorId::ConsumptionCurrent
                | SensorId::OffgridPower
                | SensorId::OffgridCurrent
                | SensorId::VebusInputCurrent
                | SensorId::EvchargerAcPower
                | SensorId::EvchargerAcCurrent
                | SensorId::EssState
                | SensorId::OutdoorTemperature
                | SensorId::SessionKwh
                // PR-ev-soc-sensor.
                | SensorId::EvSoc
                // PR-auto-extended-charge.
                | SensorId::EvChargeTarget
                // PR-ZD-1.
                | SensorId::HeatPumpPower
                | SensorId::CookerPower
                | SensorId::Mppt0OperationMode
                | SensorId::Mppt1OperationMode
                // PR-LG-THINQ-B: plain temperature sensors.
                | SensorId::LgDhwCurrentTemperatureC
                | SensorId::LgHeatingWaterCurrentTemperatureC => None,
                // PR-LG-THINQ-B: 4 actuated-mirror sensors.
                SensorId::LgHeatPumpPowerActual => Some(ActuatedId::LgHeatPumpPower),
                SensorId::LgDhwPowerActual => Some(ActuatedId::LgDhwPower),
                SensorId::LgHeatingWaterTargetActual => Some(ActuatedId::LgHeatingWaterTarget),
                SensorId::LgDhwTargetActual => Some(ActuatedId::LgDhwTarget),
            };
            assert_eq!(
                id.actuated_id(),
                expected,
                "SensorId::{id:?}.actuated_id() mismatch",
            );
        }
    }

    /// Property-style assertion of the universal rule for the fast-
    /// organic slice (cadence ≤ 15 s). A future regression that lowers
    /// `freshness_threshold` for a fast sensor below 2× cadence trips
    /// this even if the per-variant table above is also edited.
    #[test]
    fn fast_organic_sensors_satisfy_universal_rule() {
        for &id in SensorId::ALL {
            let cadence = id.reseed_cadence();
            if cadence > Duration::from_secs(15) {
                continue;
            }
            let staleness = id.freshness_threshold();
            let required = 2 * cadence;
            assert!(
                staleness >= required,
                "SensorId::{id:?} (cadence={cadence:?}) staleness {staleness:?} \
                 < 2× cadence {required:?}",
            );
        }
    }
}
