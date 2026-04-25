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
    /// Cumulative energy delivered to the EV in the current Zappi
    /// session (kWh). Sourced from the myenergi cloud poll's `che`
    /// field; resets when a session ends. Reseed-driven (cadence
    /// 300 s, owned by the myenergi poller, not the D-Bus subscriber).
    SessionKwh,
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
            | Self::BatterySoc => FreshnessRegime::SlowSignalled,
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
            | Self::SessionKwh => FreshnessRegime::ReseedDriven,
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
            | Self::MpptPower1 => Duration::from_secs(5),
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
    /// Stable `snake_case` topic-tail name. Mirrors the wire taxonomy
    /// at the top of `crates/shell/src/mqtt/discovery.rs`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ZappiActive => "zappi_active",
            Self::ChargeToFullRequired => "charge_to_full_required",
            Self::ChargeBatteryExtendedToday => "charge_battery_extended_today",
            Self::SocEndOfDayTarget => "soc_end_of_day_target",
            Self::EffectiveExportSocThreshold => "effective_export_soc_threshold",
            Self::BatterySelectedSocTarget => "battery_selected_soc_target",
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
                | SensorId::MpptPower1 => FreshnessRegime::SlowSignalled,
                SensorId::BatterySoh
                | SensorId::BatteryInstalledCapacity
                | SensorId::EssState
                | SensorId::OutdoorTemperature
                | SensorId::SessionKwh => FreshnessRegime::ReseedDriven,
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
