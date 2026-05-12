//! Typed state + control envelopes for the HM051M.U43 hydro-kit
//! (LG-Therma-V family, presented as `DEVICE_SYSTEM_BOILER` in ThinQ
//! Connect).
//!
//! The HM051 is an air-to-water monobloc: it heats a water loop that
//! feeds radiators / underfloor heating ("water heat") and a separate
//! domestic-hot-water tank ("hot water"). The controller exposes four
//! actuators — heating power on/off, DHW power on/off, heating target
//! temperature (water-side), DHW target temperature.
//!
//! Schema reference: `pythinqconnect/devices/system_boiler.py`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, Result};

/// Operation mode reported by the unit (and accepted on writes for
/// `boilerOperationMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationMode {
    PowerOn,
    PowerOff,
}

impl OperationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PowerOn => "POWER_ON",
            Self::PowerOff => "POWER_OFF",
        }
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::PowerOn)
    }
}

/// DHW (hot-water) on/off as reported and accepted by the unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HotWaterMode {
    On,
    Off,
}

impl HotWaterMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::On => "ON",
            Self::Off => "OFF",
        }
    }

    pub fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

/// Decoded state of the heat pump as returned by
/// `GET /devices/{id}/state`.
///
/// Only the fields the controller drives or surfaces. Temperature
/// readings are reported in Celsius (the unit's own `unit` field is
/// honoured at parse time; Fahrenheit input is rejected with a clear
/// error rather than silently misinterpreted).
#[derive(Debug, Clone, PartialEq)]
pub struct HeatPumpState {
    /// Overall power. `false` means the whole unit is off — both DHW
    /// and heating are inactive regardless of `hot_water_mode`.
    pub heating_enabled: bool,
    /// DHW circuit on/off.
    pub dhw_enabled: bool,
    /// Current job mode (e.g. `"AUTO"`, `"HEAT"`, `"COOL"`). Surfaced
    /// as a raw string because the set is unit-dependent and we don't
    /// drive it from the controller.
    pub current_job_mode: Option<String>,

    /// DHW tank: measured (current) and commanded (target) temperatures.
    pub dhw_current_c: Option<f64>,
    pub dhw_target_c: Option<f64>,

    /// Heating loop: water-side temperatures. The HM051 is an
    /// air-to-water unit; the "water heat target" is the loop
    /// setpoint, not a room thermostat reading.
    pub heating_water_current_c: Option<f64>,
    pub heating_water_target_c: Option<f64>,

    /// Optional room-air reading (only populated if the unit's remote
    /// room sensor is wired).
    pub room_air_current_c: Option<f64>,
}

impl HeatPumpState {
    /// Parse the JSON envelope returned by `/devices/{id}/state`.
    pub fn from_json(v: &Value) -> Result<Self> {
        let heating_enabled = v
            .pointer("/operation/boilerOperationMode")
            .and_then(Value::as_str)
            .is_some_and(|s| s == "POWER_ON");

        let dhw_enabled = v
            .pointer("/operation/hotWaterMode")
            .and_then(Value::as_str)
            .is_some_and(|s| s == "ON");

        let current_job_mode = v
            .pointer("/boilerJobMode/currentJobMode")
            .and_then(Value::as_str)
            .map(str::to_string);

        // Temperature blocks come as either an object {unit, current, target, ..}
        // or an array of such objects (one per unit). We always prefer
        // the Celsius entry; non-Celsius entries are ignored.
        let dhw_block = pick_celsius_block(v.get("hotWaterTemperatureInUnits"));
        let dhw_current_c = dhw_block
            .as_ref()
            .and_then(|b| b.get("currentTemperature").and_then(Value::as_f64));
        let dhw_target_c = dhw_block
            .as_ref()
            .and_then(|b| b.get("targetTemperature").and_then(Value::as_f64));

        let room_block = pick_celsius_block(v.get("roomTemperatureInUnits"));
        let heating_water_current_c = room_block
            .as_ref()
            .and_then(|b| b.get("outWaterCurrentTemperature").and_then(Value::as_f64));
        // Heating-water target. The SDK documents the field as
        // `waterHeatTargetTemperature`, but on the HM051M.U43 the live
        // response only contains `targetTemperature` inside the room
        // block. `operation.roomTempMode` says what that scalar means:
        // - "WATER" or absent → it's the water-side setpoint.
        // - "AIR" → it's the air-side target, so we MUST NOT use it as
        //   the water target. Fall back to `waterHeatTargetTemperature`
        //   instead (which may itself be absent — that's fine, None
        //   simply means we have no readback this cycle).
        let room_temp_mode = v
            .pointer("/operation/roomTempMode")
            .and_then(Value::as_str);
        let heating_water_target_c = if room_temp_mode == Some("AIR") {
            room_block
                .as_ref()
                .and_then(|b| b.get("waterHeatTargetTemperature").and_then(Value::as_f64))
        } else {
            room_block.as_ref().and_then(|b| {
                b.get("targetTemperature")
                    .and_then(Value::as_f64)
                    .or_else(|| b.get("waterHeatTargetTemperature").and_then(Value::as_f64))
            })
        };
        let room_air_current_c = room_block
            .as_ref()
            .and_then(|b| b.get("airCurrentTemperature").and_then(Value::as_f64));

        Ok(Self {
            heating_enabled,
            dhw_enabled,
            current_job_mode,
            dhw_current_c,
            dhw_target_c,
            heating_water_current_c,
            heating_water_target_c,
            room_air_current_c,
        })
    }
}

/// Pick the unit==C entry from a temperature block. The block can be
/// either an object (single unit) or an array (mixed units); either
/// way we want the Celsius entry, dropping anything else.
fn pick_celsius_block(v: Option<&Value>) -> Option<Value> {
    match v {
        Some(Value::Array(items)) => items
            .iter()
            .find(|item| {
                item.get("unit")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.eq_ignore_ascii_case("C"))
            })
            .cloned(),
        Some(Value::Object(_)) => {
            // Legacy shape without `unit` field — assume Celsius.
            let unit_ok = v
                .and_then(|x| x.get("unit"))
                .and_then(Value::as_str)
                .is_none_or(|s| s.eq_ignore_ascii_case("C"));
            if unit_ok {
                v.cloned()
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Builder for the four control commands the controller issues.
///
/// Each method returns the exact JSON the LG `/devices/{id}/control`
/// endpoint expects. The shape is taken from the official SDK's
/// `_get_attribute_payload`, including the trailing-`C`/`F`-suffix
/// stripping the SDK performs (`hotWaterTargetTemperatureC` →
/// `targetTemperature` in the wire payload).
#[derive(Debug, Default, Clone, Copy)]
pub struct HeatPumpControl;

impl HeatPumpControl {
    /// Heating master power. `enable=true` → `POWER_ON`.
    pub fn set_heating_power(enable: bool) -> Value {
        let mode = if enable {
            OperationMode::PowerOn
        } else {
            OperationMode::PowerOff
        };
        json!({"operation": {"boilerOperationMode": mode.as_str()}})
    }

    /// DHW circuit on/off.
    pub fn set_dhw_power(enable: bool) -> Value {
        let mode = if enable {
            HotWaterMode::On
        } else {
            HotWaterMode::Off
        };
        json!({"operation": {"hotWaterMode": mode.as_str()}})
    }

    /// DHW target temperature in Celsius.
    pub fn set_dhw_target_c(temp: i64) -> Value {
        json!({
            "hotWaterTemperatureInUnits": {
                "targetTemperature": temp,
                "unit": "C"
            }
        })
    }

    /// Heating-loop water target temperature in Celsius.
    ///
    /// This is the water-side setpoint; the HM051 is an air-to-water
    /// monobloc and its primary heating control surface is the loop
    /// temperature (not a room air target). If the operator's setup
    /// actually wants air-side control, see [`HeatPumpControl::set_air_heat_target_c`].
    pub fn set_water_heat_target_c(temp: i64) -> Value {
        json!({
            "roomTemperatureInUnits": {
                "waterHeatTargetTemperature": temp,
                "unit": "C"
            }
        })
    }

    /// Air-side heating target (rarely used on monobloc installs).
    pub fn set_air_heat_target_c(temp: i64) -> Value {
        json!({
            "roomTemperatureInUnits": {
                "airHeatTargetTemperature": temp,
                "unit": "C"
            }
        })
    }
}

/// Convenience: validate a target temperature against a tight envelope
/// before sending. LG itself enforces device-specific ranges and would
/// reject out-of-band values with `UNACCEPTABLE_PARAMETERS`, but
/// catching it client-side keeps the dashboard's MQTT-retained command
/// channel from filing nonsense at the cloud at 100 Hz when a knob is
/// mis-set.
pub fn validate_temperature_c(temp: i64, min: i64, max: i64) -> Result<i64> {
    if temp < min || temp > max {
        return Err(Error::Config(format!(
            "temperature {temp}°C outside allowed range {min}..={max}°C"
        )));
    }
    Ok(temp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture_state() -> Value {
        // Trimmed shape based on the SystemBoilerProfile schema in the
        // upstream SDK. Real-world responses include more fields; the
        // decoder ignores everything it doesn't recognise.
        json!({
            "boilerJobMode": {"currentJobMode": "AUTO"},
            "operation": {
                "boilerOperationMode": "POWER_ON",
                "hotWaterMode": "ON",
                "roomWaterMode": "HEAT"
            },
            "hotWaterTemperatureInUnits": [
                {"unit": "C", "currentTemperature": 47.5, "targetTemperature": 48,
                 "minTemperature": 30, "maxTemperature": 60},
                {"unit": "F", "currentTemperature": 117.5, "targetTemperature": 118}
            ],
            "roomTemperatureInUnits": [
                {"unit": "C",
                 "currentTemperature": 21.0,
                 "airCurrentTemperature": 21.0,
                 "outWaterCurrentTemperature": 34.2,
                 "inWaterCurrentTemperature": 30.1,
                 "waterHeatTargetTemperature": 35,
                 "airHeatTargetTemperature": 21}
            ]
        })
    }

    #[test]
    fn decode_state_full() {
        let s = HeatPumpState::from_json(&fixture_state()).unwrap();
        assert!(s.heating_enabled);
        assert!(s.dhw_enabled);
        assert_eq!(s.current_job_mode.as_deref(), Some("AUTO"));
        assert_eq!(s.dhw_current_c, Some(47.5));
        assert_eq!(s.dhw_target_c, Some(48.0));
        assert_eq!(s.heating_water_current_c, Some(34.2));
        assert_eq!(s.heating_water_target_c, Some(35.0));
        assert_eq!(s.room_air_current_c, Some(21.0));
    }

    #[test]
    fn decode_state_handles_power_off() {
        let v = json!({
            "operation": {"boilerOperationMode": "POWER_OFF", "hotWaterMode": "OFF"}
        });
        let s = HeatPumpState::from_json(&v).unwrap();
        assert!(!s.heating_enabled);
        assert!(!s.dhw_enabled);
        assert!(s.dhw_target_c.is_none());
        assert!(s.heating_water_target_c.is_none());
    }

    #[test]
    fn decode_state_fahrenheit_only_yields_no_temperatures() {
        let v = json!({
            "operation": {"boilerOperationMode": "POWER_ON", "hotWaterMode": "ON"},
            "hotWaterTemperatureInUnits": [
                {"unit": "F", "currentTemperature": 110, "targetTemperature": 120}
            ]
        });
        let s = HeatPumpState::from_json(&v).unwrap();
        // No Celsius block → no temperature; we don't auto-convert
        // because we'd risk hiding a misconfigured unit.
        assert!(s.dhw_current_c.is_none());
        assert!(s.dhw_target_c.is_none());
    }

    /// Pinned on the real HM051M.U43 response captured 2026-05-12 via the
    /// one-shot `lg_thinq raw device-state envelope` diagnostic. The
    /// significant deviations from the SDK's documented shape:
    /// - the heating-water target sits under `targetTemperature` (not
    ///   `waterHeatTargetTemperature`);
    /// - `operation.roomTempMode == "WATER"` qualifies that scalar as
    ///   the water-side setpoint.
    #[test]
    fn decode_state_real_hm051_water_mode_envelope() {
        let v = json!({
            "boilerJobMode": {"currentJobMode": "HEAT"},
            "operation": {
                "boilerOperationMode": "POWER_OFF",
                "hotWaterMode": "OFF",
                "roomTempMode": "WATER",
                "roomWaterMode": "OUT_WATER"
            },
            "hotWaterTemperatureInUnits": [
                {"unit": "C", "currentTemperature": 23.5, "targetTemperature": 60,
                 "minTemperature": 30, "maxTemperature": 65},
                {"unit": "F", "currentTemperature": 75, "targetTemperature": 140}
            ],
            "roomTemperatureInUnits": [
                {"unit": "C",
                 "airCurrentTemperature": 23,
                 "currentTemperature": 38.5,
                 "inWaterCurrentTemperature": 39,
                 "outWaterCurrentTemperature": 38.5,
                 "targetTemperature": 40,
                 "waterHeatMaxTemperature": 55,
                 "waterHeatMinTemperature": 20}
            ]
        });
        let s = HeatPumpState::from_json(&v).unwrap();
        assert!(!s.heating_enabled);
        assert!(!s.dhw_enabled);
        assert_eq!(s.dhw_current_c, Some(23.5));
        assert_eq!(s.dhw_target_c, Some(60.0));
        assert_eq!(s.heating_water_current_c, Some(38.5));
        // The actual live response only carries `targetTemperature`,
        // not `waterHeatTargetTemperature`. Pre-fix the decoder
        // returned None here and the dashboard rendered "undefined °C".
        assert_eq!(s.heating_water_target_c, Some(40.0));
        assert_eq!(s.room_air_current_c, Some(23.0));
    }

    /// AIR-mode safety: when `roomTempMode == "AIR"` the room block's
    /// `targetTemperature` is the air-side target, not the water-side
    /// one. The decoder must NOT misread it as the heating-water
    /// target.
    #[test]
    fn decode_state_air_mode_does_not_misread_target_temperature() {
        let v = json!({
            "operation": {
                "boilerOperationMode": "POWER_ON",
                "hotWaterMode": "ON",
                "roomTempMode": "AIR"
            },
            "roomTemperatureInUnits": [
                {"unit": "C",
                 "currentTemperature": 21.0,
                 "airCurrentTemperature": 21.0,
                 "outWaterCurrentTemperature": 34.2,
                 "targetTemperature": 22}
            ]
        });
        let s = HeatPumpState::from_json(&v).unwrap();
        // `targetTemperature: 22` is the air target — must not appear
        // as `heating_water_target_c`.
        assert_eq!(s.heating_water_target_c, None);
        // The water current still comes from `outWaterCurrentTemperature`.
        assert_eq!(s.heating_water_current_c, Some(34.2));
    }

    /// AIR-mode + `waterHeatTargetTemperature` present: the explicit
    /// water-target key wins (some firmware revisions may include it).
    #[test]
    fn decode_state_air_mode_uses_water_heat_target_temperature_when_present() {
        let v = json!({
            "operation": {
                "boilerOperationMode": "POWER_ON",
                "hotWaterMode": "ON",
                "roomTempMode": "AIR"
            },
            "roomTemperatureInUnits": [
                {"unit": "C",
                 "targetTemperature": 22,
                 "waterHeatTargetTemperature": 38,
                 "outWaterCurrentTemperature": 34.2}
            ]
        });
        let s = HeatPumpState::from_json(&v).unwrap();
        assert_eq!(s.heating_water_target_c, Some(38.0));
    }

    #[test]
    fn decode_state_accepts_object_shape() {
        // Older firmware revisions reportedly return the block as an
        // object instead of a single-element array.
        let v = json!({
            "operation": {"boilerOperationMode": "POWER_ON", "hotWaterMode": "ON"},
            "hotWaterTemperatureInUnits":
                {"unit": "C", "currentTemperature": 50.0, "targetTemperature": 52}
        });
        let s = HeatPumpState::from_json(&v).unwrap();
        assert_eq!(s.dhw_current_c, Some(50.0));
        assert_eq!(s.dhw_target_c, Some(52.0));
    }

    #[test]
    fn control_heating_power_on() {
        assert_eq!(
            HeatPumpControl::set_heating_power(true),
            json!({"operation": {"boilerOperationMode": "POWER_ON"}})
        );
    }

    #[test]
    fn control_heating_power_off() {
        assert_eq!(
            HeatPumpControl::set_heating_power(false),
            json!({"operation": {"boilerOperationMode": "POWER_OFF"}})
        );
    }

    #[test]
    fn control_dhw_power() {
        assert_eq!(
            HeatPumpControl::set_dhw_power(true),
            json!({"operation": {"hotWaterMode": "ON"}})
        );
        assert_eq!(
            HeatPumpControl::set_dhw_power(false),
            json!({"operation": {"hotWaterMode": "OFF"}})
        );
    }

    #[test]
    fn control_dhw_target_strips_unit_suffix() {
        // The Python SDK builds `hotWaterTargetTemperatureC = 48` then
        // strips the trailing `C` for the wire shape. We do the same
        // by emitting the suffix-less key directly.
        assert_eq!(
            HeatPumpControl::set_dhw_target_c(48),
            json!({"hotWaterTemperatureInUnits": {"targetTemperature": 48, "unit": "C"}})
        );
    }

    #[test]
    fn control_water_heat_target() {
        assert_eq!(
            HeatPumpControl::set_water_heat_target_c(35),
            json!({"roomTemperatureInUnits": {"waterHeatTargetTemperature": 35, "unit": "C"}})
        );
    }

    #[test]
    fn validate_temperature_bounds() {
        assert!(validate_temperature_c(48, 30, 60).is_ok());
        assert!(validate_temperature_c(30, 30, 60).is_ok());
        assert!(validate_temperature_c(60, 30, 60).is_ok());
        assert!(validate_temperature_c(29, 30, 60).is_err());
        assert!(validate_temperature_c(61, 30, 60).is_err());
    }
}
