//! Config file parsing. Loaded once at startup from
//! `/data/etc/victron-controller/config.toml` (or wherever
//! `--config` points).

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub dbus: DbusConfig,
    #[serde(default)]
    pub mqtt: MqttConfig,
    #[serde(default)]
    pub myenergi: MyenergiConfig,
    #[serde(default)]
    pub dashboard: DashboardConfig,
    #[serde(default)]
    pub tuning: TuningConfig,
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
    #[serde(default)]
    pub tls: bool,
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
        }
    }
}

fn default_myenergi_poll() -> Duration {
    Duration::from_secs(15)
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

    fn parse_human(s: &str) -> Result<Duration, String> {
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
    if !path.exists() {
        tracing::warn!(
            "config file {} does not exist; using defaults",
            path.display()
        );
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dbus: DbusConfig::default(),
            mqtt: MqttConfig::default(),
            myenergi: MyenergiConfig::default(),
            dashboard: DashboardConfig::default(),
            tuning: TuningConfig::default(),
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
    fn dbus_services_default_matches_venus_3_70_discovery() {
        let s = DbusServices::default_venus_3_70();
        assert!(s.battery.contains("socketcan_can0"));
        assert!(s.evcharger.contains("evcharger.cgwacs_ttyUSB0_mb2"));
        assert!(s.vebus.contains("vebus.ttyS3"));
    }
}
