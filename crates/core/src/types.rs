//! Top-level typed Events and Effects the pure `process()` consumes and
//! produces. See SPEC §5.5.
//!
//! All IDs are closed enums — the shell is responsible for parsing wire
//! formats (D-Bus / MQTT / HTTP) into these typed variants before calling
//! `process`. Correspondingly, all `Effect`s are typed; the shell
//! serialises them back to wire format when executing.

use crate::controllers::schedules::ScheduleSpec;
use crate::knobs::{
    ChargeBatteryExtendedMode, DebugFullCharge, DischargeTime, ForecastDisagreementStrategy,
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
}

impl SensorId {
    /// Per-sensor Fresh→Stale threshold.
    ///
    /// Values are authoritative per
    /// `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md` and
    /// must only change via PR review. See that matrix for the invariant
    /// `staleness > max(organic-gap, reseed-cadence)`.
    #[must_use]
    pub const fn freshness_threshold(self) -> std::time::Duration {
        use std::time::Duration;
        match self {
            // Fast paths: organic ItemsChanged at ~1 Hz drives freshness;
            // 5 s means "fail fast on signal loss".
            Self::PowerConsumption
            | Self::ConsumptionCurrent
            | Self::GridPower
            | Self::GridCurrent
            | Self::BatteryDcPower
            | Self::SoltaroPower
            | Self::OffgridPower
            | Self::OffgridCurrent
            | Self::VebusInputCurrent
            | Self::EvchargerAcPower
            | Self::EvchargerAcCurrent => Duration::from_secs(5),
            // Slow-moving fast path: grid voltage sampled regularly but
            // doesn't move much; a slightly looser window avoids spurious
            // Stale during signal jitter.
            Self::GridVoltage => Duration::from_secs(10),
            // Slow-signalled: Pylontech emits SoC at ~1 Hz while changing,
            // seconds-to-minutes idle.
            Self::BatterySoc => Duration::from_secs(15),
            // Reseed-driven slow metrics: value only moves on minutes-
            // to-hours timescales; staleness ≈ 2× reseed cadence.
            Self::BatterySoh | Self::EssState => Duration::from_secs(900),
            // Essentially static — reseed every hour.
            Self::BatteryInstalledCapacity => Duration::from_secs(3600),
            // MPPTs: sub-second while sun up, silent at night when PV=0.
            Self::MpptPower0 | Self::MpptPower1 => Duration::from_secs(30),
            // Outdoor temperature comes from Open-Meteo (30 min cadence);
            // give a 10 min grace window on top.
            Self::OutdoorTemperature => Duration::from_secs(40 * 60),
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
}

impl ActuatedId {
    /// Per-actuator Fresh→Stale threshold for the *readback* side.
    ///
    /// Readbacks change only when someone writes the underlying path,
    /// so the staleness window is measured in minutes to hours rather
    /// than seconds (sensor regime). Values are authoritative per
    /// `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md`.
    ///
    /// **Not defined** for `ZappiMode`/`EddiMode`: those readbacks come
    /// from the myenergi poller (not D-Bus) and share their freshness
    /// window with the typed sensors on the same source. The single
    /// source of truth is `ControllerParams::freshness_myenergi` — see
    /// `apply_tick` in `process.rs`. Calling this on those variants
    /// panics, to surface an accidental duplicate-threshold at compile
    /// time of a caller rather than silently diverge.
    #[must_use]
    pub const fn freshness_threshold(self) -> std::time::Duration {
        use std::time::Duration;
        match self {
            // CurrentLimit readback: reseed 60 s (vebus), staleness 600 s.
            Self::InputCurrentLimit => Duration::from_secs(600),
            // Grid setpoint & schedules: reseed 300 s (settings), staleness 900 s.
            Self::GridSetpoint | Self::Schedule0 | Self::Schedule1 => {
                Duration::from_secs(900)
            }
            Self::ZappiMode | Self::EddiMode => panic!(
                "ActuatedId::{{Zappi,Eddi}}Mode freshness is driven by \
                 ControllerParams::freshness_myenergi, not this method"
            ),
        }
    }
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
}

/// Forecast providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForecastProvider {
    Solcast,
    ForecastSolar,
    OpenMeteo,
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

/// Readback of an actuated entity (from D-Bus after a write lands, or from
/// myenergi on its next poll).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActuatedReadback {
    GridSetpoint { value: i32, at: Instant },
    InputCurrentLimit { value: f64, at: Instant },
    ZappiMode { mode: ZappiMode, at: Instant },
    EddiMode { mode: EddiMode, at: Instant },
    Schedule0 { value: ScheduleSpec, at: Instant },
    Schedule1 { value: ScheduleSpec, at: Instant },
}

/// Commands originating from dashboard / HA / explicit user action.
///
/// The `Bookkeeping` variant is used only during the MQTT bootstrap
/// phase to seed `World.bookkeeping` from retained state. There is no
/// external source that should issue it at runtime; the controllers
/// themselves own bookkeeping updates via effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Knob { id: KnobId, value: KnobValue },
    KillSwitch(bool),
    Bookkeeping { key: BookkeepingKey, value: BookkeepingValue },
}

/// Everything the pure core consumes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Sensor(SensorReading),
    TypedSensor(TypedReading),
    Readback(ActuatedReadback),
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
