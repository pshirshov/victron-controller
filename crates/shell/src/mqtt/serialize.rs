//! MQTT wire format: `PublishPayload` ↔ `(topic, body, retain)`
//! ↔ `Event::Command`.
//!
//! All state messages are retained JSON so a reboot sees the last
//! values. Logs are not retained.

use std::sync::OnceLock;
use std::time::Instant;

use serde_json::json;
use tracing::warn;

use victron_controller_core::knobs::{
    ChargeBatteryExtendedMode, DebugFullCharge, DischargeTime, ExtendedChargeMode,
    ForecastDisagreementStrategy, Mode,
};
#[cfg(test)]
use victron_controller_core::tass::Freshness;
use victron_controller_core::types::{
    ActuatedId, BookkeepingKey, BookkeepingValue, Command, Event, KnobId, KnobValue,
    PublishPayload, SensorId, SensorReading, encode_sensor_body,
};
use victron_controller_core::{HardwareParams, Owner};

/// PR-hardware-config: hardware constants threaded into the MQTT
/// layer. Set once at startup via [`set_hardware_params`]; read by
/// [`knob_range`] to compute per-direction `grid_*_limit_w` ceilings
/// (export defaults to 6000 W, import to 13_000 W). The OnceLock
/// approach was chosen over plumbing `&HardwareParams` through every
/// `knob_range` caller because:
///   - `knob_range` has many call sites (HA discovery, retained-MQTT
///     ingest validation, tests) and the parameter is otherwise
///     completely irrelevant to them;
///   - the value is set once at startup and never mutates, so the
///     `OnceLock` semantics match the actual lifecycle precisely;
///   - tests that don't explicitly initialise it transparently get
///     `HardwareParams::defaults()` (see `hardware_params()` below).
static HARDWARE_PARAMS: OnceLock<HardwareParams> = OnceLock::new();

/// Initialise the global hardware params for MQTT-layer use. Called
/// once from `main` after config load. Panics if called twice — that
/// would indicate a bug in startup wiring.
pub fn set_hardware_params(hw: HardwareParams) {
    HARDWARE_PARAMS
        .set(hw)
        .expect("set_hardware_params called twice");
}

/// Read the global hardware params. Returns `HardwareParams::defaults()`
/// when the cell hasn't been set (test-only path — production startup
/// always calls `set_hardware_params` before any MQTT I/O).
fn hardware_params() -> &'static HardwareParams {
    HARDWARE_PARAMS.get_or_init(HardwareParams::defaults)
}

/// Given a PublishPayload, produce a (subtopic, body, retain) triple
/// for MQTT. The subtopic is appended to the configured topic_root.
/// Returns None for payloads we deliberately don't publish.
pub fn encode_publish_payload(p: &PublishPayload) -> Option<(String, String, bool)> {
    match p {
        PublishPayload::Knob { id, value } => {
            let name = knob_name(*id);
            let body = encode_knob_value(*value);
            Some((format!("knob/{name}/state"), body, true))
        }
        PublishPayload::ActuatedPhase { id, phase } => {
            let body = json!({ "phase": format!("{phase:?}") }).to_string();
            Some((format!("entity/{}/phase", actuated_name(*id)), body, true))
        }
        PublishPayload::KillSwitch(v) => Some((
            "writes_enabled/state".to_string(),
            if *v { "true".to_string() } else { "false".to_string() },
            true,
        )),
        PublishPayload::Bookkeeping(key, value) => {
            let name = bookkeeping_name(*key);
            let body = encode_bookkeeping_value(*value);
            Some((format!("bookkeeping/{name}/state"), body, true))
        }
        // PR-ha-discovery-expand: scalar sensors. Body encoding lives
        // in core's `encode_sensor_body` so the SensorBroadcastCore
        // dedup cache and this wire encoder cannot drift.
        PublishPayload::Sensor { id, value, freshness } => {
            let name = sensor_name(*id);
            let body = encode_sensor_body(*value, *freshness);
            Some((format!("sensor/{name}/state"), body, true))
        }
        PublishPayload::BookkeepingNumeric { id, value } => {
            let name = id.name();
            Some((
                format!("bookkeeping/{name}/state"),
                format_sensor_value(*value),
                true,
            ))
        }
        PublishPayload::BookkeepingBool { id, value } => {
            let name = id.name();
            let body = if *value { "true" } else { "false" }.to_string();
            Some((format!("bookkeeping/{name}/state"), body, true))
        }
    }
}

/// Format a sensor / bookkeeping numeric for the wire. Trims pointless
/// trailing zeros via `f64::Display` (e.g. `42.0` → `"42"`, `42.5`
/// → `"42.5"`). Keeps three decimals for finer-grained values
/// (`1234.5678` → `"1234.568"`) so HA's number formatting has signal
/// without flooding bytes for whole-number readings.
fn format_sensor_value(v: f64) -> String {
    if !v.is_finite() {
        return "unavailable".to_string();
    }
    // f64::Display already drops `.0` for integer-valued floats
    // and uses the shortest round-trip form otherwise. Cap at three
    // decimals to keep the wire size small for fast-path sensors.
    let rounded = (v * 1000.0).round() / 1000.0;
    format!("{rounded}")
}

// -----------------------------------------------------------------------------
// PR-matter-outdoor-temp: Matter cluster attribute → SensorReading
// -----------------------------------------------------------------------------

/// Outcome of parsing a Matter outdoor-temperature MQTT body.
/// Three-way distinction so the caller can log appropriately:
/// - `Reading(°C)` — happy path; emit a `SensorReading`.
/// - `Drop` — body is `null` / non-numeric / out of int16 range;
///   silently dropped (Meross publishes `null` between low-power reads,
///   so this is the common case and must not spam logs).
/// - `OutOfRange` — body parsed but the °C value is outside the
///   configured `[min_celsius, max_celsius]` sanity bounds; caller
///   should `warn!` (rate-limited) because this signals a real sensor
///   issue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatterOutdoorTempParse {
    Reading(f64),
    Drop,
    OutOfRange(f64),
}

/// Parse a Matter `TemperatureMeasurement::MeasuredValue` body
/// (cluster 0x0402, attribute 0) — JSON-encoded signed int in centi-
/// Celsius, valid Matter int16 range `[-27315, 32767]`. Applies the
/// caller-supplied sanity bounds to the resulting °C value.
#[must_use]
pub fn parse_matter_outdoor_temp(
    payload: &[u8],
    min_celsius: f64,
    max_celsius: f64,
) -> MatterOutdoorTempParse {
    let centi: i64 = match serde_json::from_slice::<serde_json::Value>(payload) {
        Ok(serde_json::Value::Number(n)) => match n.as_i64() {
            Some(v) => v,
            None => return MatterOutdoorTempParse::Drop,
        },
        // null, "unavailable", arrays, schema drift — silently drop.
        _ => return MatterOutdoorTempParse::Drop,
    };
    // Matter int16 range. Anything outside indicates corruption /
    // schema drift, not a sensor error — drop silently.
    if !(-27315..=32767).contains(&centi) {
        return MatterOutdoorTempParse::Drop;
    }
    #[allow(clippy::cast_precision_loss)]
    let celsius = centi as f64 / 100.0;
    if !(min_celsius..=max_celsius).contains(&celsius) {
        return MatterOutdoorTempParse::OutOfRange(celsius);
    }
    MatterOutdoorTempParse::Reading(celsius)
}

/// Build the `Event::Sensor` for a parsed Matter outdoor-temperature
/// reading. Separated from the parser so tests can assert the wiring
/// without standing up an MQTT client.
#[must_use]
pub fn matter_outdoor_temp_event(celsius: f64, at: Instant) -> Event {
    Event::Sensor(SensorReading {
        id: SensorId::OutdoorTemperature,
        value: celsius,
        at,
    })
}

/// Decode an incoming MQTT `<root>/knob/<name>/set` or
/// `<root>/writes_enabled/set` message into a core Event::Command.
#[must_use]
pub fn decode_knob_set(topic_root: &str, topic: &str, payload: &[u8]) -> Option<Event> {
    decode_generic(topic_root, topic, payload, "/set", Owner::HaMqtt)
}

/// Decode a retained `<root>/knob/<name>/state` or
/// `<root>/writes_enabled/state` message into a core Event::Command
/// owned by `System` — used during the startup bootstrap phase to
/// seed knobs from retained MQTT state.
#[must_use]
pub fn decode_state_message(topic_root: &str, topic: &str, payload: &[u8]) -> Option<Event> {
    decode_generic(topic_root, topic, payload, "/state", Owner::System)
}

fn decode_generic(
    topic_root: &str,
    topic: &str,
    payload: &[u8],
    suffix: &str,
    owner: Owner,
) -> Option<Event> {
    let stripped = topic.strip_prefix(topic_root)?.strip_prefix('/')?;
    let body = std::str::from_utf8(payload).ok()?.trim();
    let at = Instant::now();

    if stripped == format!("writes_enabled{suffix}") {
        let enabled = match body.to_ascii_lowercase().as_str() {
            "true" | "1" | "on" => true,
            "false" | "0" | "off" => false,
            _ => return None,
        };
        return Some(Event::Command {
            command: Command::KillSwitch(enabled),
            owner,
            at,
        });
    }

    if let Some(rest) = stripped.strip_prefix("knob/") {
        let name = rest.strip_suffix(suffix)?;
        let id = knob_id_from_name(name)?;
        let value = parse_knob_value(id, body)?;
        return Some(Event::Command {
            command: Command::Knob { id, value },
            owner,
            at,
        });
    }

    // Bookkeeping state — only meaningful with the /state suffix during
    // bootstrap, but decoded symmetrically for uniformity.
    if let Some(rest) = stripped.strip_prefix("bookkeeping/") {
        let name = rest.strip_suffix(suffix)?;
        let key = bookkeeping_key_from_name(name)?;
        let value = parse_bookkeeping_value(key, body)?;
        return Some(Event::Command {
            command: Command::Bookkeeping { key, value },
            owner,
            at,
        });
    }

    None
}

fn bookkeeping_key_from_name(name: &str) -> Option<BookkeepingKey> {
    Some(match name {
        "schedule.full-charge.next" => BookkeepingKey::NextFullCharge,
        "battery.soc.above-threshold.date" => BookkeepingKey::AboveSocDate,
        "inverter.ess.state.previous" => BookkeepingKey::PrevEssState,
        _ => return None,
    })
}

fn parse_bookkeeping_value(key: BookkeepingKey, body: &str) -> Option<BookkeepingValue> {
    if body == "null" {
        return Some(BookkeepingValue::Cleared);
    }
    match key {
        BookkeepingKey::NextFullCharge => chrono::NaiveDateTime::parse_from_str(
            body,
            "%Y-%m-%dT%H:%M:%S",
        )
        .ok()
        .map(BookkeepingValue::NaiveDateTime),
        BookkeepingKey::AboveSocDate => chrono::NaiveDate::parse_from_str(body, "%Y-%m-%d")
            .ok()
            .map(BookkeepingValue::NaiveDate),
        BookkeepingKey::PrevEssState => body
            .parse::<i32>()
            .ok()
            .map(|n| BookkeepingValue::OptionalInt(Some(n))),
    }
}

// -----------------------------------------------------------------------------
// Name <-> id
// -----------------------------------------------------------------------------

#[allow(clippy::too_many_lines)]
pub fn knob_name(id: KnobId) -> &'static str {
    match id {
        KnobId::ForceDisableExport => "grid.export.force-disable",
        KnobId::ExportSocThreshold => "battery.soc.threshold.export.forced-value",
        KnobId::DischargeSocTarget => "battery.soc.target.discharge.forced-value",
        KnobId::BatterySocTarget => "battery.soc.target.charge.forced-value",
        KnobId::FullChargeDischargeSocTarget => "battery.soc.target.full-charge.discharge",
        KnobId::FullChargeExportSocThreshold => "battery.soc.threshold.full-charge.export",
        KnobId::DischargeTime => "battery.discharge.time",
        KnobId::DebugFullCharge => "debug.full-charge.mode",
        KnobId::PessimismMultiplierModifier => "forecast.pessimism.modifier",
        KnobId::DisableNightGridDischarge => "grid.night.discharge.disable.forced-value",
        KnobId::ChargeCarBoost => "evcharger.boost.enable",
        // PR-auto-extended-charge: tri-state mode replaces the legacy
        // bool topic `evcharger.extended.enable`.
        KnobId::ChargeCarExtendedMode => "evcharger.extended.mode",
        KnobId::ZappiCurrentTarget => "evcharger.current.target",
        KnobId::ZappiLimit => "evcharger.session.limit",
        KnobId::ZappiEmergencyMargin => "evcharger.current.margin",
        KnobId::GridExportLimitW => "grid.export.limit",
        KnobId::GridImportLimitW => "grid.import.limit",
        KnobId::AllowBatteryToCar => "battery.export.car.allow",
        KnobId::EddiEnableSoc => "eddi.soc.enable",
        KnobId::EddiDisableSoc => "eddi.soc.disable",
        KnobId::EddiDwellS => "eddi.dwell.seconds",
        KnobId::WeathersocWinterTemperatureThreshold => "weathersoc.threshold.winter-temperature",
        KnobId::WeathersocLowEnergyThreshold => "weathersoc.threshold.energy.low",
        KnobId::WeathersocOkEnergyThreshold => "weathersoc.threshold.energy.ok",
        KnobId::WeathersocHighEnergyThreshold => "weathersoc.threshold.energy.high",
        KnobId::WeathersocTooMuchEnergyThreshold => "weathersoc.threshold.energy.too-much",
        KnobId::ForecastDisagreementStrategy => "forecast.disagreement.strategy",
        KnobId::ChargeBatteryExtendedMode => "schedule.extended.charge.mode",
        // PR-gamma-hold-redesign — four mode selectors.
        KnobId::ExportSocThresholdMode => "battery.soc.threshold.export.mode",
        KnobId::DischargeSocTargetMode => "battery.soc.target.discharge.mode",
        KnobId::BatterySocTargetMode => "battery.soc.target.charge.mode",
        KnobId::DisableNightGridDischargeMode => "grid.night.discharge.disable.mode",
        KnobId::InverterSafeDischargeEnable => "inverter.safe-discharge.enable",
        // PR-baseline-forecast: 4 runtime knobs.
        KnobId::BaselineWinterStartMmDd => "forecast.baseline.winter.start.mmdd",
        KnobId::BaselineWinterEndMmDd => "forecast.baseline.winter.end.mmdd",
        KnobId::BaselineWhPerHourWinter => "forecast.baseline.wh-per-hour.winter",
        KnobId::BaselineWhPerHourSummer => "forecast.baseline.wh-per-hour.summer",
        // PR-keep-batteries-charged.
        KnobId::KeepBatteriesChargedDuringFullCharge => "ess.full-charge.keep-batteries-charged",
        KnobId::SunriseSunsetOffsetMin => "ess.full-charge.sunrise-sunset-offset-min",
    }
}

fn knob_id_from_name(n: &str) -> Option<KnobId> {
    Some(match n {
        "grid.export.force-disable" => KnobId::ForceDisableExport,
        "battery.soc.threshold.export.forced-value" => KnobId::ExportSocThreshold,
        "battery.soc.target.discharge.forced-value" => KnobId::DischargeSocTarget,
        "battery.soc.target.charge.forced-value" => KnobId::BatterySocTarget,
        "battery.soc.target.full-charge.discharge" => KnobId::FullChargeDischargeSocTarget,
        "battery.soc.threshold.full-charge.export" => KnobId::FullChargeExportSocThreshold,
        "battery.discharge.time" => KnobId::DischargeTime,
        "debug.full-charge.mode" => KnobId::DebugFullCharge,
        "forecast.pessimism.modifier" => KnobId::PessimismMultiplierModifier,
        "grid.night.discharge.disable.forced-value" => KnobId::DisableNightGridDischarge,
        "evcharger.boost.enable" => KnobId::ChargeCarBoost,
        // PR-auto-extended-charge.
        "evcharger.extended.mode" => KnobId::ChargeCarExtendedMode,
        "evcharger.current.target" => KnobId::ZappiCurrentTarget,
        "evcharger.session.limit" => KnobId::ZappiLimit,
        "evcharger.current.margin" => KnobId::ZappiEmergencyMargin,
        "grid.export.limit" => KnobId::GridExportLimitW,
        "grid.import.limit" => KnobId::GridImportLimitW,
        "battery.export.car.allow" => KnobId::AllowBatteryToCar,
        "eddi.soc.enable" => KnobId::EddiEnableSoc,
        "eddi.soc.disable" => KnobId::EddiDisableSoc,
        "eddi.dwell.seconds" => KnobId::EddiDwellS,
        "weathersoc.threshold.winter-temperature" => KnobId::WeathersocWinterTemperatureThreshold,
        "weathersoc.threshold.energy.low" => KnobId::WeathersocLowEnergyThreshold,
        "weathersoc.threshold.energy.ok" => KnobId::WeathersocOkEnergyThreshold,
        "weathersoc.threshold.energy.high" => KnobId::WeathersocHighEnergyThreshold,
        "weathersoc.threshold.energy.too-much" => KnobId::WeathersocTooMuchEnergyThreshold,
        "forecast.disagreement.strategy" => KnobId::ForecastDisagreementStrategy,
        "schedule.extended.charge.mode" => KnobId::ChargeBatteryExtendedMode,
        // PR-gamma-hold-redesign — four mode selectors.
        "battery.soc.threshold.export.mode" => KnobId::ExportSocThresholdMode,
        "battery.soc.target.discharge.mode" => KnobId::DischargeSocTargetMode,
        "battery.soc.target.charge.mode" => KnobId::BatterySocTargetMode,
        "grid.night.discharge.disable.mode" => KnobId::DisableNightGridDischargeMode,
        "inverter.safe-discharge.enable" => KnobId::InverterSafeDischargeEnable,
        // PR-baseline-forecast.
        "forecast.baseline.winter.start.mmdd" => KnobId::BaselineWinterStartMmDd,
        "forecast.baseline.winter.end.mmdd" => KnobId::BaselineWinterEndMmDd,
        "forecast.baseline.wh-per-hour.winter" => KnobId::BaselineWhPerHourWinter,
        "forecast.baseline.wh-per-hour.summer" => KnobId::BaselineWhPerHourSummer,
        // PR-keep-batteries-charged.
        "ess.full-charge.keep-batteries-charged" => KnobId::KeepBatteriesChargedDuringFullCharge,
        "ess.full-charge.sunrise-sunset-offset-min" => KnobId::SunriseSunsetOffsetMin,
        _ => return None,
    })
}

fn actuated_name(id: ActuatedId) -> &'static str {
    match id {
        ActuatedId::GridSetpoint => "grid.setpoint",
        ActuatedId::InputCurrentLimit => "inverter.input.current-limit",
        ActuatedId::ZappiMode => "evcharger.mode.target",
        ActuatedId::EddiMode => "eddi.mode.target",
        ActuatedId::Schedule0 => "schedule.0",
        ActuatedId::Schedule1 => "schedule.1",
        // PR-keep-batteries-charged.
        ActuatedId::EssStateTarget => "ess.state.target",
    }
}

fn bookkeeping_name(k: BookkeepingKey) -> &'static str {
    match k {
        BookkeepingKey::NextFullCharge => "schedule.full-charge.next",
        BookkeepingKey::AboveSocDate => "battery.soc.above-threshold.date",
        BookkeepingKey::PrevEssState => "inverter.ess.state.previous",
    }
}

/// PR-ha-discovery-expand: stable `snake_case` topic-tail name for each
/// sensor. Mirrors the `Sensors`-struct field names in
/// `crates/core/src/world.rs`. See the topic taxonomy comment at the
/// top of `crates/shell/src/mqtt/discovery.rs`.
pub(crate) fn sensor_name(id: SensorId) -> &'static str {
    match id {
        SensorId::BatterySoc => "battery.soc",
        SensorId::BatterySoh => "battery.soh",
        SensorId::BatteryInstalledCapacity => "battery.capacity.installed",
        SensorId::BatteryDcPower => "battery.power.dc",
        SensorId::MpptPower0 => "solar.mppt.0.power",
        SensorId::MpptPower1 => "solar.mppt.1.power",
        SensorId::SoltaroPower => "solar.soltaro.power",
        SensorId::PowerConsumption => "house.power.consumption",
        SensorId::GridPower => "grid.power",
        SensorId::GridVoltage => "grid.voltage",
        SensorId::GridCurrent => "grid.current",
        SensorId::ConsumptionCurrent => "house.current.consumption",
        SensorId::OffgridPower => "inverter.offgrid.power",
        SensorId::OffgridCurrent => "inverter.offgrid.current",
        SensorId::VebusInputCurrent => "inverter.input.current",
        SensorId::EvchargerAcPower => "evcharger.ac.power",
        SensorId::EvchargerAcCurrent => "evcharger.ac.current",
        SensorId::EssState => "inverter.ess.state",
        SensorId::OutdoorTemperature => "weather.temperature.outdoor",
        SensorId::SessionKwh => "evcharger.session.energy",
        // PR-ev-soc-sensor.
        SensorId::EvSoc => "ev.soc",
        // PR-auto-extended-charge.
        SensorId::EvChargeTarget => "ev.charge.target",
        // PR-AS-C: actuated-mirror SensorId variants do not have a
        // sensor wire surface — they are surfaced via the actuated
        // entity table (`PublishPayload::ActuatedPhase`). Both call
        // sites (`encode_publish_payload` for `PublishPayload::Sensor`,
        // and `discovery::publish_sensors`) are unreachable for these
        // variants because `SensorBroadcastCore` filters them out
        // before any `Publish(Sensor{...})` is produced.
        SensorId::GridSetpointActual
        | SensorId::InputCurrentLimitActual
        | SensorId::Schedule0StartActual
        | SensorId::Schedule0DurationActual
        | SensorId::Schedule0SocActual
        | SensorId::Schedule0DaysActual
        | SensorId::Schedule0AllowDischargeActual
        | SensorId::Schedule1StartActual
        | SensorId::Schedule1DurationActual
        | SensorId::Schedule1SocActual
        | SensorId::Schedule1DaysActual
        | SensorId::Schedule1AllowDischargeActual => unreachable!(
            "actuated-mirror SensorId {id:?} reached sensor_name — caller \
             must filter via id.actuated_id().is_some()"
        ),
    }
}

// -----------------------------------------------------------------------------
// Value encode / decode
// -----------------------------------------------------------------------------

fn encode_knob_value(v: KnobValue) -> String {
    match v {
        KnobValue::Bool(b) => b.to_string(),
        KnobValue::Float(f) => f.to_string(),
        KnobValue::Uint32(u) => u.to_string(),
        KnobValue::DischargeTime(DischargeTime::At0200) => "02:00".to_string(),
        KnobValue::DischargeTime(DischargeTime::At2300) => "23:00".to_string(),
        KnobValue::DebugFullCharge(DebugFullCharge::Forbid) => "forbid".to_string(),
        KnobValue::DebugFullCharge(DebugFullCharge::Force) => "force".to_string(),
        KnobValue::DebugFullCharge(DebugFullCharge::Auto) => "auto".to_string(),
        KnobValue::ForecastDisagreementStrategy(s) => match s {
            ForecastDisagreementStrategy::Max => "max".to_string(),
            ForecastDisagreementStrategy::Min => "min".to_string(),
            ForecastDisagreementStrategy::Mean => "mean".to_string(),
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean => {
                "solcast_if_available_else_mean".to_string()
            }
        },
        KnobValue::ChargeBatteryExtendedMode(m) => match m {
            ChargeBatteryExtendedMode::Auto => "auto".to_string(),
            ChargeBatteryExtendedMode::Forced => "forced".to_string(),
            ChargeBatteryExtendedMode::Disabled => "disabled".to_string(),
        },
        // PR-auto-extended-charge.
        KnobValue::ExtendedChargeMode(m) => match m {
            ExtendedChargeMode::Auto => "auto".to_string(),
            ExtendedChargeMode::Forced => "forced".to_string(),
            ExtendedChargeMode::Disabled => "disabled".to_string(),
        },
        // PR-gamma-hold-redesign.
        KnobValue::Mode(m) => match m {
            Mode::Weather => "weather".to_string(),
            Mode::Forced => "forced".to_string(),
        },
    }
}

/// Acceptable inclusive `[min, max]` range for float/int knobs at the
/// MQTT parse boundary. Values outside are rejected with a warn! and
/// the retained message is dropped (no apply). Matches the HA discovery
/// entity constraints in `mqtt::discovery::knob_schemas`.
/// Per-knob (min, max) range — the ONE source of truth.
/// Both `parse_knob_value` (bounds-check on MQTT ingest) and
/// `knob_schemas` (HA discovery min/max) consume this. PR-06-D01:
/// previously discovery.rs had its own parallel table and drift was
/// silent — moved to a single function here so that any future range
/// change updates both ingress and egress atomically.
// A-14: `ZappiLimit` shares the numeric range `(0.0, 100.0)` with the
// SoC-percentage knobs, but is semantically kWh (per-session EV charge
// ceiling), not %. Keeping the arm separate documents the unit
// distinction at the site of truth; clippy's `match_same_arms` is
// silenced intentionally for that reason.
#[allow(clippy::match_same_arms)]
pub(crate) fn knob_range(id: KnobId) -> Option<(f64, f64)> {
    Some(match id {
        // Percentages (0..100)
        KnobId::ExportSocThreshold
        | KnobId::DischargeSocTarget
        | KnobId::BatterySocTarget
        | KnobId::FullChargeDischargeSocTarget
        | KnobId::FullChargeExportSocThreshold => (0.0, 100.0),

        // A-14: zappi_limit is kWh now (per-session EV charge ceiling).
        // Range 0..100 kWh fits a typical EV full-charge plus some
        // headroom; step is 0.5 kWh (see discovery.rs).
        KnobId::ZappiLimit => (0.0, 100.0),
        KnobId::EddiEnableSoc | KnobId::EddiDisableSoc => (50.0, 100.0),

        // Currents (A)
        KnobId::ZappiCurrentTarget => (6.0, 32.0),
        KnobId::ZappiEmergencyMargin => (0.0, 10.0),

        // Temperature (°C)
        KnobId::WeathersocWinterTemperatureThreshold => (-30.0, 40.0),

        // Energy thresholds (kWh). A-44: SPEC §3.6 is 0..1000; previously
        // the knob_range and HA discovery both said 500 — the ceiling is
        // unreachable for a ~15 kWp system anyway, but consistency with
        // SPEC matters for users reading the dashboard's knob metadata.
        KnobId::WeathersocLowEnergyThreshold
        | KnobId::WeathersocOkEnergyThreshold
        | KnobId::WeathersocHighEnergyThreshold
        | KnobId::WeathersocTooMuchEnergyThreshold => (0.0, 1000.0),

        // Power (W) — PR-hardware-config: per-direction ceilings
        // sourced from HardwareParams, so the export and import caps
        // can differ. Defaults: export 6000 W (G99 export
        // authorisation), import 13 000 W (MultiPlus continuous import).
        KnobId::GridExportLimitW => (0.0, f64::from(hardware_params().grid_export_knob_max_w)),
        KnobId::GridImportLimitW => (0.0, f64::from(hardware_params().grid_import_knob_max_w)),

        // Time (s)
        KnobId::EddiDwellS => (0.0, 3600.0),

        // PR-keep-batteries-charged: minutes inset from sunrise/sunset.
        // Cap at 8 h — anything larger collapses the daylight window
        // even at the summer solstice; the controller falls back to "no
        // write" via the empty-window guard if the operator goes
        // pathological.
        KnobId::SunriseSunsetOffsetMin => (0.0, 480.0),

        // PR-baseline-forecast: MMDD-encoded dates. Range covers any
        // legal MMDD literal (101 = Jan 1, 1231 = Dec 31). Day-of-month
        // legality (e.g. rejecting 230 / 431) is enforced at use-site
        // in the baseline scheduler — this range is the wire-level
        // sanity floor / ceiling for the HA discovery slider.
        KnobId::BaselineWinterStartMmDd | KnobId::BaselineWinterEndMmDd => (101.0, 1231.0),

        // PR-baseline-forecast: per-daylight-hour Wh constants. Upper
        // bound generous — a 10 kWp array at peak yields ~10 kWh in a
        // single fully-clear midday hour, but the baseline is a
        // pessimistic average so 10000 Wh/h is hard ceiling.
        KnobId::BaselineWhPerHourWinter | KnobId::BaselineWhPerHourSummer => (0.0, 10000.0),

        // Multiplier
        KnobId::PessimismMultiplierModifier => (0.0, 2.0),

        // Enums + bools don't use this table.
        KnobId::ForceDisableExport
        | KnobId::DisableNightGridDischarge
        | KnobId::ChargeCarBoost
        // PR-auto-extended-charge — enum, no range.
        | KnobId::ChargeCarExtendedMode
        | KnobId::AllowBatteryToCar
        | KnobId::DischargeTime
        | KnobId::DebugFullCharge
        | KnobId::ForecastDisagreementStrategy
        | KnobId::ChargeBatteryExtendedMode
        // PR-gamma-hold-redesign — mode selectors are enums.
        | KnobId::ExportSocThresholdMode
        | KnobId::DischargeSocTargetMode
        | KnobId::BatterySocTargetMode
        | KnobId::DisableNightGridDischargeMode
        // bool — no range
        | KnobId::InverterSafeDischargeEnable
        // PR-keep-batteries-charged — bool, no range.
        | KnobId::KeepBatteriesChargedDuringFullCharge => return None,
    })
}

/// Parse a float knob body, rejecting non-finite values and values
/// outside the HA-advertised range. Returns `None` (and logs) on
/// rejection.
fn parse_ranged_float(id: KnobId, body: &str) -> Option<f64> {
    // PR-06-D02: split parse and finiteness so NaN/Inf are logged
    // before being dropped (rather than silently discarded via
    // `Option::filter`).
    let parsed = body.parse::<f64>().ok()?;
    if !parsed.is_finite() {
        warn!(
            id = ?id,
            value = %body,
            "knob non-finite; dropped"
        );
        return None;
    }
    if let Some((min, max)) = knob_range(id) {
        if parsed < min || parsed > max {
            // PR-06-D03: wording — this path runs for live HaMqtt
            // writes too, not just retained knob replay.
            warn!(
                id = ?id,
                value = %body,
                min,
                max,
                "knob value out of range; dropped"
            );
            return None;
        }
    }
    Some(parsed)
}

/// Parse a u32 knob body, rejecting values outside the HA-advertised
/// range. Returns `None` (and logs) on rejection.
fn parse_ranged_u32(id: KnobId, body: &str) -> Option<u32> {
    let parsed = body.parse::<u32>().ok()?;
    if let Some((min, max)) = knob_range(id) {
        let as_f = f64::from(parsed);
        if as_f < min || as_f > max {
            // PR-06-D03: wording — shared path (live + retained).
            warn!(
                id = ?id,
                value = %body,
                min,
                max,
                "knob value out of range; dropped"
            );
            return None;
        }
    }
    Some(parsed)
}

fn parse_knob_value(id: KnobId, body: &str) -> Option<KnobValue> {
    // Map each KnobId to its expected value shape.
    match id {
        KnobId::ForceDisableExport
        | KnobId::DisableNightGridDischarge
        | KnobId::ChargeCarBoost
        | KnobId::AllowBatteryToCar
        | KnobId::InverterSafeDischargeEnable
        // PR-keep-batteries-charged — bool.
        | KnobId::KeepBatteriesChargedDuringFullCharge => parse_bool(body).map(KnobValue::Bool),
        KnobId::ExportSocThreshold
        | KnobId::DischargeSocTarget
        | KnobId::BatterySocTarget
        | KnobId::FullChargeDischargeSocTarget
        | KnobId::FullChargeExportSocThreshold
        | KnobId::PessimismMultiplierModifier
        | KnobId::ZappiCurrentTarget
        | KnobId::ZappiLimit
        | KnobId::ZappiEmergencyMargin
        | KnobId::EddiEnableSoc
        | KnobId::EddiDisableSoc
        | KnobId::WeathersocWinterTemperatureThreshold
        | KnobId::WeathersocLowEnergyThreshold
        | KnobId::WeathersocOkEnergyThreshold
        | KnobId::WeathersocHighEnergyThreshold
        | KnobId::WeathersocTooMuchEnergyThreshold
        // PR-baseline-forecast: per-hour Wh constants are floats.
        | KnobId::BaselineWhPerHourWinter
        | KnobId::BaselineWhPerHourSummer => {
            parse_ranged_float(id, body).map(KnobValue::Float)
        }
        KnobId::GridExportLimitW
        | KnobId::GridImportLimitW
        | KnobId::EddiDwellS
        // PR-baseline-forecast: MMDD encoded as Uint32.
        | KnobId::BaselineWinterStartMmDd
        | KnobId::BaselineWinterEndMmDd
        // PR-keep-batteries-charged — minutes encoded as Uint32.
        | KnobId::SunriseSunsetOffsetMin => {
            parse_ranged_u32(id, body).map(KnobValue::Uint32)
        }
        KnobId::DischargeTime => match body.trim() {
            "02:00" | "02:00:00" => Some(KnobValue::DischargeTime(DischargeTime::At0200)),
            "23:00" | "23:00:00" => Some(KnobValue::DischargeTime(DischargeTime::At2300)),
            _ => {
                warn!("unknown DischargeTime value: {body}");
                None
            }
        },
        KnobId::DebugFullCharge => match body {
            "forbid" => Some(KnobValue::DebugFullCharge(DebugFullCharge::Forbid)),
            "force" => Some(KnobValue::DebugFullCharge(DebugFullCharge::Force)),
            "auto" | "none" => Some(KnobValue::DebugFullCharge(DebugFullCharge::Auto)),
            _ => None,
        },
        KnobId::ForecastDisagreementStrategy => match body {
            "max" => Some(KnobValue::ForecastDisagreementStrategy(
                ForecastDisagreementStrategy::Max,
            )),
            "min" => Some(KnobValue::ForecastDisagreementStrategy(
                ForecastDisagreementStrategy::Min,
            )),
            "mean" => Some(KnobValue::ForecastDisagreementStrategy(
                ForecastDisagreementStrategy::Mean,
            )),
            "solcast_if_available_else_mean" => Some(KnobValue::ForecastDisagreementStrategy(
                ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
            )),
            _ => None,
        },
        KnobId::ChargeBatteryExtendedMode => match body {
            "auto" => Some(KnobValue::ChargeBatteryExtendedMode(
                ChargeBatteryExtendedMode::Auto,
            )),
            "forced" => Some(KnobValue::ChargeBatteryExtendedMode(
                ChargeBatteryExtendedMode::Forced,
            )),
            "disabled" => Some(KnobValue::ChargeBatteryExtendedMode(
                ChargeBatteryExtendedMode::Disabled,
            )),
            _ => None,
        },
        // PR-auto-extended-charge.
        KnobId::ChargeCarExtendedMode => match body {
            "auto" => Some(KnobValue::ExtendedChargeMode(ExtendedChargeMode::Auto)),
            "forced" => Some(KnobValue::ExtendedChargeMode(ExtendedChargeMode::Forced)),
            "disabled" => Some(KnobValue::ExtendedChargeMode(ExtendedChargeMode::Disabled)),
            _ => {
                warn!("unknown ExtendedChargeMode value: {body}");
                None
            }
        },
        // PR-gamma-hold-redesign.
        KnobId::ExportSocThresholdMode
        | KnobId::DischargeSocTargetMode
        | KnobId::BatterySocTargetMode
        | KnobId::DisableNightGridDischargeMode => match body {
            "weather" => Some(KnobValue::Mode(Mode::Weather)),
            "forced" => Some(KnobValue::Mode(Mode::Forced)),
            _ => {
                warn!("unknown Mode value: {body}");
                None
            }
        },
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "true" | "1" | "on" => Some(true),
        "false" | "0" | "off" => Some(false),
        _ => None,
    }
}

fn encode_bookkeeping_value(v: BookkeepingValue) -> String {
    match v {
        BookkeepingValue::NaiveDateTime(dt) => dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        BookkeepingValue::NaiveDate(d) => d.format("%Y-%m-%d").to_string(),
        BookkeepingValue::OptionalInt(None) | BookkeepingValue::Cleared => "null".to_string(),
        BookkeepingValue::OptionalInt(Some(n)) => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use victron_controller_core::types::BookkeepingId;

    // ------------------------------------------------------------------
    // Knob → wire
    // ------------------------------------------------------------------

    #[test]
    fn encode_bool_knob() {
        let p = PublishPayload::Knob {
            id: KnobId::ForceDisableExport,
            value: KnobValue::Bool(true),
        };
        let (t, b, r) = encode_publish_payload(&p).unwrap();
        assert_eq!(t, "knob/grid.export.force-disable/state");
        assert_eq!(b, "true");
        assert!(r);
    }

    #[test]
    fn encode_float_knob() {
        let (t, b, _) = encode_publish_payload(&PublishPayload::Knob {
            id: KnobId::ExportSocThreshold,
            value: KnobValue::Float(80.0),
        })
        .unwrap();
        assert_eq!(t, "knob/battery.soc.threshold.export.forced-value/state");
        assert_eq!(b, "80");
    }

    #[test]
    fn encode_discharge_time() {
        let (_, b, _) = encode_publish_payload(&PublishPayload::Knob {
            id: KnobId::DischargeTime,
            value: KnobValue::DischargeTime(DischargeTime::At2300),
        })
        .unwrap();
        assert_eq!(b, "23:00");
    }

    #[test]
    fn encode_kill_switch() {
        let (t, b, r) = encode_publish_payload(&PublishPayload::KillSwitch(false)).unwrap();
        assert_eq!(t, "writes_enabled/state");
        assert_eq!(b, "false");
        assert!(r);
    }

    #[test]
    fn encode_actuated_phase() {
        let p = PublishPayload::ActuatedPhase {
            id: victron_controller_core::types::ActuatedId::GridSetpoint,
            phase: victron_controller_core::TargetPhase::Confirmed,
        };
        let (t, b, _) = encode_publish_payload(&p).unwrap();
        assert_eq!(t, "entity/grid.setpoint/phase");
        assert!(b.contains("Confirmed"));
    }

    // ------------------------------------------------------------------
    // Wire → Event::Command
    // ------------------------------------------------------------------

    #[test]
    fn decode_bool_knob_set() {
        let e = decode_knob_set(
            "victron-controller",
            "victron-controller/knob/grid.export.force-disable/set",
            b"true",
        )
        .unwrap();
        match e {
            Event::Command {
                command:
                    Command::Knob {
                        id: KnobId::ForceDisableExport,
                        value: KnobValue::Bool(true),
                    },
                owner: Owner::HaMqtt,
                ..
            } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_float_knob_set() {
        let e = decode_knob_set(
            "victron-controller",
            "victron-controller/knob/battery.soc.threshold.export.forced-value/set",
            b"67.5",
        )
        .unwrap();
        match e {
            Event::Command {
                command:
                    Command::Knob {
                        value: KnobValue::Float(f),
                        ..
                    },
                ..
            } => assert!((f - 67.5).abs() < f64::EPSILON),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_kill_switch_off() {
        let e = decode_knob_set(
            "victron-controller",
            "victron-controller/writes_enabled/set",
            b"off",
        )
        .unwrap();
        match e {
            Event::Command {
                command: Command::KillSwitch(false),
                ..
            } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_unknown_topic_returns_none() {
        assert!(decode_knob_set(
            "victron-controller",
            "victron-controller/not-a-topic/set",
            b"true"
        )
        .is_none());
    }

    #[test]
    fn decode_unknown_knob_returns_none() {
        assert!(decode_knob_set(
            "victron-controller",
            "victron-controller/knob/bogus_knob/set",
            b"true"
        )
        .is_none());
    }

    #[test]
    fn decode_bad_bool_payload_returns_none() {
        assert!(decode_knob_set(
            "victron-controller",
            "victron-controller/knob/grid.export.force-disable/set",
            b"maybe"
        )
        .is_none());
    }

    #[test]
    fn decode_wrong_topic_root_returns_none() {
        // Root prefix mismatch.
        assert!(decode_knob_set(
            "victron-controller",
            "other-root/knob/grid.export.force-disable/set",
            b"true"
        )
        .is_none());
    }

    // ------------------------------------------------------------------
    // decode_state_message (bootstrap path)
    // ------------------------------------------------------------------

    #[test]
    fn decode_state_knob_uses_system_owner() {
        let e = decode_state_message(
            "victron-controller",
            "victron-controller/knob/battery.soc.threshold.export.forced-value/state",
            b"67.0",
        )
        .unwrap();
        match e {
            Event::Command {
                command:
                    Command::Knob {
                        id: KnobId::ExportSocThreshold,
                        value: KnobValue::Float(f),
                    },
                owner: Owner::System,
                ..
            } => assert!((f - 67.0).abs() < f64::EPSILON),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_state_kill_switch_uses_system_owner() {
        let e = decode_state_message(
            "victron-controller",
            "victron-controller/writes_enabled/state",
            b"false",
        )
        .unwrap();
        assert!(matches!(
            e,
            Event::Command {
                command: Command::KillSwitch(false),
                owner: Owner::System,
                ..
            }
        ));
    }

    #[test]
    fn decode_state_rejects_set_suffix() {
        // State decoder must not match /set topics.
        assert!(decode_state_message(
            "victron-controller",
            "victron-controller/knob/grid.export.force-disable/set",
            b"true"
        )
        .is_none());
    }

    #[test]
    fn decode_knob_set_rejects_state_suffix() {
        // Symmetrically: /set decoder must not match /state topics.
        assert!(decode_knob_set(
            "victron-controller",
            "victron-controller/knob/grid.export.force-disable/state",
            b"true"
        )
        .is_none());
    }

    // ------------------------------------------------------------------
    // Bookkeeping restoration
    // ------------------------------------------------------------------

    #[test]
    fn decode_bookkeeping_next_full_charge_datetime() {
        let e = decode_state_message(
            "victron-controller",
            "victron-controller/bookkeeping/schedule.full-charge.next/state",
            b"2026-04-26T17:00:00",
        )
        .unwrap();
        match e {
            Event::Command {
                command:
                    Command::Bookkeeping {
                        key: BookkeepingKey::NextFullCharge,
                        value: BookkeepingValue::NaiveDateTime(dt),
                    },
                owner: Owner::System,
                ..
            } => {
                assert_eq!(dt.to_string(), "2026-04-26 17:00:00");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_bookkeeping_above_soc_date() {
        let e = decode_state_message(
            "victron-controller",
            "victron-controller/bookkeeping/battery.soc.above-threshold.date/state",
            b"2026-04-21",
        )
        .unwrap();
        match e {
            Event::Command {
                command:
                    Command::Bookkeeping {
                        key: BookkeepingKey::AboveSocDate,
                        value: BookkeepingValue::NaiveDate(d),
                    },
                ..
            } => {
                assert_eq!(d.to_string(), "2026-04-21");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn decode_bookkeeping_prev_ess_state_int() {
        let e = decode_state_message(
            "victron-controller",
            "victron-controller/bookkeeping/inverter.ess.state.previous/state",
            b"10",
        )
        .unwrap();
        assert!(matches!(
            e,
            Event::Command {
                command: Command::Bookkeeping {
                    key: BookkeepingKey::PrevEssState,
                    value: BookkeepingValue::OptionalInt(Some(10)),
                },
                ..
            }
        ));
    }

    #[test]
    fn decode_bookkeeping_null_is_cleared() {
        let e = decode_state_message(
            "victron-controller",
            "victron-controller/bookkeeping/schedule.full-charge.next/state",
            b"null",
        )
        .unwrap();
        assert!(matches!(
            e,
            Event::Command {
                command: Command::Bookkeeping {
                    value: BookkeepingValue::Cleared,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn decode_bookkeeping_bad_date_returns_none() {
        assert!(decode_state_message(
            "victron-controller",
            "victron-controller/bookkeeping/battery.soc.above-threshold.date/state",
            b"nope"
        )
        .is_none());
    }

    // ------------------------------------------------------------------
    // parse_knob_value boundary validation (A-08)
    // ------------------------------------------------------------------

    #[test]
    fn parse_knob_value_rejects_nan_inf_out_of_range() {
        // (id, body) pairs that must all be rejected. Chosen to exercise
        // the main float / int knob ranges from `knob_range`.
        let cases: &[(KnobId, &str)] = &[
            // Non-finite
            (KnobId::ExportSocThreshold, "NaN"),
            (KnobId::ExportSocThreshold, "inf"),
            (KnobId::ExportSocThreshold, "-inf"),
            (KnobId::PessimismMultiplierModifier, "NaN"),
            // Percentage out-of-range
            (KnobId::ExportSocThreshold, "101"),
            (KnobId::ExportSocThreshold, "-1"),
            (KnobId::DischargeSocTarget, "9999"),
            // A-14: ZappiLimit is kWh now (HA advertises 0..100 kWh).
            // Negatives and > 100 must reject.
            (KnobId::ZappiLimit, "-0.1"),
            (KnobId::ZappiLimit, "101"),
            // Eddi soc (HA advertises 50..100)
            (KnobId::EddiEnableSoc, "49"),
            (KnobId::EddiDisableSoc, "101"),
            // Current
            (KnobId::ZappiCurrentTarget, "5"),
            (KnobId::ZappiCurrentTarget, "33"),
            (KnobId::ZappiEmergencyMargin, "11"),
            // Temperature
            (KnobId::WeathersocWinterTemperatureThreshold, "-31"),
            (KnobId::WeathersocWinterTemperatureThreshold, "41"),
            // Energy (A-44: range widened 500 → 1000 to match SPEC §3.6)
            (KnobId::WeathersocLowEnergyThreshold, "-1"),
            (KnobId::WeathersocTooMuchEnergyThreshold, "1001"),
            // Pessimism multiplier
            (KnobId::PessimismMultiplierModifier, "-0.1"),
            (KnobId::PessimismMultiplierModifier, "2.01"),
            // Power (u32) — PR-hardware-config: per-direction
            // ceilings now (export 6000, import 13000).
            (KnobId::GridExportLimitW, "6001"),
            (KnobId::GridImportLimitW, "13001"),
            // Dwell (u32)
            (KnobId::EddiDwellS, "3601"),
        ];
        for (id, body) in cases {
            assert!(
                parse_knob_value(*id, body).is_none(),
                "expected reject for id={id:?} body={body:?}"
            );
        }
    }

    #[test]
    fn parse_knob_value_accepts_in_range() {
        // Sanity: valid values still parse.
        assert!(matches!(
            parse_knob_value(KnobId::ExportSocThreshold, "80"),
            Some(KnobValue::Float(f)) if (f - 80.0).abs() < f64::EPSILON
        ));
        assert!(matches!(
            parse_knob_value(KnobId::ZappiCurrentTarget, "16"),
            Some(KnobValue::Float(_))
        ));
        assert!(matches!(
            parse_knob_value(KnobId::GridExportLimitW, "5000"),
            Some(KnobValue::Uint32(5000))
        ));
        assert!(matches!(
            parse_knob_value(KnobId::EddiDwellS, "0"),
            Some(KnobValue::Uint32(0))
        ));
    }

    #[test]
    fn parse_knob_value_export_soc_threshold_9999_rejected() {
        // Explicit named case called out in PR-06 scope.
        assert!(parse_knob_value(KnobId::ExportSocThreshold, "9999").is_none());
    }

    #[test]
    fn parse_knob_value_zappi_limit_kwh_round_trip() {
        // A-14: ZappiLimit wire value is kWh. A retained "50" must
        // decode as 50.0 kWh (not a percentage, and not rejected).
        match parse_knob_value(KnobId::ZappiLimit, "50") {
            Some(KnobValue::Float(v)) => {
                assert!((v - 50.0).abs() < f64::EPSILON, "kWh round-trip lost precision: {v}");
            }
            other => panic!("expected Float(50.0) for 50 kWh, got {other:?}"),
        }
        // Fractional kWh (step is 0.5) must also round-trip.
        match parse_knob_value(KnobId::ZappiLimit, "7.5") {
            Some(KnobValue::Float(v)) => {
                assert!((v - 7.5).abs() < f64::EPSILON);
            }
            other => panic!("expected Float(7.5) for 7.5 kWh, got {other:?}"),
        }
    }

    #[test]
    fn parse_knob_value_accepts_exact_boundaries() {
        // PR-06-D04: ensure min and max bounds are inclusive (`>=` /
        // `<=`) and an off-by-one flip to `>` / `<` would be caught.
        // Each case covers a different knob range from knob_range().
        let cases: &[(KnobId, &str)] = &[
            // Percentage rails
            (KnobId::ExportSocThreshold, "0"),
            (KnobId::ExportSocThreshold, "100"),
            (KnobId::DischargeSocTarget, "0"),
            (KnobId::DischargeSocTarget, "100"),
            // A-14: Zappi limit is kWh (0..100 inclusive).
            (KnobId::ZappiLimit, "0"),
            (KnobId::ZappiLimit, "100"),
            // Eddi soc (50..100 inclusive)
            (KnobId::EddiEnableSoc, "50"),
            (KnobId::EddiEnableSoc, "100"),
            // Temperature rails
            (KnobId::WeathersocWinterTemperatureThreshold, "-30"),
            (KnobId::WeathersocWinterTemperatureThreshold, "40"),
            // Energy rails (PR-weather-soc-range widened to 1000)
            (KnobId::WeathersocLowEnergyThreshold, "0"),
            (KnobId::WeathersocLowEnergyThreshold, "1000"),
            // Pessimism multiplier
            (KnobId::PessimismMultiplierModifier, "0"),
            (KnobId::PessimismMultiplierModifier, "2"),
            // Current bounds
            (KnobId::ZappiCurrentTarget, "6"),
            (KnobId::ZappiCurrentTarget, "32"),
            (KnobId::ZappiEmergencyMargin, "0"),
            (KnobId::ZappiEmergencyMargin, "10"),
            // Power caps — PR-hardware-config: export 0..6000,
            // import 0..13000 by default.
            (KnobId::GridExportLimitW, "0"),
            (KnobId::GridExportLimitW, "6000"),
            (KnobId::GridImportLimitW, "0"),
            (KnobId::GridImportLimitW, "13000"),
            // Time
            (KnobId::EddiDwellS, "0"),
            (KnobId::EddiDwellS, "3600"),
        ];
        for (id, body) in cases {
            assert!(
                parse_knob_value(*id, body).is_some(),
                "exact-boundary value {body:?} rejected for {id:?}"
            );
        }
    }

    // ------------------------------------------------------------------
    // PR-ha-discovery-expand: Sensor / BookkeepingNumeric / BookkeepingBool
    // ------------------------------------------------------------------

    #[test]
    fn encode_sensor_stale_is_unavailable() {
        let p = PublishPayload::Sensor {
            id: SensorId::BatterySoc,
            value: Some(75.0),
            freshness: Freshness::Stale,
        };
        let (t, b, r) = encode_publish_payload(&p).unwrap();
        assert_eq!(t, "sensor/battery.soc/state");
        assert_eq!(b, "unavailable");
        assert!(r);
    }

    #[test]
    fn encode_sensor_unknown_is_unavailable() {
        let p = PublishPayload::Sensor {
            id: SensorId::GridPower,
            value: None,
            freshness: Freshness::Unknown,
        };
        let (_, b, _) = encode_publish_payload(&p).unwrap();
        assert_eq!(b, "unavailable");
    }

    #[test]
    fn encode_sensor_fresh_emits_numeric() {
        let p = PublishPayload::Sensor {
            id: SensorId::OutdoorTemperature,
            value: Some(42.5),
            freshness: Freshness::Fresh,
        };
        let (t, b, _) = encode_publish_payload(&p).unwrap();
        assert_eq!(t, "sensor/weather.temperature.outdoor/state");
        assert_eq!(b, "42.5");
    }

    #[test]
    fn encode_sensor_fresh_integer_drops_zero() {
        // f64::Display drops `.0` for integer-valued floats so HA sees
        // a clean "75" rather than "75.000".
        let p = PublishPayload::Sensor {
            id: SensorId::BatterySoc,
            value: Some(75.0),
            freshness: Freshness::Fresh,
        };
        let (_, b, _) = encode_publish_payload(&p).unwrap();
        assert_eq!(b, "75");
    }

    #[test]
    fn encode_bookkeeping_bool_true_false() {
        let (t, b, r) = encode_publish_payload(&PublishPayload::BookkeepingBool {
            id: BookkeepingId::ZappiActive,
            value: true,
        })
        .unwrap();
        assert_eq!(t, "bookkeeping/evcharger.active/state");
        assert_eq!(b, "true");
        assert!(r);

        let (_, b, _) = encode_publish_payload(&PublishPayload::BookkeepingBool {
            id: BookkeepingId::ChargeToFullRequired,
            value: false,
        })
        .unwrap();
        assert_eq!(b, "false");
    }

    #[test]
    fn encode_bookkeeping_numeric_formats_value() {
        let (t, b, _) = encode_publish_payload(&PublishPayload::BookkeepingNumeric {
            id: BookkeepingId::SocEndOfDayTarget,
            value: 75.0,
        })
        .unwrap();
        assert_eq!(t, "bookkeeping/battery.soc.target.end-of-day/state");
        // Integer-valued float drops `.0` per `f64::Display`.
        assert_eq!(b, "75");

        let (_, b, _) = encode_publish_payload(&PublishPayload::BookkeepingNumeric {
            id: BookkeepingId::EffectiveExportSocThreshold,
            value: 87.5,
        })
        .unwrap();
        assert_eq!(b, "87.5");
    }

    // ------------------------------------------------------------------
    // PR-matter-outdoor-temp: Matter centi-Celsius → SensorReading
    // ------------------------------------------------------------------

    #[test]
    fn matter_outdoor_temp_parses_positive_centi_celsius() {
        // 16.4 °C — the running example from PR scope.
        match parse_matter_outdoor_temp(b"1640", -50.0, 80.0) {
            MatterOutdoorTempParse::Reading(v) => {
                assert!((v - 16.4).abs() < 1e-9, "got {v}");
            }
            other => panic!("expected Reading(16.4), got {other:?}"),
        }
        // The wired-event helper preserves the value.
        let ev = matter_outdoor_temp_event(16.4, Instant::now());
        match ev {
            Event::Sensor(SensorReading {
                id: SensorId::OutdoorTemperature,
                value,
                ..
            }) => assert!((value - 16.4).abs() < 1e-9),
            other => panic!("expected Sensor(OutdoorTemperature), got {other:?}"),
        }
    }

    #[test]
    fn matter_outdoor_temp_parses_negative_centi_celsius() {
        match parse_matter_outdoor_temp(b"-450", -50.0, 80.0) {
            MatterOutdoorTempParse::Reading(v) => {
                assert!((v - -4.5).abs() < 1e-9, "got {v}");
            }
            other => panic!("expected Reading(-4.5), got {other:?}"),
        }
    }

    #[test]
    fn matter_outdoor_temp_drops_null_body() {
        assert_eq!(
            parse_matter_outdoor_temp(b"null", -50.0, 80.0),
            MatterOutdoorTempParse::Drop,
        );
    }

    #[test]
    fn matter_outdoor_temp_drops_string_body() {
        assert_eq!(
            parse_matter_outdoor_temp(b"\"unavailable\"", -50.0, 80.0),
            MatterOutdoorTempParse::Drop,
        );
    }

    #[test]
    fn matter_outdoor_temp_drops_garbage_body() {
        // Not even valid JSON.
        assert_eq!(
            parse_matter_outdoor_temp(b"not-json", -50.0, 80.0),
            MatterOutdoorTempParse::Drop,
        );
    }

    #[test]
    fn matter_outdoor_temp_drops_int16_overflow() {
        // 100 000 centi-°C is well outside Matter's int16 range
        // [-27315, 32767] — drop silently as schema drift.
        assert_eq!(
            parse_matter_outdoor_temp(b"100000", -50.0, 80.0),
            MatterOutdoorTempParse::Drop,
        );
        // Negative overflow too.
        assert_eq!(
            parse_matter_outdoor_temp(b"-30000", -50.0, 80.0),
            MatterOutdoorTempParse::Drop,
        );
    }

    #[test]
    fn matter_outdoor_temp_in_int16_but_out_of_bounds_is_oor() {
        // 7000 centi-°C = 70.0 °C, within int16 but above the
        // configured upper sanity bound (50.0).
        match parse_matter_outdoor_temp(b"7000", -20.0, 50.0) {
            MatterOutdoorTempParse::OutOfRange(v) => {
                assert!((v - 70.0).abs() < 1e-9);
            }
            other => panic!("expected OutOfRange(70.0), got {other:?}"),
        }
    }

    #[test]
    fn matter_outdoor_temp_in_bounds_round_trip() {
        // 500 centi-°C = 5.0 °C — passes both int16 and configured
        // [-20, 50] bounds.
        match parse_matter_outdoor_temp(b"500", -20.0, 50.0) {
            MatterOutdoorTempParse::Reading(v) => assert!((v - 5.0).abs() < 1e-9),
            other => panic!("expected Reading(5.0), got {other:?}"),
        }
    }

    #[test]
    fn matter_outdoor_temp_bounds_are_inclusive() {
        // Exactly at the bounds should pass.
        match parse_matter_outdoor_temp(b"-2000", -20.0, 50.0) {
            MatterOutdoorTempParse::Reading(v) => assert!((v - -20.0).abs() < 1e-9),
            other => panic!("expected Reading(-20.0), got {other:?}"),
        }
        match parse_matter_outdoor_temp(b"5000", -20.0, 50.0) {
            MatterOutdoorTempParse::Reading(v) => assert!((v - 50.0).abs() < 1e-9),
            other => panic!("expected Reading(50.0), got {other:?}"),
        }
    }

    #[test]
    fn parse_knob_value_accepts_hhmmss_discharge_time() {
        // A-49 ride-along: HA's default time-selector sends "HH:MM:SS".
        assert!(matches!(
            parse_knob_value(KnobId::DischargeTime, "02:00:00"),
            Some(KnobValue::DischargeTime(DischargeTime::At0200))
        ));
        assert!(matches!(
            parse_knob_value(KnobId::DischargeTime, "23:00:00"),
            Some(KnobValue::DischargeTime(DischargeTime::At2300))
        ));
        // The existing short forms must still work.
        assert!(matches!(
            parse_knob_value(KnobId::DischargeTime, "02:00"),
            Some(KnobValue::DischargeTime(DischargeTime::At0200))
        ));
        assert!(matches!(
            parse_knob_value(KnobId::DischargeTime, "23:00"),
            Some(KnobValue::DischargeTime(DischargeTime::At2300))
        ));
    }
}
