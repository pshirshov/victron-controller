//! LG ThinQ heat-pump integration — TASS-integrated (Phase B).
//!
//! Three components:
//!
//! - [`Client`] — wraps the `lg-thinq-client` crate API. Shared by
//!   [`Writer`] and [`Poller`].
//! - [`Writer`] — accepts `Effect::CallLgThinq(LgThinqAction)` and
//!   posts the corresponding control call to LG ThinQ Connect.
//! - [`Poller`] — polls device state on a configurable cadence (default
//!   60 s), parses the `HeatPumpState`, and emits six `Event::Sensor`
//!   readings into the core event channel:
//!   - `LgHeatPumpPowerActual` (bool readback → 1.0/0.0)
//!   - `LgDhwPowerActual` (bool readback → 1.0/0.0)
//!   - `LgHeatingWaterTargetActual` (f64, target °C)
//!   - `LgDhwTargetActual` (f64, target °C)
//!   - `LgDhwCurrentTemperatureC` (f64, current DHW tank °C)
//!   - `LgHeatingWaterCurrentTemperatureC` (f64, current loop °C)
//!
//! HA discovery for these entities flows through the main
//! `mqtt::discovery::publish_ha_discovery` path via the new `KnobId`
//! / `ActuatedId` / `SensorId` variants added in D03 — the old
//! `lg_thinq/discovery.rs` file is deleted.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use victron_controller_lg_thinq_client::api::ThinqApi;
use victron_controller_lg_thinq_client::heat_pump::{HeatPumpControl, HeatPumpState};
use victron_controller_lg_thinq_client::region::Country;
use victron_controller_core::types::{Event, LgThinqAction, SensorId, SensorReading, TimerId, TimerStatus};

use crate::config::LgThinqConfig;

// =============================================================================
// Client
// =============================================================================

/// Shared API client — wraps the `ThinqApi` from `lg-thinq-client`.
/// Cheap to clone (the underlying `reqwest::Client` is `Arc`-backed).
#[derive(Clone)]
pub struct Client {
    api: ThinqApi,
    device_id: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `api` holds bearer token — omitted for safety.
        f.debug_struct("lg_thinq::Client")
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Build a `Client` from the `[lg_thinq]` section of the config.
    ///
    /// Returns `Err` when the config is incomplete or the country code
    /// is not recognised.
    pub fn new(cfg: &LgThinqConfig) -> Result<Self> {
        let country = Country::new(&cfg.country)
            .map_err(|e| anyhow!("[lg_thinq] country {:?}: {e}", cfg.country))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .context("build reqwest client")?;
        let api = ThinqApi::new(http, cfg.pat.clone(), &country)
            .map_err(|e| anyhow!("[lg_thinq] ThinqApi::new: {e}"))?;
        Ok(Self {
            api,
            device_id: cfg.device_id.clone(),
        })
    }

    /// Fetch the current device state.
    pub async fn get_state(&self) -> Result<HeatPumpState> {
        let raw = self
            .api
            .get_device_state(&self.device_id)
            .await
            .map_err(|e| anyhow!("get_device_state: {e}"))?;
        HeatPumpState::from_json(&raw).map_err(|e| anyhow!("decode state: {e}"))
    }

    /// Post a control payload to LG ThinQ.
    async fn post_control(&self, payload: serde_json::Value) -> Result<()> {
        self.api
            .post_device_control(&self.device_id, payload)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("post_device_control: {e}"))
    }
}

// =============================================================================
// Writer — executes CallLgThinq effects
// =============================================================================

/// Executes `Effect::CallLgThinq(LgThinqAction)` effects. Spawned as a
/// sibling task in the runtime's effect dispatcher (see `runtime.rs`).
#[derive(Debug, Clone)]
pub struct Writer {
    client: Client,
    /// When `true`, actions are logged but not sent to LG.
    dry_run: bool,
}

impl Writer {
    #[must_use]
    pub const fn new(client: Client, dry_run: bool) -> Self {
        Self { client, dry_run }
    }

    /// Execute a single `LgThinqAction`. Logs the outcome.
    pub async fn execute(&self, action: LgThinqAction) {
        if self.dry_run {
            info!(?action, "lg_thinq action (dry-run; writes_enabled=false, not sent)");
            return;
        }
        let payload = action_to_payload(action);
        match self.client.post_control(payload).await {
            Ok(()) => info!(?action, "lg_thinq action applied"),
            Err(e) => warn!(?action, error = %e, "lg_thinq action failed"),
        }
    }
}

/// Map a `LgThinqAction` to the exact JSON payload posted to LG's
/// `/devices/{id}/control` endpoint. Extracted from `Writer::execute`
/// so the routing can be tested directly without a recording fake
/// HTTP client. The payload-builder primitives
/// (`HeatPumpControl::set_*`) are themselves tested in the
/// `lg-thinq-client` crate.
pub(crate) fn action_to_payload(action: LgThinqAction) -> serde_json::Value {
    match action {
        LgThinqAction::SetHeatPumpPower(on) => HeatPumpControl::set_heating_power(on),
        LgThinqAction::SetDhwPower(on) => HeatPumpControl::set_dhw_power(on),
        LgThinqAction::SetHeatingWaterTargetC(t) => HeatPumpControl::set_water_heat_target_c(t),
        LgThinqAction::SetDhwTargetC(t) => HeatPumpControl::set_dhw_target_c(t),
    }
}

// =============================================================================
// Poller — emits Event::Sensor readings
// =============================================================================

/// Polls device state periodically and emits six `Event::Sensor`
/// readings into the core event channel. Mirrors
/// `myenergi::Poller::run` for the four actuated-mirror sensors and the
/// two plain temperature sensors.
#[derive(Debug)]
pub struct Poller {
    client: Client,
    poll_period: Duration,
}

impl Poller {
    #[must_use]
    pub const fn new(client: Client, poll_period: Duration) -> Self {
        Self { client, poll_period }
    }

    pub async fn run(self, tx: mpsc::Sender<Event>) -> Result<()> {
        info!(
            period_s = self.poll_period.as_secs(),
            device_id = %self.client.device_id,
            "lg_thinq poller started"
        );
        let mut ticker = tokio::time::interval(self.poll_period);
        loop {
            ticker.tick().await;
            let outcome = self.poll_once(&tx).await;
            // PR-timers-section: emit a TimerState per cycle.
            let timer_status = if outcome { TimerStatus::Idle } else { TimerStatus::FailedLastRun };
            let last_fire_ms = epoch_ms_now();
            let next_fire_ms = Some(
                last_fire_ms
                    + i64::try_from(self.poll_period.as_millis()).unwrap_or(i64::MAX),
            );
            if tx
                .send(Event::TimerState {
                    id: TimerId::LgThinqPoller,
                    last_fire_epoch_ms: last_fire_ms,
                    next_fire_epoch_ms: next_fire_ms,
                    status: timer_status,
                    at: Instant::now(),
                })
                .await
                .is_err()
            {
                return Ok(());
            }
        }
    }

    /// Execute one poll cycle. Returns `true` on success, `false` on error.
    async fn poll_once(&self, tx: &mpsc::Sender<Event>) -> bool {
        let now = Instant::now();
        let state = match self.client.get_state().await {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "lg_thinq state poll failed; will retry");
                return false;
            }
        };

        // Emit all six sensor readings. The four actuated-mirror sensors
        // arrive as `f64` (1.0=true, 0.0=false); `apply_sensor_reading`
        // in `process.rs` converts them via the post-hook
        // (`v != 0.0` for bool, `v as i32` for temperatures).
        let readings: [Option<(SensorId, f64)>; 6] = [
            // Bool actuated-mirrors (1.0 = true, 0.0 = false).
            Some((
                SensorId::LgHeatPumpPowerActual,
                if state.heating_enabled { 1.0 } else { 0.0 },
            )),
            Some((
                SensorId::LgDhwPowerActual,
                if state.dhw_enabled { 1.0 } else { 0.0 },
            )),
            // i32 actuated-mirrors (target temperatures).
            state.heating_water_target_c.map(|t| (SensorId::LgHeatingWaterTargetActual, t)),
            state.dhw_target_c.map(|t| (SensorId::LgDhwTargetActual, t)),
            // Plain temperature sensors.
            state.dhw_current_c.map(|t| (SensorId::LgDhwCurrentTemperatureC, t)),
            state.heating_water_current_c.map(|t| (SensorId::LgHeatingWaterCurrentTemperatureC, t)),
        ];

        for entry in readings.into_iter().flatten() {
            let (id, value) = entry;
            if tx
                .send(Event::Sensor(SensorReading { id, value, at: now }))
                .await
                .is_err()
            {
                // Channel closed → runtime shutting down.
                return true;
            }
        }

        debug!(
            heating_enabled = state.heating_enabled,
            dhw_enabled = state.dhw_enabled,
            dhw_current_c = ?state.dhw_current_c,
            heating_water_current_c = ?state.heating_water_current_c,
            "lg_thinq poll ok"
        );
        true
    }
}

fn epoch_ms_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    //! PR-LG-THINQ-B-1-D04: lock the LgThinqAction → JSON payload
    //! mapping against the exact wire shape LG's
    //! `/devices/{id}/control` endpoint expects. The underlying
    //! payload-builder primitives (`HeatPumpControl::set_*`) are also
    //! tested in the `lg-thinq-client` crate; these tests pin the
    //! *routing* — that the right LgThinqAction variant produces the
    //! right payload.

    use super::action_to_payload;
    use serde_json::json;
    use victron_controller_core::types::LgThinqAction;

    #[test]
    fn writer_set_heat_pump_power_true_posts_power_on_envelope() {
        let p = action_to_payload(LgThinqAction::SetHeatPumpPower(true));
        assert_eq!(p, json!({"operation": {"boilerOperationMode": "POWER_ON"}}));
    }

    #[test]
    fn writer_set_heat_pump_power_false_posts_power_off_envelope() {
        let p = action_to_payload(LgThinqAction::SetHeatPumpPower(false));
        assert_eq!(p, json!({"operation": {"boilerOperationMode": "POWER_OFF"}}));
    }

    #[test]
    fn writer_set_dhw_power_true_posts_on_envelope() {
        let p = action_to_payload(LgThinqAction::SetDhwPower(true));
        assert_eq!(p, json!({"operation": {"hotWaterMode": "ON"}}));
    }

    #[test]
    fn writer_set_dhw_power_false_posts_off_envelope() {
        let p = action_to_payload(LgThinqAction::SetDhwPower(false));
        assert_eq!(p, json!({"operation": {"hotWaterMode": "OFF"}}));
    }

    #[test]
    fn writer_set_heating_water_target_c_posts_room_temperature_envelope() {
        let p = action_to_payload(LgThinqAction::SetHeatingWaterTargetC(48));
        assert_eq!(
            p,
            json!({
                "roomTemperatureInUnits": {
                    "waterHeatTargetTemperature": 48,
                    "unit": "C"
                }
            })
        );
    }

    #[test]
    fn writer_set_dhw_target_c_posts_hot_water_envelope() {
        let p = action_to_payload(LgThinqAction::SetDhwTargetC(60));
        assert_eq!(
            p,
            json!({
                "hotWaterTemperatureInUnits": {
                    "targetTemperature": 60,
                    "unit": "C"
                }
            })
        );
    }
}
