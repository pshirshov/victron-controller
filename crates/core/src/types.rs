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
            // Rule: `staleness >= 2 * cadence` for slow-signalled sensors
            // (60 s reseed → 120 s window).
            Self::BatterySoc => Duration::from_secs(120),
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

/// Classification of a sensor's freshness regime.
///
/// Drives the staleness-floor invariant — see
/// `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md` and the
/// PR-staleness-floor section of M-UX-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessRegime {
    /// Organic `ItemsChanged` signals at ≥ 1 Hz drive freshness; the
    /// reseed is belt-and-suspenders. Lower bound only: `staleness ≥ 5 s`.
    Fast,
    /// Organic signals fire on change but inter-change gaps span
    /// seconds–minutes. Both regimes apply: `staleness ≥ 2 × cadence`.
    SlowSignalled,
    /// Organic signals essentially never fire; reseed IS the freshness
    /// source. `staleness ≥ 2 × cadence`.
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
    ];

    /// Freshness regime for this sensor — see [`FreshnessRegime`].
    ///
    /// Authority: the audit table in
    /// `docs/drafts/20260425-0130-m-ux-1-plan.md` § "PR-staleness-floor".
    /// Explicit per-variant match (no `_` arm) so adding a new variant
    /// forces an explicit classification call at compile time.
    #[must_use]
    pub const fn regime(self) -> FreshnessRegime {
        match self {
            // Fast — organic signals at ≥ 1 Hz.
            //
            // MPPTs are classified Fast despite emitting at sub-second
            // cadence sunny / silently at night (PV=0): Stale-at-night
            // is semantically correct since controllers treat the value
            // as 0 W via the `solar_export_w` rule. The matrix-doc note
            // on the 30 s window covers the trade-off.
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
            | Self::EvchargerAcCurrent => FreshnessRegime::Fast,
            // Slow-signalled — emits on change but gaps can be minutes.
            Self::BatterySoc => FreshnessRegime::SlowSignalled,
            // Reseed-driven — value moves on minutes-to-hours timescales.
            Self::BatterySoh
            | Self::BatteryInstalledCapacity
            | Self::EssState
            | Self::OutdoorTemperature => FreshnessRegime::ReseedDriven,
        }
    }

    /// Reseed cadence for this sensor in the regime where reseed matters.
    ///
    /// Mirrors the constants in `crates/shell/src/dbus/subscriber.rs`
    /// (`SEED_INTERVAL_DEFAULT = 60s`, `SEED_INTERVAL_SETTINGS = 300s`)
    /// plus the Open-Meteo poll cadence for `OutdoorTemperature`. Hard-
    /// coded here so the core crate doesn't pull a dependency on the
    /// shell crate just to validate its own invariants.
    #[must_use]
    pub const fn reseed_cadence(self) -> std::time::Duration {
        use std::time::Duration;
        match self {
            // Reseeded by `SEED_INTERVAL_DEFAULT = 60 s`.
            Self::BatterySoc
            | Self::BatterySoh
            | Self::BatteryInstalledCapacity
            | Self::BatteryDcPower
            | Self::MpptPower0
            | Self::MpptPower1
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
            | Self::EvchargerAcCurrent => Duration::from_secs(60),
            // Reseeded by `SEED_INTERVAL_SETTINGS = 300 s`.
            Self::EssState => Duration::from_secs(300),
            // Open-Meteo poll cadence (30 min).
            Self::OutdoorTemperature => Duration::from_secs(30 * 60),
        }
    }
}

/// Lower bound for fast-regime staleness. Bias-to-safety floor — the
/// only check we can usefully do for sub-second sensors at the
/// unit-test level.
pub const FAST_REGIME_STALENESS_FLOOR: std::time::Duration =
    std::time::Duration::from_secs(5);

/// Sensors whose cadence is NOT governed by the D-Bus subscriber
/// constants and which therefore use a grace-window freshness model
/// (`staleness = cadence + slack`) rather than the strict `2× cadence`
/// headroom. Mirrors the user-stated myenergi exclusion in
/// `docs/drafts/20260425-0130-m-ux-1-plan.md` § "PR-staleness-floor".
#[must_use]
const fn is_external_polled(id: SensorId) -> bool {
    matches!(id, SensorId::OutdoorTemperature)
}

/// Verify the per-sensor staleness invariant for one sensor. Returns
/// `Err(message)` describing the violation if the invariant fails.
///
/// Used by both the `freshness_threshold_invariant_holds_for_every_sensor`
/// unit test and by `Runtime::new` as a startup belt-and-braces check.
#[allow(clippy::missing_errors_doc)]
pub fn check_staleness_invariant(id: SensorId) -> Result<(), String> {
    let staleness = id.freshness_threshold();
    match id.regime() {
        FreshnessRegime::Fast => {
            if staleness < FAST_REGIME_STALENESS_FLOOR {
                return Err(format!(
                    "staleness invariant violated: SensorId::{id:?} \
                     staleness={}s < fast-regime floor {}s; \
                     fix freshness_threshold",
                    staleness.as_secs(),
                    FAST_REGIME_STALENESS_FLOOR.as_secs()
                ));
            }
        }
        FreshnessRegime::SlowSignalled | FreshnessRegime::ReseedDriven => {
            let cadence = id.reseed_cadence();
            // External-polled sensors (Open-Meteo, future myenergi)
            // use a grace-window model: `staleness > cadence` is the
            // ping-pong-avoidance floor; the strict 2× headroom doesn't
            // apply because their cadence is owned by an external
            // service, not the D-Bus reseed constants.
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
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Per-PR-staleness-floor: every `SensorId` must satisfy the regime-
    /// dependent staleness invariant. The match below is intentionally
    /// explicit (no `_` arm) so adding a new variant forces the test
    /// author to classify it. Cadence numbers mirror
    /// `SEED_INTERVAL_DEFAULT` / `SEED_INTERVAL_SETTINGS` in
    /// `crates/shell/src/dbus/subscriber.rs`; hard-coded here because the
    /// core crate cannot depend on the shell.
    #[test]
    fn freshness_threshold_invariant_holds_for_every_sensor() {
        const SEED_INTERVAL_DEFAULT: Duration = Duration::from_secs(60);
        const SEED_INTERVAL_SETTINGS: Duration = Duration::from_secs(300);
        const OPEN_METEO_CADENCE: Duration = Duration::from_secs(30 * 60);

        for &id in SensorId::ALL {
            // Categorise via an explicit match — adding a variant later
            // breaks the build until the new arm is added.
            let (regime, cadence): (FreshnessRegime, Duration) = match id {
                // Fast — organic ItemsChanged ≥ 1 Hz drives freshness.
                // MPPTs join this group: silent at night when PV=0 is
                // semantically correct (controllers treat Stale as 0 W).
                SensorId::PowerConsumption
                | SensorId::ConsumptionCurrent
                | SensorId::GridPower
                | SensorId::GridCurrent
                | SensorId::GridVoltage
                | SensorId::BatteryDcPower
                | SensorId::SoltaroPower
                | SensorId::MpptPower0
                | SensorId::MpptPower1
                | SensorId::OffgridPower
                | SensorId::OffgridCurrent
                | SensorId::VebusInputCurrent
                | SensorId::EvchargerAcPower
                | SensorId::EvchargerAcCurrent => {
                    (FreshnessRegime::Fast, SEED_INTERVAL_DEFAULT)
                }
                // Slow-signalled — emits on change, gaps span minutes.
                SensorId::BatterySoc => {
                    (FreshnessRegime::SlowSignalled, SEED_INTERVAL_DEFAULT)
                }
                // Reseed-driven — value moves on minutes-to-hours.
                SensorId::BatterySoh | SensorId::BatteryInstalledCapacity => {
                    (FreshnessRegime::ReseedDriven, SEED_INTERVAL_DEFAULT)
                }
                SensorId::EssState => {
                    (FreshnessRegime::ReseedDriven, SEED_INTERVAL_SETTINGS)
                }
                SensorId::OutdoorTemperature => {
                    (FreshnessRegime::ReseedDriven, OPEN_METEO_CADENCE)
                }
            };

            // Cross-check: the local categorisation matches the impl-side
            // metadata. A mismatch here means the test and the runtime
            // assertion would diverge — fail loud rather than silently.
            assert_eq!(
                id.regime(),
                regime,
                "SensorId::{id:?} regime mismatch between test and `regime()`"
            );
            assert_eq!(
                id.reseed_cadence(),
                cadence,
                "SensorId::{id:?} cadence mismatch between test and `reseed_cadence()`"
            );

            let staleness = id.freshness_threshold();
            match regime {
                FreshnessRegime::Fast => {
                    assert!(
                        staleness >= FAST_REGIME_STALENESS_FLOOR,
                        "SensorId::{id:?} (Fast) staleness {staleness:?} < {FAST_REGIME_STALENESS_FLOOR:?}",
                    );
                }
                FreshnessRegime::SlowSignalled | FreshnessRegime::ReseedDriven => {
                    // External-polled sensors (Open-Meteo today, future
                    // myenergi `SessionKwh`) use a grace-window model
                    // — see `is_external_polled` doc.
                    let required = if matches!(id, SensorId::OutdoorTemperature) {
                        cadence + Duration::from_secs(1)
                    } else {
                        2 * cadence
                    };
                    assert!(
                        staleness >= required,
                        "SensorId::{id:?} ({regime:?}) staleness {staleness:?} < required {required:?}",
                    );
                }
            }

            // Belt-and-braces: the shared helper agrees.
            check_staleness_invariant(id).unwrap_or_else(|e| panic!("{e}"));
        }
    }
}
