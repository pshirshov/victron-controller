//! Config file parsing. Loaded once at startup from
//! `/data/etc/victron-controller/config.toml` (or wherever
//! `--config` points).

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use victron_controller_core::HardwareParams;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub dbus: DbusConfig,
    #[serde(default)]
    pub mqtt: MqttConfig,
    #[serde(default)]
    pub myenergi: MyenergiConfig,
    #[serde(default)]
    pub forecast: ForecastConfig,
    #[serde(default)]
    pub dashboard: DashboardConfig,
    #[serde(default)]
    pub tuning: TuningConfig,
    #[serde(default)]
    pub outdoor_temperature_local: OutdoorTemperatureLocalConfig,
    /// PR-hardware-config: deploy-time hardware constants
    /// (inverter / breaker / grid voltage band / capacity model). Not
    /// runtime-tunable — see `core::topology::HardwareParams`.
    #[serde(default)]
    pub hardware: HardwareConfig,
    /// PR-ev-soc-sensor / PR-auto-extended-charge: optional MQTT
    /// bridge for EV state-of-charge + configured charge-target sensors
    /// published by an external integration (e.g. saic-python-mqtt-
    /// gateway). Same broker as `[mqtt]`. Each topic is independently
    /// optional — the bridge stays dormant for whichever is None.
    #[serde(default)]
    pub ev: EvConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbusConfig {
    /// When `true`, `Effect::WriteDbus` effects are executed. When
    /// `false`, they are logged but not emitted. Distinct from the
    /// runtime `writes_enabled` kill switch (which applies to all
    /// actuation effects including myenergi calls).
    #[serde(default = "default_true")]
    pub writes_enabled: bool,
    /// Override service bus names. When absent, defaults from the M1
    /// discovery (see SPEC §10.5) are used.
    #[serde(default)]
    pub services: Option<DbusServices>,
}

impl Default for DbusConfig {
    fn default() -> Self {
        Self {
            writes_enabled: true,
            services: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbusServices {
    pub system: String,
    pub settings: String,
    pub battery: String,
    pub mppt_0: String,
    pub mppt_1: String,
    pub pvinverter_soltaro: String,
    pub grid: String,
    pub vebus: String,
    pub evcharger: String,
}

impl DbusServices {
    /// Defaults matching the Venus v3.70 topology captured in SPEC §10.5.
    #[must_use]
    pub fn default_venus_3_70() -> Self {
        Self {
            system: "com.victronenergy.system".to_string(),
            settings: "com.victronenergy.settings".to_string(),
            battery: "com.victronenergy.battery.socketcan_can0".to_string(),
            mppt_0: "com.victronenergy.solarcharger.ttyUSB1".to_string(),
            mppt_1: "com.victronenergy.solarcharger.ttyS2".to_string(),
            pvinverter_soltaro: "com.victronenergy.pvinverter.cgwacs_ttyUSB2_mb1".to_string(),
            grid: "com.victronenergy.grid.cgwacs_ttyUSB0_mb1".to_string(),
            vebus: "com.victronenergy.vebus.ttyS3".to_string(),
            evcharger: "com.victronenergy.evcharger.cgwacs_ttyUSB0_mb2".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MqttConfig {
    /// Broker hostname; when empty, MQTT is disabled.
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    /// When `true`, use TLS. Requires `ca_path` to point at a PEM-
    /// encoded CA certificate the broker's server cert chains to.
    #[serde(default)]
    pub tls: bool,
    /// Path to a PEM-encoded CA certificate (or bundle). Only
    /// consulted when `tls = true`.
    #[serde(default)]
    pub ca_path: Option<String>,
    #[serde(default = "default_mqtt_root")]
    pub topic_root: String,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: default_mqtt_port(),
            username: None,
            password: None,
            tls: false,
            ca_path: None,
            topic_root: default_mqtt_root(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MyenergiConfig {
    /// myenergi account username.
    #[serde(default)]
    pub username: String,
    /// myenergi account password.
    #[serde(default)]
    pub password: String,
    /// Director region URL, e.g. `https://s18.myenergi.net`. Leave blank
    /// to disable the integration.
    #[serde(default)]
    pub director_url: String,
    /// Serial number of the Zappi to poll + write. Optional — if None,
    /// Zappi polling/writes are skipped.
    #[serde(default)]
    pub zappi_serial: Option<String>,
    /// Serial number of the Eddi. Optional.
    #[serde(default)]
    pub eddi_serial: Option<String>,
    /// Polling interval. Default 15 s — matches the legacy NR flow, but
    /// myenergi cloud returns ≤ 5-min-granular data per SPEC so this
    /// mostly drives reactivity to plug-state transitions rather than
    /// value freshness.
    #[serde(default = "default_myenergi_poll", with = "humantime_serde_compat")]
    pub poll_period: Duration,
    /// When `true`, `CallMyenergi(SetZappiMode|SetEddiMode)` effects are
    /// executed. When `false`, they are logged but not emitted —
    /// mirrors `[dbus] writes_enabled` but for the myenergi cloud
    /// HTTP path. Distinct from the runtime `writes_enabled` kill
    /// switch (which also gates these effects at the core layer).
    #[serde(default = "default_true")]
    pub writes_enabled: bool,
}

impl Default for MyenergiConfig {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            director_url: "https://s18.myenergi.net".to_string(),
            zappi_serial: None,
            eddi_serial: None,
            poll_period: default_myenergi_poll(),
            writes_enabled: true,
        }
    }
}

fn default_myenergi_poll() -> Duration {
    Duration::from_secs(15)
}

// -----------------------------------------------------------------------------
// Forecast config — three providers, each optional.
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ForecastConfig {
    #[serde(default)]
    pub solcast: SolcastProviderConfig,
    #[serde(default)]
    pub forecast_solar: ForecastSolarProviderConfig,
    #[serde(default)]
    pub open_meteo: OpenMeteoProviderConfig,
    /// IANA name of the site's timezone — used both when querying
    /// Open-Meteo (`timezone=…`) and when bucketing Solcast / Open-Meteo
    /// responses into today/tomorrow. Do NOT rely on the machine TZ:
    /// Venus OS runs UTC by default, so a site-local forecast returned
    /// by `timezone=auto` gets mis-bucketed against `Local::now()` by
    /// the site's UTC offset (A-50).
    #[serde(default = "default_forecast_timezone")]
    pub timezone: String,
}

impl Default for ForecastConfig {
    fn default() -> Self {
        Self {
            solcast: SolcastProviderConfig::default(),
            forecast_solar: ForecastSolarProviderConfig::default(),
            open_meteo: OpenMeteoProviderConfig::default(),
            timezone: default_forecast_timezone(),
        }
    }
}

fn default_forecast_timezone() -> String {
    "Europe/London".to_string()
}

impl ForecastConfig {
    /// Parse the configured IANA timezone string. Called at startup so
    /// a typo fails fast rather than silently falling back to UTC
    /// during the first forecast fetch.
    pub fn parse_timezone(&self) -> Result<chrono_tz::Tz> {
        self.timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|e| anyhow::anyhow!(
                "[forecast] timezone = {:?} is not a valid IANA TZ name: {e}",
                self.timezone
            ))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SolcastProviderConfig {
    /// API key. Blank ⇒ provider disabled.
    #[serde(default)]
    pub api_key: String,
    /// Rooftop site IDs (up to 2 on free tier). Empty ⇒ disabled.
    #[serde(default)]
    pub site_ids: Vec<String>,
    /// Poll cadence. Free tier = 10 calls/day/site; default = 2 h.
    #[serde(
        default = "default_solcast_cadence",
        with = "humantime_serde_compat"
    )]
    pub cadence: Duration,
}

fn default_solcast_cadence() -> Duration {
    Duration::from_secs(2 * 60 * 60)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForecastSolarProviderConfig {
    /// Latitude / longitude of the site.
    #[serde(default)]
    pub latitude: f64,
    #[serde(default)]
    pub longitude: f64,
    /// Representative planes. Empty ⇒ provider disabled.
    #[serde(default)]
    pub planes: Vec<PlaneConfig>,
    /// Poll cadence. Free tier rate-limited at ~12 req/h/IP. Default 1 h.
    #[serde(
        default = "default_forecast_solar_cadence",
        with = "humantime_serde_compat"
    )]
    pub cadence: Duration,
}

impl Default for ForecastSolarProviderConfig {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            planes: Vec::new(),
            cadence: default_forecast_solar_cadence(),
        }
    }
}

fn default_forecast_solar_cadence() -> Duration {
    Duration::from_secs(60 * 60)
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenMeteoProviderConfig {
    #[serde(default)]
    pub latitude: f64,
    #[serde(default)]
    pub longitude: f64,
    #[serde(default)]
    pub planes: Vec<PlaneConfig>,
    /// Poll cadence. No rate limit; default 30 min (see
    /// `default_open_meteo_cadence` below for rationale — Open-Meteo
    /// is free and the temperature signal moves slowly, so finer-
    /// grained polls would waste calls without improving control).
    #[serde(
        default = "default_open_meteo_cadence",
        with = "humantime_serde_compat"
    )]
    pub cadence: Duration,
    /// Combined panel × inverter × BOS efficiency, applied to raw
    /// Open-Meteo irradiance before summing. Default 0.75 matches
    /// the legacy hard-coded constant. A-43: exposed so the user can
    /// normalize Open-Meteo's output against Forecast.Solar / Solcast
    /// when calibrating weather_soc thresholds. Values outside
    /// [0.1, 1.0] are treated as configuration errors.
    #[serde(default = "default_open_meteo_system_efficiency")]
    pub system_efficiency: f64,
}

impl Default for OpenMeteoProviderConfig {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            planes: Vec::new(),
            cadence: default_open_meteo_cadence(),
            system_efficiency: default_open_meteo_system_efficiency(),
        }
    }
}

fn default_open_meteo_system_efficiency() -> f64 {
    0.75
}

fn default_open_meteo_cadence() -> Duration {
    // 30 min. Covers both the solar-irradiance forecast (slow-moving,
    // no need to hit it more often) and the current-temperature poll,
    // which feeds `SensorId::OutdoorTemperature.freshness_threshold()`
    // (40 min) in the core types — one fetch every cadence, ~10 min
    // of fresh headroom.
    Duration::from_secs(30 * 60)
}

/// One PV plane from config. `azimuth_deg` follows the compass
/// convention: 0=N, 90=E, 180=S, 270=W.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PlaneConfig {
    pub tilt_deg: f64,
    pub azimuth_deg: f64,
    pub kwp: f64,
}

impl From<PlaneConfig> for crate::forecast::Plane {
    fn from(p: PlaneConfig) -> Self {
        Self {
            tilt_deg: p.tilt_deg,
            azimuth_deg: p.azimuth_deg,
            kwp: p.kwp,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashboardConfig {
    #[serde(default = "default_dashboard_port")]
    pub port: u16,
    #[serde(default = "default_dashboard_bind")]
    pub bind: String,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            port: default_dashboard_port(),
            bind: default_dashboard_bind(),
        }
    }
}

/// PR-matter-outdoor-temp: optional MQTT bridge for a Matter
/// outdoor-temperature sensor (e.g. a Meross Smart Hub publishing
/// raw cluster attributes). Same broker/credentials as `[mqtt]`. When
/// `mqtt_topic` is `None`, this path is silent — Open-Meteo's current-
/// weather poller remains the sole `outdoor_temperature` source.
#[derive(Debug, Clone, Deserialize)]
pub struct OutdoorTemperatureLocalConfig {
    /// Optional MQTT topic publishing local outdoor temperature.
    /// Body must be a JSON-encoded int (centi-Celsius units, e.g.
    /// `1640` for 16.4°C, signed int16 range [-27315, 32767]).
    /// `null` / non-numeric bodies are silently dropped (the Meross
    /// hub publishes `null` between low-power reads).
    #[serde(default)]
    pub mqtt_topic: Option<String>,
    /// Sanity bounds. Readings outside are dropped as glitches.
    /// Defaults: -50.0 / 80.0.
    #[serde(default = "default_min_celsius")]
    pub min_celsius: f64,
    #[serde(default = "default_max_celsius")]
    pub max_celsius: f64,
}

impl Default for OutdoorTemperatureLocalConfig {
    fn default() -> Self {
        Self {
            mqtt_topic: None,
            min_celsius: default_min_celsius(),
            max_celsius: default_max_celsius(),
        }
    }
}

fn default_min_celsius() -> f64 {
    -50.0
}
fn default_max_celsius() -> f64 {
    80.0
}

/// PR-ev-soc-sensor / PR-auto-extended-charge: external MQTT
/// publisher providing the EV state-of-charge and the EV's configured
/// charge-target SoC. Each field is the publisher's HA-discovery
/// config topic. For each `Some` topic the shell subscribes to the
/// discovery topic, parses the retained JSON for `state_topic`, then
/// subscribes to `state_topic` for the actual readings. When the
/// field is `None`, that path stays dormant — no subscription, no log
/// noise, and the corresponding sensor slot remains `Unknown`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EvConfig {
    /// HA-discovery config topic for the EV's State of Charge sensor.
    #[serde(default)]
    pub soc_topic: Option<String>,
    /// HA-discovery config topic for the EV's configured charge-target
    /// SoC. Used by the auto-extended-charge logic in the `Auto` mode
    /// of `evcharger.extended`.
    #[serde(default)]
    pub charge_target_topic: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TuningConfig {
    /// Heartbeat for freshness decay + periodic controller re-evaluation.
    #[serde(default = "default_tick_period", with = "humantime_serde_compat")]
    pub tick_period: Duration,
}

impl Default for TuningConfig {
    fn default() -> Self {
        Self {
            tick_period: default_tick_period(),
        }
    }
}

// -----------------------------------------------------------------------------
// Hardware (deploy-time constants — see SPEC §7 / PR-hardware-config)
// -----------------------------------------------------------------------------

/// Deploy-time hardware constants. Promoted out of per-controller
/// `const`s so a different physical install can override them without
/// recompiling the core. NOT runtime knobs — they don't appear on the
/// dashboard, are not retained in MQTT, and should not change at
/// runtime. The wire form here uses positive magnitudes throughout
/// (e.g. `inverter_max_discharge_w = 5000`); the `From` impl below
/// flips the sign when feeding `HardwareParams`, which stores
/// `inverter_max_discharge_w` as the negative floor that
/// `prepare_setpoint` actually consumes.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct HardwareConfig {
    /// MultiPlus AC-export ceiling (positive magnitude). Default 5000.
    #[serde(default = "default_inverter_max_discharge_w")]
    pub inverter_max_discharge_w: u32,
    /// Margin below the inverter "forced grid charge above ~4.8 kW"
    /// glitch — used by setpoint's `max_discharge` formula. Default 4020.
    #[serde(default = "default_inverter_safe_discharge_w")]
    pub inverter_safe_discharge_w: u32,
    /// Main breaker rating ceiling (A) for the current-limit
    /// controller. Default 65.
    #[serde(default = "default_max_grid_current_a")]
    pub max_grid_current_a: u32,
    /// Floor — keeps inverter aux fed. Default 10.
    #[serde(default = "default_min_system_current_a")]
    pub min_system_current_a: u32,
    /// Forced-import baseline (Soltaro 23:55 quirk). Default 10.
    #[serde(default = "default_idle_setpoint_w")]
    pub idle_setpoint_w: u32,
    /// Evening planner: `preserve_battery` baseload threshold.
    /// Default 1200.
    #[serde(default = "default_baseload_consumption_w")]
    pub baseload_consumption_w: u32,
    /// Caps `grid_export_limit_w` knob. ESB G99 typical authorisation
    /// = 6000 W.
    #[serde(default = "default_grid_export_knob_max_w")]
    pub grid_export_knob_max_w: u32,
    /// Caps `grid_import_limit_w` knob. MultiPlus continuous import
    /// capability ≈ 13 000 W.
    #[serde(default = "default_grid_import_knob_max_w")]
    pub grid_import_knob_max_w: u32,
    /// Pylontech 48 V stack — capacity model nominal voltage.
    #[serde(default = "default_battery_nominal_voltage_v")]
    pub battery_nominal_voltage_v: f64,
    /// EN 50160 nominal grid voltage. Default 230.0.
    #[serde(default = "default_grid_nominal_voltage_v")]
    pub grid_nominal_voltage_v: f64,
    /// EN 50160 -10% sanity floor. Default 207.0.
    #[serde(default = "default_grid_min_sensible_voltage_v")]
    pub grid_min_sensible_voltage_v: f64,
    /// EN 50160 +10% + ~7 V noise headroom — sanity ceiling.
    /// Default 260.0.
    #[serde(default = "default_grid_max_sensible_voltage_v")]
    pub grid_max_sensible_voltage_v: f64,
}

impl Default for HardwareConfig {
    fn default() -> Self {
        Self {
            inverter_max_discharge_w: default_inverter_max_discharge_w(),
            inverter_safe_discharge_w: default_inverter_safe_discharge_w(),
            max_grid_current_a: default_max_grid_current_a(),
            min_system_current_a: default_min_system_current_a(),
            idle_setpoint_w: default_idle_setpoint_w(),
            baseload_consumption_w: default_baseload_consumption_w(),
            grid_export_knob_max_w: default_grid_export_knob_max_w(),
            grid_import_knob_max_w: default_grid_import_knob_max_w(),
            battery_nominal_voltage_v: default_battery_nominal_voltage_v(),
            grid_nominal_voltage_v: default_grid_nominal_voltage_v(),
            grid_min_sensible_voltage_v: default_grid_min_sensible_voltage_v(),
            grid_max_sensible_voltage_v: default_grid_max_sensible_voltage_v(),
        }
    }
}

impl From<HardwareConfig> for HardwareParams {
    fn from(c: HardwareConfig) -> Self {
        Self {
            // Sign-flip: the controller stores this as the negative
            // floor used in `prepare_setpoint(max_discharge, …)`.
            inverter_max_discharge_w: -f64::from(c.inverter_max_discharge_w),
            inverter_safe_discharge_w: f64::from(c.inverter_safe_discharge_w),
            max_grid_current_a: f64::from(c.max_grid_current_a),
            min_system_current_a: f64::from(c.min_system_current_a),
            idle_setpoint_w: f64::from(c.idle_setpoint_w),
            baseload_consumption_w: f64::from(c.baseload_consumption_w),
            grid_export_knob_max_w: c.grid_export_knob_max_w,
            grid_import_knob_max_w: c.grid_import_knob_max_w,
            battery_nominal_voltage_v: c.battery_nominal_voltage_v,
            grid_nominal_voltage_v: c.grid_nominal_voltage_v,
            grid_min_sensible_voltage_v: c.grid_min_sensible_voltage_v,
            grid_max_sensible_voltage_v: c.grid_max_sensible_voltage_v,
        }
    }
}

fn default_inverter_max_discharge_w() -> u32 {
    5000
}
fn default_inverter_safe_discharge_w() -> u32 {
    4020
}
fn default_max_grid_current_a() -> u32 {
    65
}
fn default_min_system_current_a() -> u32 {
    10
}
fn default_idle_setpoint_w() -> u32 {
    10
}
fn default_baseload_consumption_w() -> u32 {
    1200
}
fn default_grid_export_knob_max_w() -> u32 {
    6000
}
fn default_grid_import_knob_max_w() -> u32 {
    13000
}
fn default_battery_nominal_voltage_v() -> f64 {
    48.0
}
fn default_grid_nominal_voltage_v() -> f64 {
    230.0
}
fn default_grid_min_sensible_voltage_v() -> f64 {
    207.0
}
fn default_grid_max_sensible_voltage_v() -> f64 {
    260.0
}

// --- defaults ---

fn default_true() -> bool {
    true
}
fn default_mqtt_port() -> u16 {
    1883
}
fn default_mqtt_root() -> String {
    "victron-controller".to_string()
}
fn default_dashboard_port() -> u16 {
    8910
}
fn default_dashboard_bind() -> String {
    "0.0.0.0".to_string()
}
fn default_tick_period() -> Duration {
    Duration::from_secs(1)
}

// Tiny embedded alternative to the `humantime-serde` crate — avoids one
// more dependency.
mod humantime_serde_compat {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        // Accept either integer seconds or an "Ns"/"Nms" string.
        let v = toml::Value::deserialize(d)?;
        match v {
            toml::Value::Integer(n) if n >= 0 => {
                Ok(Duration::from_secs(u64::try_from(n).unwrap_or(1)))
            }
            toml::Value::String(s) => parse_human(&s).map_err(serde::de::Error::custom),
            other => Err(serde::de::Error::custom(format!(
                "expected integer seconds or duration string, got {other:?}"
            ))),
        }
    }

    pub(crate) fn parse_human(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if let Some(n) = s.strip_suffix("ms") {
            n.trim()
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|e| e.to_string())
        } else if let Some(n) = s.strip_suffix('s') {
            n.trim()
                .parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|e| e.to_string())
        } else if let Some(n) = s.strip_suffix('m') {
            n.trim()
                .parse::<u64>()
                .map(|m| Duration::from_secs(m * 60))
                .map_err(|e| e.to_string())
        } else if let Some(n) = s.strip_suffix('h') {
            n.trim()
                .parse::<u64>()
                .map(|h| Duration::from_secs(h * 3600))
                .map_err(|e| e.to_string())
        } else if let Some(n) = s.strip_suffix('d') {
            n.trim()
                .parse::<u64>()
                .map(|d| Duration::from_secs(d * 86400))
                .map_err(|e| e.to_string())
        } else {
            s.parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|e| e.to_string())
        }
    }
}

/// Load + parse a TOML config from disk. An empty/missing file yields
/// all defaults.
pub fn load(path: &Path) -> Result<Config> {
    let cfg: Config = if path.exists() {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?
    } else {
        tracing::warn!(
            "config file {} does not exist; using defaults",
            path.display()
        );
        Config::default()
    };
    // A-50: validate the forecast TZ string at startup so a typo fails
    // fast instead of silently poisoning the today/tomorrow buckets on
    // the first fetch.
    cfg.forecast.parse_timezone()?;
    Ok(cfg)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dbus: DbusConfig::default(),
            mqtt: MqttConfig::default(),
            myenergi: MyenergiConfig::default(),
            forecast: ForecastConfig::default(),
            dashboard: DashboardConfig::default(),
            tuning: TuningConfig::default(),
            outdoor_temperature_local: OutdoorTemperatureLocalConfig::default(),
            hardware: HardwareConfig::default(),
            ev: EvConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_yields_safe_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert!(c.dbus.writes_enabled);
        assert_eq!(c.dashboard.port, 8910);
        assert_eq!(c.dashboard.bind, "0.0.0.0");
        assert_eq!(c.mqtt.port, 1883);
        assert_eq!(c.mqtt.topic_root, "victron-controller");
        assert_eq!(c.tuning.tick_period, Duration::from_secs(1));
    }

    #[test]
    fn parses_full_config() {
        let t = r#"
            [dbus]
            writes_enabled = false

            [mqtt]
            host = "mqtt.example.invalid"
            port = 8883
            username = "svc"
            password = "secret"
            tls = true
            topic_root = "victron-ctrl-test"

            [dashboard]
            port = 9000
            bind = "127.0.0.1"

            [tuning]
            tick_period = "5s"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert!(!c.dbus.writes_enabled);
        assert_eq!(c.mqtt.host, "mqtt.example.invalid");
        assert_eq!(c.mqtt.port, 8883);
        assert_eq!(c.mqtt.username.as_deref(), Some("svc"));
        assert!(c.mqtt.tls);
        assert_eq!(c.mqtt.topic_root, "victron-ctrl-test");
        assert_eq!(c.dashboard.port, 9000);
        assert_eq!(c.dashboard.bind, "127.0.0.1");
        assert_eq!(c.tuning.tick_period, Duration::from_secs(5));
    }

    #[test]
    fn parses_hour_and_day_suffixes() {
        use super::humantime_serde_compat::parse_human;
        assert_eq!(parse_human("2h"), Ok(Duration::from_secs(7200)));
        assert_eq!(parse_human("1h"), Ok(Duration::from_secs(3600)));
        assert_eq!(parse_human("15m"), Ok(Duration::from_secs(900)));
        assert_eq!(parse_human("1d"), Ok(Duration::from_secs(86400)));
    }

    #[test]
    fn forecast_timezone_defaults_to_europe_london() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.forecast.timezone, "Europe/London");
        let tz = c.forecast.parse_timezone().unwrap();
        assert_eq!(tz, chrono_tz::Europe::London);
    }

    #[test]
    fn forecast_timezone_accepts_custom_iana_name() {
        let t = r#"
            [forecast]
            timezone = "America/Los_Angeles"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        let tz = c.forecast.parse_timezone().unwrap();
        assert_eq!(tz, chrono_tz::America::Los_Angeles);
    }

    #[test]
    fn forecast_timezone_rejects_invalid_name() {
        let t = r#"
            [forecast]
            timezone = "Atlantis/R'lyeh"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        let err = c.forecast.parse_timezone().unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Atlantis/R'lyeh"),
            "error must mention the offending value, got: {msg}"
        );
    }

    #[test]
    fn outdoor_temperature_local_absent_yields_silent_defaults() {
        // PR-matter-outdoor-temp: the Open-Meteo path is the sole
        // outdoor_temperature source when the section is absent.
        let c: Config = toml::from_str("").unwrap();
        assert!(c.outdoor_temperature_local.mqtt_topic.is_none());
        assert!((c.outdoor_temperature_local.min_celsius - -50.0).abs() < f64::EPSILON);
        assert!((c.outdoor_temperature_local.max_celsius - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn outdoor_temperature_local_parses_section() {
        let t = r#"
            [outdoor_temperature_local]
            mqtt_topic  = "matter/1/attributes/107_1026_0"
            min_celsius = -20.0
            max_celsius = 50.0
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(
            c.outdoor_temperature_local.mqtt_topic.as_deref(),
            Some("matter/1/attributes/107_1026_0")
        );
        assert!((c.outdoor_temperature_local.min_celsius - -20.0).abs() < f64::EPSILON);
        assert!((c.outdoor_temperature_local.max_celsius - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dbus_services_default_matches_venus_3_70_discovery() {
        let s = DbusServices::default_venus_3_70();
        assert!(s.battery.contains("socketcan_can0"));
        assert!(s.evcharger.contains("evcharger.cgwacs_ttyUSB0_mb2"));
        assert!(s.vebus.contains("vebus.ttyS3"));
    }
}
