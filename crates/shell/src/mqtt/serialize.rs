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
use victron_controller_core::types::{
    ActuatedId, BookkeepingKey, BookkeepingValue, Command, Event, KnobId, KnobValue, PublishPayload,
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
    }
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
        | KnobId::WeathersocTooMuchEnergyThreshold => body.parse::<f64>().ok().map(KnobValue::Float),
        KnobId::GridExportLimitW | KnobId::GridImportLimitW | KnobId::EddiDwellS => {
            body.parse::<u32>().ok().map(KnobValue::Uint32)
        }
        KnobId::DischargeTime => match body {
            "02:00" => Some(KnobValue::DischargeTime(DischargeTime::At0200)),
            "23:00" => Some(KnobValue::DischargeTime(DischargeTime::At2300)),
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
}
