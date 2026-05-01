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
//! - **Eddi**: `sta` — operational status; see `parse_eddi` for the
//!   sta→EddiMode inference.
//!
//! Note on timestamps: myenergi reports `dat`/`tim` in UTC, but the
//! core's wait-timeout math runs against `Clock::monotonic()`. Mixing
//! UTC wall-clock with local naive time produced defects A-04 (1 h
//! offset in BST) and A-24 (sentinel date collapsing change-detection).
//! PR-03 removes the wall-clock timestamp from the path: the poller
//! stamps `Instant::now()` on every observed `(zmo, sta, pst)` change
//! and passes that `Instant` through as `zappi_last_change_signature`.

use std::time::Instant;

use victron_controller_core::myenergi::{
    EddiMode, ZappiMode, ZappiPlugState, ZappiState, ZappiStatus,
};

/// One Zappi state observation parsed from `cgi-jstatus-Z*`.
///
/// `state.zappi_last_change_signature` is stamped by the poller via
/// [`ZappiChangeTracker`] — this parser does not touch wall-clock
/// time. `state.session_kwh` comes from the myenergi `che` field and
/// is consumed by the Zappi-mode controller's night auto-stop rule
/// (A-13 / A-14).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiObservation {
    pub state: ZappiState,
}

/// Per-poller state used to decide whether the observed
/// `(zmo, sta, pst)` tuple has changed since the previous poll.
/// Holds the latched monotonic `Instant` at which the last change
/// was observed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiChangeTracker {
    last: (ZappiMode, ZappiStatus, ZappiPlugState),
    stamp: Instant,
}

impl ZappiChangeTracker {
    /// Initial tracker — stamped with the provided `Instant` (typically
    /// `Instant::now()` at poller start). The classifier's
    /// `WAIT_TIMEOUT_MIN` branch will then wait ~5 min before timing
    /// out, which is correct: we can't assume anything about zappi
    /// state age at startup.
    #[must_use]
    pub fn new(
        initial: (ZappiMode, ZappiStatus, ZappiPlugState),
        stamp: Instant,
    ) -> Self {
        Self { last: initial, stamp }
    }

    /// Observe a new tuple. Returns the latched `Instant` — the stamp
    /// of the most recent change. If `tuple` differs from the previous
    /// observation, `now` becomes the new stamp.
    pub fn observe(
        &mut self,
        tuple: (ZappiMode, ZappiStatus, ZappiPlugState),
        now: Instant,
    ) -> Instant {
        if tuple != self.last {
            self.last = tuple;
            self.stamp = now;
        }
        self.stamp
    }
}

/// Top-level myenergi status response: either `{"zappi":[...]}` or
/// `{"eddi":[...]}`. We pull the first entry of whichever array is
/// present.
///
/// `stamp` is the monotonic `Instant` the poller has determined for
/// the mode/plug/status signature — see [`ZappiChangeTracker`].
pub fn parse_zappi(
    body: &serde_json::Value,
    stamp: Instant,
) -> Option<ZappiObservation> {
    let entry = body.get("zappi").and_then(|v| v.as_array())?.first()?;

    // A-25: reject out-of-range zmo/sta instead of silently wrapping.
    // `as_u64() as u8` wraps on ≥256 → sta=257 would decode as sta=1
    // (Paused) and we'd trust the wrong state. `try_from` + ? returns
    // None for the whole poll, which the poller treats as a missed
    // poll rather than bogus data.
    let zmo = u8::try_from(entry.get("zmo")?.as_u64()?).ok()?;
    let sta = u8::try_from(entry.get("sta")?.as_u64()?).ok()?;
    let pst = entry.get("pst")?.as_str()?;
    // A-51: `che` is session-kWh; accept finite non-negative numbers,
    // reject NaN / Inf / negative firmware bugs.
    let che = entry
        .get("che")
        .and_then(|v| v.as_f64())
        .filter(|n| n.is_finite() && *n >= 0.0)
        .unwrap_or(0.0);

    Some(ZappiObservation {
        state: ZappiState {
            zappi_mode: zappi_mode_from_code(zmo),
            zappi_plug_state: zappi_plug_state_from_str(pst),
            zappi_status: zappi_status_from_code(sta),
            zappi_last_change_signature: stamp,
            session_kwh: che,
        },
    })
}

/// Extract only the `(zmo, sta, pst)` change-detection tuple from a
/// `cgi-jstatus-Z*` body. The poller calls this first to decide
/// whether the signature changed, then calls [`parse_zappi`] with the
/// resulting latched `Instant`.
#[must_use]
pub fn parse_zappi_signature(
    body: &serde_json::Value,
) -> Option<(ZappiMode, ZappiStatus, ZappiPlugState)> {
    let entry = body.get("zappi").and_then(|v| v.as_array())?.first()?;
    let zmo = u8::try_from(entry.get("zmo")?.as_u64()?).ok()?;
    let sta = u8::try_from(entry.get("sta")?.as_u64()?).ok()?;
    let pst = entry.get("pst")?.as_str()?;
    Some((
        zappi_mode_from_code(zmo),
        zappi_status_from_code(sta),
        zappi_plug_state_from_str(pst),
    ))
}

/// Extract the Eddi mode from the `sta` field.
///
/// Important: `sta` is the *operational status*, not the *mode*. The
/// device exposes mode (Normal/Stopped) only indirectly — we infer it
/// from the operational status code. Documented codes:
///
/// - `0` — Stopped
/// - `1` — Paused (mode is Normal, no surplus right now)
/// - `3` — Diverting (mode is Normal, dumping power)
/// - `4` — Boost (mode is Normal, manual / scheduled boost)
/// - `5` — Hot / max-temp reached (mode is Normal)
/// - `6` — Stopped
///
/// Mapping: `sta ∈ {0, 6}` ⇒ Stopped; everything else (including
/// undocumented codes) ⇒ Normal. This is heuristic-from-docs and will
/// be verified against real device captures once the entity inspector
/// popup surfaces raw bodies (PR-EDDI-SENSORS-1, Part D).
///
/// Defaulting unknown codes to Normal looks unsafe at the parser layer
/// but is acceptable: the EddiController gates on freshness, and
/// `Stale`/`Unknown` freshness drives the actuator to the safe-Stopped
/// direction one layer up. The parser only governs what we report when
/// we *do* have a fresh reading.
pub fn parse_eddi(body: &serde_json::Value) -> Option<EddiMode> {
    let entry = body.get("eddi").and_then(|v| v.as_array())?.first()?;
    let sta = u8::try_from(entry.get("sta")?.as_u64()?).ok()?;
    Some(match sta {
        0 | 6 => EddiMode::Stopped,
        _ => EddiMode::Normal,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn fixed_stamp() -> Instant {
        // Any `Instant` works; parser is stamp-agnostic.
        Instant::now()
    }

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
        let stamp = fixed_stamp();
        let obs = parse_zappi(&body, stamp).unwrap();
        assert_eq!(obs.state.zappi_mode, ZappiMode::Fast);
        assert_eq!(obs.state.zappi_status, ZappiStatus::DivertingOrCharging);
        assert_eq!(obs.state.zappi_plug_state, ZappiPlugState::Charging);
        assert_eq!(obs.state.zappi_last_change_signature, stamp);
        assert!((obs.state.session_kwh - 5.3).abs() < f64::EPSILON);
    }

    #[test]
    fn parses_zappi_off_disconnected() {
        let body = json!({
            "zappi": [{"zmo": 4, "sta": 1, "pst": "A"}]
        });
        let obs = parse_zappi(&body, fixed_stamp()).unwrap();
        assert_eq!(obs.state.zappi_mode, ZappiMode::Off);
        assert_eq!(obs.state.zappi_plug_state, ZappiPlugState::EvDisconnected);
    }

    #[test]
    fn parses_zappi_unknown_status_code_propagates() {
        let body = json!({
            "zappi": [{"zmo": 2, "sta": 7, "pst": "B1"}]
        });
        let obs = parse_zappi(&body, fixed_stamp()).unwrap();
        assert_eq!(obs.state.zappi_status, ZappiStatus::Other(7));
    }

    #[test]
    fn parse_zappi_returns_none_on_missing_array() {
        let stamp = fixed_stamp();
        assert!(parse_zappi(&json!({}), stamp).is_none());
        assert!(parse_zappi(&json!({"zappi": []}), stamp).is_none());
    }

    #[test]
    fn parse_eddi_sta_1_is_normal() {
        // sta=1 is "Paused under Normal mode" — not an explicit Normal
        // indicator. The device is in Normal mode but has no surplus
        // right now, so it isn't actively diverting.
        let b = json!({"eddi":[{"sta": 1}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Normal));
    }

    #[test]
    fn parse_eddi_sta_0_is_stopped() {
        let b = json!({"eddi":[{"sta": 0}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Stopped));
    }

    #[test]
    fn parse_eddi_sta_3_is_normal_diverting() {
        let b = json!({"eddi":[{"sta": 3}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Normal));
    }

    #[test]
    fn parse_eddi_sta_4_is_normal_boost() {
        let b = json!({"eddi":[{"sta": 4}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Normal));
    }

    #[test]
    fn parse_eddi_sta_5_is_normal_max_temp() {
        let b = json!({"eddi":[{"sta": 5}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Normal));
    }

    #[test]
    fn parse_eddi_sta_6_is_stopped() {
        let b = json!({"eddi":[{"sta": 6}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Stopped));
    }

    #[test]
    fn parse_eddi_unknown_sta_defaults_to_normal_per_docs_mapping() {
        // Parser-layer default for unknown sta values is Normal: the
        // documented mapping says any non-Stopped operational status
        // implies the mode is Normal. The safety net for genuinely-bad
        // readings sits one layer up — the EddiController drives the
        // actuator to safe-Stopped on Stale/Unknown freshness.
        let b = json!({"eddi":[{"sta": 99}]});
        assert_eq!(parse_eddi(&b), Some(EddiMode::Normal));
    }

    #[test]
    fn change_tracker_latches_stamp_on_tuple_change() {
        let t0 = Instant::now();
        let mut tr = ZappiChangeTracker::new(
            (ZappiMode::Off, ZappiStatus::Paused, ZappiPlugState::EvDisconnected),
            t0,
        );

        // Same tuple → stamp unchanged.
        let t1 = t0 + Duration::from_secs(30);
        let s = tr.observe(
            (ZappiMode::Off, ZappiStatus::Paused, ZappiPlugState::EvDisconnected),
            t1,
        );
        assert_eq!(s, t0);

        // Different tuple → stamp latches to t2.
        let t2 = t0 + Duration::from_secs(60);
        let s = tr.observe(
            (ZappiMode::Eco, ZappiStatus::DivertingOrCharging, ZappiPlugState::Charging),
            t2,
        );
        assert_eq!(s, t2);

        // Same-as-just-stored tuple → stamp held at t2.
        let t3 = t0 + Duration::from_secs(90);
        let s = tr.observe(
            (ZappiMode::Eco, ZappiStatus::DivertingOrCharging, ZappiPlugState::Charging),
            t3,
        );
        assert_eq!(s, t2);
    }
}
