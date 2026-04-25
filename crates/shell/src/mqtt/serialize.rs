//! MQTT wire format: `PublishPayload` ↔ `(topic, body, retain)`
//! ↔ `Event::Command`.
//!
//! All state messages are retained JSON so a reboot sees the last
//! values. Logs are not retained.

use std::time::Instant;

use serde_json::json;
use tracing::warn;

use victron_controller_core::knobs::{
    ChargeBatteryExtendedMode, DebugFullCharge, DischargeTime, ForecastDisagreementStrategy,
};
#[cfg(test)]
use victron_controller_core::tass::Freshness;
use victron_controller_core::types::{
    ActuatedId, BookkeepingKey, BookkeepingValue, Command, Event, KnobId, KnobValue,
    PublishPayload, SensorId, encode_sensor_body,
};
use victron_controller_core::Owner;

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
        "next_full_charge" => BookkeepingKey::NextFullCharge,
        "above_soc_date" => BookkeepingKey::AboveSocDate,
        "prev_ess_state" => BookkeepingKey::PrevEssState,
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
        KnobId::ForceDisableExport => "force_disable_export",
        KnobId::ExportSocThreshold => "export_soc_threshold",
        KnobId::DischargeSocTarget => "discharge_soc_target",
        KnobId::BatterySocTarget => "battery_soc_target",
        KnobId::FullChargeDischargeSocTarget => "full_charge_discharge_soc_target",
        KnobId::FullChargeExportSocThreshold => "full_charge_export_soc_threshold",
        KnobId::DischargeTime => "discharge_time",
        KnobId::DebugFullCharge => "debug_full_charge",
        KnobId::PessimismMultiplierModifier => "pessimism_multiplier_modifier",
        KnobId::DisableNightGridDischarge => "disable_night_grid_discharge",
        KnobId::ChargeCarBoost => "charge_car_boost",
        KnobId::ChargeCarExtended => "charge_car_extended",
        KnobId::ZappiCurrentTarget => "zappi_current_target",
        KnobId::ZappiLimit => "zappi_limit",
        KnobId::ZappiEmergencyMargin => "zappi_emergency_margin",
        KnobId::GridExportLimitW => "grid_export_limit_w",
        KnobId::GridImportLimitW => "grid_import_limit_w",
        KnobId::AllowBatteryToCar => "allow_battery_to_car",
        KnobId::EddiEnableSoc => "eddi_enable_soc",
        KnobId::EddiDisableSoc => "eddi_disable_soc",
        KnobId::EddiDwellS => "eddi_dwell_s",
        KnobId::WeathersocWinterTemperatureThreshold => "weathersoc_winter_temperature_threshold",
        KnobId::WeathersocLowEnergyThreshold => "weathersoc_low_energy_threshold",
        KnobId::WeathersocOkEnergyThreshold => "weathersoc_ok_energy_threshold",
        KnobId::WeathersocHighEnergyThreshold => "weathersoc_high_energy_threshold",
        KnobId::WeathersocTooMuchEnergyThreshold => "weathersoc_too_much_energy_threshold",
        KnobId::ForecastDisagreementStrategy => "forecast_disagreement_strategy",
        KnobId::ChargeBatteryExtendedMode => "charge_battery_extended_mode",
    }
}

fn knob_id_from_name(n: &str) -> Option<KnobId> {
    Some(match n {
        "force_disable_export" => KnobId::ForceDisableExport,
        "export_soc_threshold" => KnobId::ExportSocThreshold,
        "discharge_soc_target" => KnobId::DischargeSocTarget,
        "battery_soc_target" => KnobId::BatterySocTarget,
        "full_charge_discharge_soc_target" => KnobId::FullChargeDischargeSocTarget,
        "full_charge_export_soc_threshold" => KnobId::FullChargeExportSocThreshold,
        "discharge_time" => KnobId::DischargeTime,
        "debug_full_charge" => KnobId::DebugFullCharge,
        "pessimism_multiplier_modifier" => KnobId::PessimismMultiplierModifier,
        "disable_night_grid_discharge" => KnobId::DisableNightGridDischarge,
        "charge_car_boost" => KnobId::ChargeCarBoost,
        "charge_car_extended" => KnobId::ChargeCarExtended,
        "zappi_current_target" => KnobId::ZappiCurrentTarget,
        "zappi_limit" => KnobId::ZappiLimit,
        "zappi_emergency_margin" => KnobId::ZappiEmergencyMargin,
        "grid_export_limit_w" => KnobId::GridExportLimitW,
        "grid_import_limit_w" => KnobId::GridImportLimitW,
        "allow_battery_to_car" => KnobId::AllowBatteryToCar,
        "eddi_enable_soc" => KnobId::EddiEnableSoc,
        "eddi_disable_soc" => KnobId::EddiDisableSoc,
        "eddi_dwell_s" => KnobId::EddiDwellS,
        "weathersoc_winter_temperature_threshold" => KnobId::WeathersocWinterTemperatureThreshold,
        "weathersoc_low_energy_threshold" => KnobId::WeathersocLowEnergyThreshold,
        "weathersoc_ok_energy_threshold" => KnobId::WeathersocOkEnergyThreshold,
        "weathersoc_high_energy_threshold" => KnobId::WeathersocHighEnergyThreshold,
        "weathersoc_too_much_energy_threshold" => KnobId::WeathersocTooMuchEnergyThreshold,
        "forecast_disagreement_strategy" => KnobId::ForecastDisagreementStrategy,
        "charge_battery_extended_mode" => KnobId::ChargeBatteryExtendedMode,
        _ => return None,
    })
}

fn actuated_name(id: ActuatedId) -> &'static str {
    match id {
        ActuatedId::GridSetpoint => "grid_setpoint",
        ActuatedId::InputCurrentLimit => "input_current_limit",
        ActuatedId::ZappiMode => "zappi_mode",
        ActuatedId::EddiMode => "eddi_mode",
        ActuatedId::Schedule0 => "schedule_0",
        ActuatedId::Schedule1 => "schedule_1",
    }
}

fn bookkeeping_name(k: BookkeepingKey) -> &'static str {
    match k {
        BookkeepingKey::NextFullCharge => "next_full_charge",
        BookkeepingKey::AboveSocDate => "above_soc_date",
        BookkeepingKey::PrevEssState => "prev_ess_state",
    }
}

/// PR-ha-discovery-expand: stable `snake_case` topic-tail name for each
/// sensor. Mirrors the `Sensors`-struct field names in
/// `crates/core/src/world.rs`. See the topic taxonomy comment at the
/// top of `crates/shell/src/mqtt/discovery.rs`.
pub(crate) fn sensor_name(id: SensorId) -> &'static str {
    match id {
        SensorId::BatterySoc => "battery_soc",
        SensorId::BatterySoh => "battery_soh",
        SensorId::BatteryInstalledCapacity => "battery_installed_capacity",
        SensorId::BatteryDcPower => "battery_dc_power",
        SensorId::MpptPower0 => "mppt_power_0",
        SensorId::MpptPower1 => "mppt_power_1",
        SensorId::SoltaroPower => "soltaro_power",
        SensorId::PowerConsumption => "power_consumption",
        SensorId::GridPower => "grid_power",
        SensorId::GridVoltage => "grid_voltage",
        SensorId::GridCurrent => "grid_current",
        SensorId::ConsumptionCurrent => "consumption_current",
        SensorId::OffgridPower => "offgrid_power",
        SensorId::OffgridCurrent => "offgrid_current",
        SensorId::VebusInputCurrent => "vebus_input_current",
        SensorId::EvchargerAcPower => "evcharger_ac_power",
        SensorId::EvchargerAcCurrent => "evcharger_ac_current",
        SensorId::EssState => "ess_state",
        SensorId::OutdoorTemperature => "outdoor_temperature",
        SensorId::SessionKwh => "session_kwh",
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
        KnobValue::DebugFullCharge(DebugFullCharge::None) => "none".to_string(),
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

        // Power (W)
        KnobId::GridExportLimitW | KnobId::GridImportLimitW => (0.0, 10_000.0),

        // Time (s)
        KnobId::EddiDwellS => (0.0, 3600.0),

        // Multiplier
        KnobId::PessimismMultiplierModifier => (0.0, 2.0),

        // Enums + bools don't use this table.
        KnobId::ForceDisableExport
        | KnobId::DisableNightGridDischarge
        | KnobId::ChargeCarBoost
        | KnobId::ChargeCarExtended
        | KnobId::AllowBatteryToCar
        | KnobId::DischargeTime
        | KnobId::DebugFullCharge
        | KnobId::ForecastDisagreementStrategy
        | KnobId::ChargeBatteryExtendedMode => return None,
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
        | KnobId::ChargeCarExtended
        | KnobId::AllowBatteryToCar => parse_bool(body).map(KnobValue::Bool),
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
        | KnobId::WeathersocTooMuchEnergyThreshold => {
            parse_ranged_float(id, body).map(KnobValue::Float)
        }
        KnobId::GridExportLimitW | KnobId::GridImportLimitW | KnobId::EddiDwellS => {
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
            "none" => Some(KnobValue::DebugFullCharge(DebugFullCharge::None)),
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
        assert_eq!(t, "knob/force_disable_export/state");
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
        assert_eq!(t, "knob/export_soc_threshold/state");
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
        assert_eq!(t, "entity/grid_setpoint/phase");
        assert!(b.contains("Confirmed"));
    }

    // ------------------------------------------------------------------
    // Wire → Event::Command
    // ------------------------------------------------------------------

    #[test]
    fn decode_bool_knob_set() {
        let e = decode_knob_set(
            "victron-controller",
            "victron-controller/knob/force_disable_export/set",
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
            "victron-controller/knob/export_soc_threshold/set",
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
            "victron-controller/knob/force_disable_export/set",
            b"maybe"
        )
        .is_none());
    }

    #[test]
    fn decode_wrong_topic_root_returns_none() {
        // Root prefix mismatch.
        assert!(decode_knob_set(
            "victron-controller",
            "other-root/knob/force_disable_export/set",
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
            "victron-controller/knob/export_soc_threshold/state",
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
            "victron-controller/knob/force_disable_export/set",
            b"true"
        )
        .is_none());
    }

    #[test]
    fn decode_knob_set_rejects_state_suffix() {
        // Symmetrically: /set decoder must not match /state topics.
        assert!(decode_knob_set(
            "victron-controller",
            "victron-controller/knob/force_disable_export/state",
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
            "victron-controller/bookkeeping/next_full_charge/state",
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
            "victron-controller/bookkeeping/above_soc_date/state",
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
            "victron-controller/bookkeeping/prev_ess_state/state",
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
            "victron-controller/bookkeeping/next_full_charge/state",
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
            "victron-controller/bookkeeping/above_soc_date/state",
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
            // Power (u32)
            (KnobId::GridExportLimitW, "10001"),
            (KnobId::GridImportLimitW, "10001"),
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
            parse_knob_value(KnobId::GridExportLimitW, "4900"),
            Some(KnobValue::Uint32(4900))
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
            // Power caps
            (KnobId::GridExportLimitW, "0"),
            (KnobId::GridExportLimitW, "10000"),
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
        assert_eq!(t, "sensor/battery_soc/state");
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
        assert_eq!(t, "sensor/outdoor_temperature/state");
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
        assert_eq!(t, "bookkeeping/zappi_active/state");
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
        assert_eq!(t, "bookkeeping/soc_end_of_day_target/state");
        // Integer-valued float drops `.0` per `f64::Display`.
        assert_eq!(b, "75");

        let (_, b, _) = encode_publish_payload(&PublishPayload::BookkeepingNumeric {
            id: BookkeepingId::EffectiveExportSocThreshold,
            value: 87.5,
        })
        .unwrap();
        assert_eq!(b, "87.5");
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
