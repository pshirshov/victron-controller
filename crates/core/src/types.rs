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
    ChargeBatteryExtendedMode, DebugFullCharge, DischargeTime, ForecastDisagreementStrategy, Mode,
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
            Self::Schedule0StartActual
            | Self::Schedule0DurationActual
            | Self::Schedule0SocActual
            | Self::Schedule0DaysActual
            | Self::Schedule0AllowDischargeActual
            | Self::Schedule1StartActual
            | Self::Schedule1DurationActual
            | Self::Schedule1SocActual
            | Self::Schedule1DaysActual
            | Self::Schedule1AllowDischargeActual => Duration::from_secs(180),
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
            | Self::InputCurrentLimitActual => FreshnessRegime::SlowSignalled,
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
            | Self::Schedule1AllowDischargeActual => FreshnessRegime::ReseedDriven,
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
            Self::BatterySoc
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
            | Self::EssState
            | Self::OutdoorTemperature
            | Self::SessionKwh => None,
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
        SensorId::OutdoorTemperature | SensorId::SessionKwh
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

/// Actuated-entity identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActuatedId {
    GridSetpoint,
    InputCurrentLimit,
    ZappiMode,
    EddiMode,
    Schedule0,
    Schedule1,
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
    ChargeCarExtended,
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
}

/// Forecast providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForecastProvider {
    Solcast,
    ForecastSolar,
    OpenMeteo,
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
}

impl TimerId {
    /// All variants — used by the dashboard converter to enumerate the
    /// expected timer set (so the table renders even before the first
    /// fire of a given timer).
    pub const ALL: &'static [TimerId] = &[
        TimerId::ForecastSolcast,
        TimerId::ForecastSolar,
        TimerId::OpenMeteo,
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypedReading {
    Zappi { state: ZappiState, at: Instant },
    Eddi { mode: EddiMode, at: Instant },
    Forecast {
        provider: ForecastProvider,
        today_kwh: f64,
        tomorrow_kwh: f64,
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
    // PR-ha-discovery-D01 (resolved): `prev_ess_state` was originally
    // surfaced here too, but `BookkeepingKey::PrevEssState` already owns
    // `bookkeeping/prev_ess_state/state` for the persistence path. Two
    // writers on the same retained topic with different body formats
    // (canonical "null"/int vs plain f64) would scramble restore. The
    // ESS state code is also low-value as an HA entity. Skip HA exposure
    // entirely and let the persistence path remain the sole writer.
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
    PrevEssState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BookkeepingValue {
    NaiveDateTime(chrono::NaiveDateTime),
    NaiveDate(chrono::NaiveDate),
    OptionalInt(Option<i32>),
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
    CallMyenergi(MyenergiAction),
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
                | SensorId::Schedule1AllowDischargeActual => FreshnessRegime::ReseedDriven,
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
                SensorId::OutdoorTemperature | SensorId::SessionKwh,
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
                | SensorId::SessionKwh => None,
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
