//! Parsers for the myenergi `cgi-jstatus-*` JSON responses.
//!
//! The shape is a top-level object with a single array key:
//!
//! ```json
//! { "zappi": [{ "sno": 12345678, "zmo": 3, "sta": 3, "pst": "C2", ... }] }
//! { "eddi":  [{ "sno": 12345678, "sta": 1, "div": 300, "che": 2.5, ... }] }
//! ```
//!
//! Fields we consume:
//!
//! - **Zappi**: `zmo` (mode 1/2/3/4), `sta` (status 1/3/5), `pst`
//!   (plug state A/B1/B2/C1/C2/F), session `che` (kWh).
//! - **Eddi**: `sta` (status 1=Normal/0=Stopped — actually the mapping
//!   differs between firmwares; best-effort).

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use victron_controller_core::myenergi::{
    EddiMode, ZappiMode, ZappiPlugState, ZappiState, ZappiStatus,
};

/// One Zappi state observation parsed from `cgi-jstatus-Z*`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiObservation {
    pub state: ZappiState,
    /// Session energy delivered so far (kWh). We keep this even though
    /// it's not on `ZappiState` so the Zappi-mode controller can gate
    /// night auto-stop on it via a future bookkeeping field.
    pub session_che_kwh: f64,
}

/// Top-level myenergi status response: either `{"zappi":[...]}` or
/// `{"eddi":[...]}`. We pull the first entry of whichever array is
/// present.
pub fn parse_zappi(body: &serde_json::Value) -> Option<ZappiObservation> {
    let entry = body.get("zappi").and_then(|v| v.as_array())?.first()?;

    let zmo = entry.get("zmo")?.as_u64()? as u8;
    let sta = entry.get("sta")?.as_u64()? as u8;
    let pst = entry.get("pst")?.as_str()?;
    let dat = entry.get("dat").and_then(|v| v.as_str()).unwrap_or("01-01-2026");
    let tim = entry.get("tim").and_then(|v| v.as_str()).unwrap_or("00:00:00");
    let che = entry.get("che").and_then(|v| v.as_f64()).unwrap_or(0.0);

    Some(ZappiObservation {
        state: ZappiState {
            zappi_mode: zappi_mode_from_code(zmo),
            zappi_plug_state: zappi_plug_state_from_str(pst),
            zappi_status: zappi_status_from_code(sta),
            zappi_last_change_signature: parse_myenergi_ts(dat, tim),
        },
        session_che_kwh: che,
    })
}

/// Extract the Eddi mode from the `sta` field. This is firmware-
/// dependent; the mapping below matches the modern devices:
///
/// - `sta = 1` ⇒ Normal (diverting)
/// - `sta = 0` or `3` ⇒ Stopped
/// - anything else ⇒ Stopped (safe direction)
pub fn parse_eddi(body: &serde_json::Value) -> Option<EddiMode> {
    let entry = body.get("eddi").and_then(|v| v.as_array())?.first()?;
    let sta = entry.get("sta")?.as_u64()? as u8;
    Some(match sta {
        1 => EddiMode::Normal,
        _ => EddiMode::Stopped,
    })
}

// --- Coercions ---

fn zappi_mode_from_code(c: u8) -> ZappiMode {
    match c {
        1 => ZappiMode::Fast,
        2 => ZappiMode::Eco,
        3 => ZappiMode::EcoPlus,
        _ => ZappiMode::Off,
    }
}

fn zappi_status_from_code(c: u8) -> ZappiStatus {
    match c {
        1 => ZappiStatus::Paused,
        3 => ZappiStatus::DivertingOrCharging,
        5 => ZappiStatus::Complete,
        other => ZappiStatus::Other(other),
    }
}

fn zappi_plug_state_from_str(s: &str) -> ZappiPlugState {
    match s {
        "A" => ZappiPlugState::EvDisconnected,
        "B1" => ZappiPlugState::EvConnected,
        "B2" => ZappiPlugState::WaitingForEv,
        "C1" => ZappiPlugState::EvReadyToCharge,
        "C2" => ZappiPlugState::Charging,
        // "F" and any unknown string: fail conservatively to Fault.
        _ => ZappiPlugState::Fault,
    }
}

/// Parse `dat = "21-04-2026"` (DD-MM-YYYY) + `tim = "22:38:13"` into
/// a `NaiveDateTime`. Falls back to epoch on parse failure so the
/// caller doesn't have to handle a None.
fn parse_myenergi_ts(dat: &str, tim: &str) -> NaiveDateTime {
    let date = NaiveDate::parse_from_str(dat, "%d-%m-%Y")
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2_026, 1, 1).unwrap());
    let time = NaiveTime::parse_from_str(tim, "%H:%M:%S")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    NaiveDateTime::new(date, time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_zappi_fast_charging() {
        let body = json!({
            "zappi": [{
                "sno": 12_345_678,
                "zmo": 1,     // Fast
                "sta": 3,     // DivertingOrCharging
                "pst": "C2",  // Charging
                "dat": "21-04-2026",
                "tim": "22:38:13",
                "che": 5.3
            }]
        });
        let obs = parse_zappi(&body).unwrap();
        assert_eq!(obs.state.zappi_mode, ZappiMode::Fast);
        assert_eq!(obs.state.zappi_status, ZappiStatus::DivertingOrCharging);
        assert_eq!(obs.state.zappi_plug_state, ZappiPlugState::Charging);
        assert!((obs.session_che_kwh - 5.3).abs() < f64::EPSILON);
    }

    #[test]
    fn parses_zappi_off_disconnected() {
        let body = json!({
            "zappi": [{"zmo": 4, "sta": 1, "pst": "A"}]
        });
        let obs = parse_zappi(&body).unwrap();
        assert_eq!(obs.state.zappi_mode, ZappiMode::Off);
        assert_eq!(obs.state.zappi_plug_state, ZappiPlugState::EvDisconnected);
    }

    #[test]
    fn parses_zappi_unknown_status_code_propagates() {
        let body = json!({
            "zappi": [{"zmo": 2, "sta": 7, "pst": "B1"}]
        });
        let obs = parse_zappi(&body).unwrap();
        assert_eq!(obs.state.zappi_status, ZappiStatus::Other(7));
    }

    #[test]
    fn parse_zappi_returns_none_on_missing_array() {
        assert!(parse_zappi(&json!({})).is_none());
        assert!(parse_zappi(&json!({"zappi": []})).is_none());
    }

    #[test]
    fn parse_eddi_sta_1_is_normal() {
        let b = json!({"eddi":[{"sta": 1}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Normal));
    }

    #[test]
    fn parse_eddi_sta_0_is_stopped() {
        let b = json!({"eddi":[{"sta": 0}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Stopped));
    }

    #[test]
    fn parse_eddi_unknown_sta_is_stopped_safe_default() {
        let b = json!({"eddi":[{"sta": 99}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Stopped));
    }

    #[test]
    fn parse_myenergi_ts_roundtrip() {
        let nt = parse_myenergi_ts("21-04-2026", "22:38:13");
        assert_eq!(nt.date(), NaiveDate::from_ymd_opt(2026, 4, 21).unwrap());
    }

    #[test]
    fn parse_myenergi_ts_bad_input_falls_back() {
        let nt = parse_myenergi_ts("garbage", "also garbage");
        // Doesn't panic; returns the sentinel we defined.
        assert_eq!(nt.date(), NaiveDate::from_ymd_opt(2_026, 1, 1).unwrap());
    }
}
