//! Config file parsing. Loaded once at startup from
//! `/data/etc/victron-controller/config.toml` (or wherever
//! `--config` points).

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer};
use victron_controller_core::HardwareParams;
use victron_controller_core::knobs::{
    ChargeBatteryExtendedMode as CoreChargeBatteryExtendedMode,
    DebugFullCharge as CoreDebugFullCharge, DischargeTime as CoreDischargeTime,
    ExtendedChargeMode as CoreExtendedChargeMode,
    ForecastDisagreementStrategy as CoreForecastDisagreementStrategy, Knobs,
    Mode as CoreMode,
};

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
    /// PR-ZD-1: optional zigbee2mqtt bridge for house appliance power
    /// sensors (heat pump + cooker). Each topic is independently optional.
    #[serde(default)]
    pub zigbee2mqtt: Zigbee2MqttConfig,
    /// PR-pinned-registers: list of (path, type, value) triplets the
    /// shell re-asserts on a 1 h cadence. Any register listed here is
    /// read once an hour; if the bus value differs from the configured
    /// constant, an `Effect::WriteDbus` is emitted to put it back.
    /// Drift counters and last-drift / last-check timestamps are
    /// surfaced on the dashboard. Empty by default — opt-in feature.
    /// Goes through the existing `[dbus] writes_enabled` chokepoint.
    #[serde(default)]
    pub dbus_pinned_registers: Vec<DbusPinnedRegister>,
    /// Cold-start overrides for `Knobs::safe_defaults`. Each field is
    /// optional — anything left absent keeps the SPEC §7 baseline.
    /// Retained MQTT values still win on the next boot; this section
    /// only changes the *seed* used before any retained value arrives.
    #[serde(default)]
    pub knobs: KnobsDefaultsConfig,
    /// PR-keep-batteries-charged: site location used by the always-on
    /// sunrise/sunset scheduler. Independent of `[forecast.baseline]`
    /// (which retains its own copy for back-compat); when this section
    /// is absent the sunrise/sunset scheduler does not start and
    /// `world.sunrise` / `world.sunset` stay at their default `None`,
    /// which the ESS-state override controller treats as
    /// "bias-to-safety, no write".
    #[serde(default)]
    pub location: LocationConfig,
    /// LG ThinQ heat-pump cloud bridge (HM051M.U43). Optional sidecar:
    /// when `pat` is non-empty, the shell spawns a self-contained
    /// subscriber that maps `victron-controller/knob/lg_*/set` MQTT
    /// commands to LG cloud control calls and publishes readback state.
    /// Does not yet integrate with the core TASS surfaces — see
    /// `crates/shell/src/lg_thinq/mod.rs`.
    #[serde(default)]
    pub lg_thinq: LgThinqConfig,
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
// LG ThinQ heat-pump bridge (Option A — self-contained sidecar).
// -----------------------------------------------------------------------------

/// LG ThinQ Connect bridge config. Dormant when `pat` is empty.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LgThinqConfig {
    /// Personal Access Token from the LG ThinQ Developer site
    /// (<https://thinq.developer.lge.com/>). Empty ⇒ bridge disabled.
    #[serde(default)]
    pub pat: String,
    /// ISO-3166-1 alpha-2 country code matching the LG account
    /// (e.g. `"IE"`, `"DE"`). Determines the regional API endpoint.
    #[serde(default)]
    pub country: String,
    /// LG device id of the heat pump. Find it via the test command
    /// `victron-controller --dump-lg-devices` (TODO) or by inspecting
    /// the HA `lg_thinq` integration. Empty ⇒ bridge disabled.
    #[serde(default)]
    pub device_id: String,
    /// Persistent directory for the MQTT-push cert bundle. Survives
    /// firmware updates only if it lives under `/data/...`. Unused in
    /// Option A (no MQTT push yet) but kept in config so Option B
    /// can adopt it without a TOML migration.
    #[serde(default = "default_lg_thinq_cache_dir")]
    pub cache_dir: String,
    /// When `false`, control commands are logged but not sent. Mirrors
    /// the `[myenergi] writes_enabled` gate.
    #[serde(default = "default_true")]
    pub writes_enabled: bool,
    /// State-poll cadence. LG's free tier allows generous quota but
    /// the heat pump itself only changes state on minute timescales,
    /// so 60 s is the documented sweet spot.
    #[serde(default = "default_lg_thinq_poll", with = "humantime_serde_compat")]
    pub poll_period: Duration,

    // --- Dashboard / HA `number` entity bounds. Tightening these
    // client-side keeps the dashboard slider sane and prevents
    // out-of-range values from being sent to LG (which would reject
    // them with UNACCEPTABLE_PARAMETERS). Defaults match a typical
    // Therma V install. ---
    #[serde(default = "default_lg_heating_min")]
    pub heating_target_min_c: u32,
    #[serde(default = "default_lg_heating_max")]
    pub heating_target_max_c: u32,
    #[serde(default = "default_lg_dhw_min")]
    pub dhw_target_min_c: u32,
    #[serde(default = "default_lg_dhw_max")]
    pub dhw_target_max_c: u32,
}

impl LgThinqConfig {
    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.pat.is_empty() && !self.device_id.is_empty() && !self.country.is_empty()
    }
}

fn default_lg_thinq_cache_dir() -> String {
    "/data/var/lib/victron-controller/lg-thinq/".to_string()
}

fn default_lg_thinq_poll() -> Duration {
    Duration::from_secs(60)
}

fn default_lg_heating_min() -> u32 {
    25
}
fn default_lg_heating_max() -> u32 {
    55
}
fn default_lg_dhw_min() -> u32 {
    30
}
fn default_lg_dhw_max() -> u32 {
    65
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
    /// PR-baseline-forecast: locally-computed pessimistic baseline. Used
    /// only when every cloud provider is stale — see fusion logic.
    #[serde(default)]
    pub baseline: BaselineProviderConfig,
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
            baseline: BaselineProviderConfig::default(),
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
    /// Representative planes. Empty ⇒ provider disabled.
    /// Latitude / longitude come from the top-level `[location]`
    /// section — single source of truth shared with every coord-driven
    /// scheduler (forecast.solar / open-meteo / baseline / sunrise-sunset).
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
    /// Representative planes. Empty ⇒ provider disabled.
    /// Latitude / longitude come from `[location]` — see
    /// `LocationConfig`.
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
            planes: Vec::new(),
            cadence: default_open_meteo_cadence(),
            system_efficiency: default_open_meteo_system_efficiency(),
        }
    }
}

fn default_open_meteo_system_efficiency() -> f64 {
    0.75
}

/// PR-baseline-forecast: install-time config for the local baseline
/// forecast. Runtime-tunable values (winter range, per-hour Wh) live on
/// `World::knobs` as four runtime knobs and are read by the scheduler
/// per cycle — see `core::knobs::Knobs::baseline_*` and the four
/// `KnobId::Baseline*` variants. This struct only carries the
/// install-time values that don't change at runtime.
#[derive(Debug, Clone, Deserialize)]
pub struct BaselineProviderConfig {
    /// When `false`, the scheduler doesn't start regardless of `[location]`.
    /// Default `false` so a fresh deployment doesn't emit dummy
    /// baseline values until the operator has explicitly opted in.
    /// Latitude / longitude come from `[location]` — see
    /// `LocationConfig`.
    #[serde(default)]
    pub enabled: bool,
    /// Recompute cadence. Sunrise/sunset and the season indicator move
    /// slowly — once an hour is plenty, but the cheap math means a
    /// faster cadence is fine too. Default 1 h.
    #[serde(
        default = "default_baseline_cadence",
        with = "humantime_serde_compat"
    )]
    pub cadence: Duration,
}

impl Default for BaselineProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cadence: default_baseline_cadence(),
        }
    }
}

impl BaselineProviderConfig {
    /// True iff the provider is enabled and has a non-zero cadence.
    /// Coordinate sanity is the sunrise crate's job.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.enabled && !self.cadence.is_zero()
    }
}

fn default_baseline_cadence() -> Duration {
    Duration::from_secs(60 * 60)
}

/// PR-keep-batteries-charged: site location for the always-on
/// sunrise/sunset scheduler. Independent of `[forecast.baseline]` (the
/// baseline forecast retains its own coordinates so a deployment with
/// only the baseline configured keeps working unchanged).
///
/// `is_configured` requires both fields to be non-default zeros — `(0,
/// 0)` is interpreted as "absent" rather than "Null Island". The
/// timezone for sunrise/sunset comes from `[forecast].timezone` (single
/// source of truth shared with the forecast schedulers), so an operator
/// that only wants this feature still configures `[forecast].timezone`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationConfig {
    #[serde(default)]
    pub latitude: f64,
    #[serde(default)]
    pub longitude: f64,
    /// Sunrise/sunset recompute cadence. Kept short by default so a
    /// fresh-boot has a value within ~15 min — the celestial values
    /// move slowly so a faster cadence has no real cost. Default 15 min.
    #[serde(
        default = "default_sunrise_sunset_cadence",
        with = "humantime_serde_compat"
    )]
    pub cadence: Duration,
}

impl LocationConfig {
    /// True iff the operator has supplied non-zero coordinates. `(0, 0)`
    /// is rejected as the absent state — Null Island is fine to ignore.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.latitude != 0.0 || self.longitude != 0.0
    }
}

fn default_sunrise_sunset_cadence() -> Duration {
    Duration::from_secs(15 * 60)
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

/// Optional MQTT bridge feeding `SensorId::OutdoorTemperature`. Same
/// broker/credentials as `[mqtt]`. Each entry in `sources` subscribes
/// to one topic; readings from all sources are merged "most recent
/// wins" via `Actual::on_reading`. Empty list ⇒ silent: the Open-Meteo
/// current-weather poller remains the sole source.
#[derive(Debug, Clone, Deserialize)]
pub struct OutdoorTemperatureLocalConfig {
    /// MQTT topics to subscribe to. Each entry declares its own body
    /// shape (`format`). See `OutdoorTempSource`.
    #[serde(default)]
    pub sources: Vec<OutdoorTempSource>,
    /// Sanity bounds applied to every source. Readings outside are
    /// dropped as glitches. Defaults: -50.0 / 80.0.
    #[serde(default = "default_min_celsius")]
    pub min_celsius: f64,
    #[serde(default = "default_max_celsius")]
    pub max_celsius: f64,
}

/// One source feeding the outdoor-temperature sensor. Internally tagged
/// on `format` so each variant carries the fields it needs.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum OutdoorTempSource {
    /// Matter `TemperatureMeasurement::MeasuredValue` shape (cluster
    /// 0x0402, attr 0): JSON-encoded signed int in centi-Celsius
    /// (e.g. `1640` = 16.4°C). `null` / non-numeric / out-of-int16
    /// bodies are silently dropped (the Meross hub publishes `null`
    /// between low-power reads).
    MatterCentiCelsius { topic: String },
    /// Zigbee2MQTT-style JSON object: pluck the named field as a
    /// floating-point °C value (e.g. `{"temperature":16.4,...}` with
    /// `field = "temperature"`). Non-numeric / missing fields are
    /// silently dropped.
    JsonField { topic: String, field: String },
}

impl OutdoorTempSource {
    #[must_use]
    pub fn topic(&self) -> &str {
        match self {
            Self::MatterCentiCelsius { topic } | Self::JsonField { topic, .. } => topic,
        }
    }
}

impl Default for OutdoorTemperatureLocalConfig {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
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

/// PR-ZD-1: optional zigbee2mqtt bridge for house appliance power
/// sensors. Body shape: JSON object with a `.power` field (W).
/// Availability topics are subscribed but informational only — the
/// freshness-window handles disconnects; no synthetic stale events.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Zigbee2MqttConfig {
    /// Value topic for the heat pump energy meter
    /// (e.g. `zigbee2mqtt/nodon-mtr-heat-pump`). None = bridge dormant.
    #[serde(default)]
    pub heat_pump_topic: Option<String>,
    /// Availability topic for the heat pump meter
    /// (e.g. `zigbee2mqtt/nodon-mtr-heat-pump/availability`).
    /// Subscribed but informational only. None = not subscribed.
    #[serde(default)]
    pub heat_pump_availability_topic: Option<String>,
    /// Value topic for the cooker/stove energy meter
    /// (e.g. `zigbee2mqtt/nodon-mtr-stove`). None = bridge dormant.
    #[serde(default)]
    pub cooker_topic: Option<String>,
    /// Availability topic for the cooker meter
    /// (e.g. `zigbee2mqtt/nodon-mtr-stove/availability`).
    /// Subscribed but informational only. None = not subscribed.
    #[serde(default)]
    pub cooker_availability_topic: Option<String>,
}

/// PR-pinned-registers: one (path, type, value) triplet the shell will
/// re-assert hourly. `path` joins the well-known service name and the
/// D-Bus object path with a single `:` separator, e.g.
/// `"com.victronenergy.vebus.ttyS3:/Devices/0/Settings/PowerAssistEnabled"`.
///
/// Validated in `config::load`: malformed paths and value/type
/// mismatches fail loud at startup rather than silently dropping a
/// pinned entry the operator believes is enforced.
#[derive(Debug, Clone, Deserialize)]
pub struct DbusPinnedRegister {
    pub path: String,
    /// Wire type: `"bool"` | `"int"` | `"float"` | `"string"`.
    #[serde(rename = "type")]
    pub value_type: PinnedType,
    pub value: toml::Value,
}

/// Wire-type of a pinned register, matching the TOML `type =` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinnedType {
    Bool,
    Int,
    Float,
    String,
}

impl PinnedType {
    /// `snake_case` for log / dashboard messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
        }
    }
}

/// Validated typed value for a pinned register. Constructed via
/// `PinnedValue::from_validated` once `config::load` has cross-checked
/// the type and the literal; the controller side then never has to
/// re-validate.
#[derive(Debug, Clone, PartialEq)]
pub enum PinnedValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl PinnedValue {
    /// Build a `PinnedValue` from a (validated) `(PinnedType, toml::Value)`
    /// pair. Rejects type mismatches, NaN/Inf floats, and (for the
    /// Int/Float coercion case) integers that don't fit `i64`.
    pub fn from_validated(value_type: PinnedType, value: &toml::Value) -> Result<Self> {
        match (value_type, value) {
            (PinnedType::Bool, toml::Value::Boolean(b)) => Ok(Self::Bool(*b)),
            (PinnedType::Int, toml::Value::Integer(n)) => Ok(Self::Int(*n)),
            (PinnedType::Float, toml::Value::Float(f)) => {
                if f.is_finite() {
                    Ok(Self::Float(*f))
                } else {
                    Err(anyhow::anyhow!(
                        "pinned-register float value is not finite: {f}"
                    ))
                }
            }
            // Operator convenience: a `value = 5000` literal in TOML
            // parses as Integer; accept that for a `type = "float"`
            // entry rather than forcing them to type `5000.0`.
            (PinnedType::Float, toml::Value::Integer(n)) => {
                #[allow(clippy::cast_precision_loss)]
                Ok(Self::Float(*n as f64))
            }
            (PinnedType::String, toml::Value::String(s)) => Ok(Self::String(s.clone())),
            (t, v) => Err(anyhow::anyhow!(
                "pinned-register value {v:?} is not compatible with type \"{}\"",
                t.name()
            )),
        }
    }
}

impl std::fmt::Display for PinnedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(x) => write!(f, "{x}"),
            // Quote strings so the dashboard distinguishes the literal
            // string `"true"` from the boolean `true`.
            Self::String(s) => write!(f, "\"{s}\""),
        }
    }
}

impl DbusPinnedRegister {
    /// Split the joined `service:path` into its two halves. Caller has
    /// already validated the format via `validate`, so the returned
    /// references are non-empty and the service is `com.victronenergy.*`.
    #[must_use]
    pub fn split_path(&self) -> (&str, &str) {
        // `validate` guarantees exactly one `:` separator with non-empty
        // sides. Defensive split: if a future code path constructs an
        // unvalidated entry, return empty halves so the consumer fails
        // visibly rather than panicking.
        match self.path.split_once(':') {
            Some((svc, path)) => (svc, path),
            None => ("", ""),
        }
    }

    /// Validate `path` shape + `(value_type, value)` compatibility.
    /// Called once at startup from `config::load`.
    fn validate(&self) -> Result<()> {
        let (svc, path) = self
            .path
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!(
                "[dbus_pinned_registers] path {:?} is missing the ':' separator \
                 between the service well-known name and the D-Bus path",
                self.path
            ))?;
        if svc.is_empty() {
            return Err(anyhow::anyhow!(
                "[dbus_pinned_registers] path {:?}: empty service name",
                self.path
            ));
        }
        if !svc.starts_with("com.victronenergy.") {
            return Err(anyhow::anyhow!(
                "[dbus_pinned_registers] path {:?}: service {:?} must start with \
                 \"com.victronenergy.\"",
                self.path,
                svc
            ));
        }
        if path.is_empty() {
            return Err(anyhow::anyhow!(
                "[dbus_pinned_registers] path {:?}: empty D-Bus path",
                self.path
            ));
        }
        if !path.starts_with('/') {
            return Err(anyhow::anyhow!(
                "[dbus_pinned_registers] path {:?}: D-Bus path {:?} must start with '/'",
                self.path,
                path
            ));
        }
        // Cross-check value/type at startup; reject NaN/Inf for float.
        let _ = PinnedValue::from_validated(self.value_type, &self.value)
            .map_err(|e| anyhow::anyhow!("[dbus_pinned_registers] {}: {e}", self.path))?;
        Ok(())
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

// -----------------------------------------------------------------------------
// Knob defaults (cold-start seed for World::knobs)
// -----------------------------------------------------------------------------

/// Cold-start overrides for `Knobs::safe_defaults`. Each field is
/// `Option<…>`; absent fields keep the SPEC §7 baseline. The struct is
/// applied once at boot in `main`, after `World::fresh_boot` and before
/// retained MQTT values are replayed — so retained values still win
/// per-knob if the user has touched them.
///
/// Mirror enums (`DischargeTimeCfg` etc.) exist so we can derive
/// `Deserialize` without polluting `core` with a serde dep, and so the
/// TOML wire-form spelling matches the existing MQTT/HA spelling
/// (`"02:00"`, `"forbid"`, `"weather"`, …).
///
/// PR-WSOC-EDIT-1: the 48 weather-SoC table cell knobs
/// (`KnobId::WeathersocTableCell`) are intentionally NOT seedable from
/// `config.toml`. Boot defaults flow from
/// `Knobs::safe_defaults().weather_soc_table`; runtime state arrives via
/// retained MQTT (`<root>/knob/weathersoc.table.<bucket>.<temp>.<field>/state`).
/// The 6 boundary knobs (`weathersoc.threshold.energy.*`,
/// `weathersoc.threshold.winter-temperature`) keep their existing
/// seedable fields below.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KnobsDefaultsConfig {
    // --- Export / discharge policy ---
    pub force_disable_export: Option<bool>,
    pub export_soc_threshold: Option<f64>,
    pub discharge_soc_target: Option<f64>,
    pub battery_soc_target: Option<f64>,
    pub full_charge_discharge_soc_target: Option<f64>,
    pub full_charge_export_soc_threshold: Option<f64>,
    pub discharge_time: Option<DischargeTimeCfg>,
    pub debug_full_charge: Option<DebugFullChargeCfg>,
    pub pessimism_multiplier_modifier: Option<f64>,
    pub disable_night_grid_discharge: Option<bool>,

    // --- Zappi ---
    pub charge_car_boost: Option<bool>,
    pub charge_car_extended_mode: Option<ExtendedChargeModeCfg>,
    pub zappi_current_target: Option<f64>,
    pub zappi_limit: Option<f64>,
    pub zappi_emergency_margin: Option<f64>,

    // --- Setpoint / grid caps ---
    pub grid_export_limit_w: Option<u32>,
    pub grid_import_limit_w: Option<u32>,
    // `allow_battery_to_car` is intentionally NOT exposed here — it
    // always boots `false` regardless of any external value (SPEC §2.10a).

    // --- Eddi ---
    pub eddi_enable_soc: Option<f64>,
    pub eddi_disable_soc: Option<f64>,
    pub eddi_dwell_s: Option<u32>,

    // --- Weather-SoC planner thresholds ---
    pub weathersoc_winter_temperature_threshold: Option<f64>,
    pub weathersoc_low_energy_threshold: Option<f64>,
    pub weathersoc_ok_energy_threshold: Option<f64>,
    pub weathersoc_high_energy_threshold: Option<f64>,
    pub weathersoc_too_much_energy_threshold: Option<f64>,
    /// PR-WSOC-TABLE-1: bucket-boundary kWh knob (default 67.5).
    pub weathersoc_very_sunny_threshold: Option<f64>,

    // --- Ops ---
    pub writes_enabled: Option<bool>,
    pub forecast_disagreement_strategy: Option<ForecastDisagreementStrategyCfg>,
    pub charge_battery_extended_mode: Option<ChargeBatteryExtendedModeCfg>,

    // --- PR-gamma-hold-redesign: per-knob source selectors ---
    pub export_soc_threshold_mode: Option<ModeCfg>,
    pub discharge_soc_target_mode: Option<ModeCfg>,
    pub battery_soc_target_mode: Option<ModeCfg>,
    pub disable_night_grid_discharge_mode: Option<ModeCfg>,

    // --- PR-safe-discharge-enable ---
    pub inverter_safe_discharge_enable: Option<bool>,

    // --- PR-baseline-forecast: 4 runtime knobs ---
    pub baseline_winter_start_mm_dd: Option<u32>,
    pub baseline_winter_end_mm_dd: Option<u32>,
    pub baseline_wh_per_hour_winter: Option<f64>,
    pub baseline_wh_per_hour_summer: Option<f64>,

    // --- PR2: cloud-cover modulation of the baseline forecast ---
    pub baseline_cloud_sunny_threshold_pct: Option<u32>,
    pub baseline_cloud_cloudy_threshold_pct: Option<u32>,
    pub baseline_cloud_factor_sunny: Option<f64>,
    pub baseline_cloud_factor_partial: Option<f64>,
    pub baseline_cloud_factor_cloudy: Option<f64>,

    // --- PR-keep-batteries-charged ---
    pub keep_batteries_charged_during_full_charge: Option<bool>,
    pub sunrise_sunset_offset_min: Option<u32>,

    pub full_charge_defer_to_next_sunday: Option<bool>,
    pub full_charge_snap_back_max_weekday: Option<u32>,

    // --- PR-ZD-2: compensated battery-drain feedback loop ---
    pub zappi_battery_drain_threshold_w: Option<u32>,
    pub zappi_battery_drain_relax_step_w: Option<u32>,
    pub zappi_battery_drain_kp: Option<f64>,
    pub zappi_battery_drain_target_w: Option<i32>,
    pub zappi_battery_drain_hard_clamp_w: Option<u32>,
    // --- PR-ZDP-1: MPPT curtailment probe ---
    pub zappi_battery_drain_mppt_probe_w: Option<u32>,

    // --- PR-ACT-RETRY-1: universal actuator retry threshold ---
    pub actuator_retry_s: Option<u32>,

    // --- PR-LG-THINQ-B: heat-pump knobs ---
    pub lg_heat_pump_power: Option<bool>,
    pub lg_dhw_power: Option<bool>,
    pub lg_heating_water_target_c: Option<u32>,
    pub lg_dhw_target_c: Option<u32>,
}

impl KnobsDefaultsConfig {
    /// Override the corresponding fields in `knobs`. Fields left `None`
    /// keep their `Knobs::safe_defaults` value.
    pub fn apply_to(self, knobs: &mut Knobs) {
        macro_rules! set {
            ($field:ident) => {
                if let Some(v) = self.$field {
                    knobs.$field = v;
                }
            };
            ($field:ident, into) => {
                if let Some(v) = self.$field {
                    knobs.$field = v.into();
                }
            };
        }
        set!(force_disable_export);
        set!(export_soc_threshold);
        set!(discharge_soc_target);
        set!(battery_soc_target);
        set!(full_charge_discharge_soc_target);
        set!(full_charge_export_soc_threshold);
        set!(discharge_time, into);
        set!(debug_full_charge, into);
        set!(pessimism_multiplier_modifier);
        set!(disable_night_grid_discharge);

        set!(charge_car_boost);
        set!(charge_car_extended_mode, into);
        set!(zappi_current_target);
        set!(zappi_limit);
        set!(zappi_emergency_margin);

        set!(grid_export_limit_w);
        set!(grid_import_limit_w);

        set!(eddi_enable_soc);
        set!(eddi_disable_soc);
        set!(eddi_dwell_s);

        set!(weathersoc_winter_temperature_threshold);
        set!(weathersoc_low_energy_threshold);
        set!(weathersoc_ok_energy_threshold);
        set!(weathersoc_high_energy_threshold);
        set!(weathersoc_too_much_energy_threshold);
        // PR-WSOC-TABLE-1.
        set!(weathersoc_very_sunny_threshold);

        set!(writes_enabled);
        set!(forecast_disagreement_strategy, into);
        set!(charge_battery_extended_mode, into);

        set!(export_soc_threshold_mode, into);
        set!(discharge_soc_target_mode, into);
        set!(battery_soc_target_mode, into);
        set!(disable_night_grid_discharge_mode, into);

        set!(inverter_safe_discharge_enable);

        set!(baseline_winter_start_mm_dd);
        set!(baseline_winter_end_mm_dd);
        set!(baseline_wh_per_hour_winter);
        set!(baseline_wh_per_hour_summer);

        set!(baseline_cloud_sunny_threshold_pct);
        set!(baseline_cloud_cloudy_threshold_pct);
        set!(baseline_cloud_factor_sunny);
        set!(baseline_cloud_factor_partial);
        set!(baseline_cloud_factor_cloudy);

        set!(keep_batteries_charged_during_full_charge);
        set!(sunrise_sunset_offset_min);

        set!(full_charge_defer_to_next_sunday);
        set!(full_charge_snap_back_max_weekday);

        // PR-ZD-2: compensated battery-drain feedback loop.
        set!(zappi_battery_drain_threshold_w);
        set!(zappi_battery_drain_relax_step_w);
        set!(zappi_battery_drain_kp);
        set!(zappi_battery_drain_target_w);
        set!(zappi_battery_drain_hard_clamp_w);
        // PR-ZDP-1: MPPT curtailment probe.
        set!(zappi_battery_drain_mppt_probe_w);
        // PR-ACT-RETRY-1: universal actuator retry threshold.
        set!(actuator_retry_s);

        // PR-LG-THINQ-B: heat-pump knobs.
        set!(lg_heat_pump_power);
        set!(lg_dhw_power);
        set!(lg_heating_water_target_c);
        set!(lg_dhw_target_c);
    }
}

/// Mirror of `core::knobs::DischargeTime`. Wire form matches MQTT:
/// `"02:00"` / `"23:00"` (or `"02:00:00"` / `"23:00:00"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DischargeTimeCfg {
    At0200,
    At2300,
}

impl<'de> Deserialize<'de> for DischargeTimeCfg {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.trim() {
            "02:00" | "02:00:00" => Ok(Self::At0200),
            "23:00" | "23:00:00" => Ok(Self::At2300),
            other => Err(serde::de::Error::custom(format!(
                "invalid discharge_time {other:?}; expected \"02:00\" or \"23:00\""
            ))),
        }
    }
}

impl From<DischargeTimeCfg> for CoreDischargeTime {
    fn from(c: DischargeTimeCfg) -> Self {
        match c {
            DischargeTimeCfg::At0200 => Self::At0200,
            DischargeTimeCfg::At2300 => Self::At2300,
        }
    }
}

/// Mirror of `core::knobs::DebugFullCharge`. Wire form matches MQTT:
/// `"forbid"` / `"force"` / `"auto"`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DebugFullChargeCfg {
    Forbid,
    Force,
    Auto,
}

impl From<DebugFullChargeCfg> for CoreDebugFullCharge {
    fn from(c: DebugFullChargeCfg) -> Self {
        match c {
            DebugFullChargeCfg::Forbid => Self::Forbid,
            DebugFullChargeCfg::Force => Self::Force,
            DebugFullChargeCfg::Auto => Self::Auto,
        }
    }
}

/// Mirror of `core::knobs::ForecastDisagreementStrategy`. Wire form
/// matches MQTT: `"max"` / `"min"` / `"mean"` /
/// `"solcast_if_available_else_mean"`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForecastDisagreementStrategyCfg {
    Max,
    Min,
    Mean,
    SolcastIfAvailableElseMean,
}

impl From<ForecastDisagreementStrategyCfg> for CoreForecastDisagreementStrategy {
    fn from(c: ForecastDisagreementStrategyCfg) -> Self {
        match c {
            ForecastDisagreementStrategyCfg::Max => Self::Max,
            ForecastDisagreementStrategyCfg::Min => Self::Min,
            ForecastDisagreementStrategyCfg::Mean => Self::Mean,
            ForecastDisagreementStrategyCfg::SolcastIfAvailableElseMean => {
                Self::SolcastIfAvailableElseMean
            }
        }
    }
}

/// Mirror of `core::knobs::ChargeBatteryExtendedMode`. Wire form
/// matches MQTT: `"auto"` / `"forced"` / `"disabled"`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChargeBatteryExtendedModeCfg {
    Auto,
    Forced,
    Disabled,
}

impl From<ChargeBatteryExtendedModeCfg> for CoreChargeBatteryExtendedMode {
    fn from(c: ChargeBatteryExtendedModeCfg) -> Self {
        match c {
            ChargeBatteryExtendedModeCfg::Auto => Self::Auto,
            ChargeBatteryExtendedModeCfg::Forced => Self::Forced,
            ChargeBatteryExtendedModeCfg::Disabled => Self::Disabled,
        }
    }
}

/// Mirror of `core::knobs::ExtendedChargeMode`. Wire form matches MQTT:
/// `"auto"` / `"forced"` / `"disabled"`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtendedChargeModeCfg {
    Auto,
    Forced,
    Disabled,
}

impl From<ExtendedChargeModeCfg> for CoreExtendedChargeMode {
    fn from(c: ExtendedChargeModeCfg) -> Self {
        match c {
            ExtendedChargeModeCfg::Auto => Self::Auto,
            ExtendedChargeModeCfg::Forced => Self::Forced,
            ExtendedChargeModeCfg::Disabled => Self::Disabled,
        }
    }
}

/// Mirror of `core::knobs::Mode` (PR-gamma-hold-redesign per-knob
/// selector). Wire form matches MQTT: `"weather"` / `"forced"`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModeCfg {
    Weather,
    Forced,
}

impl From<ModeCfg> for CoreMode {
    fn from(c: ModeCfg) -> Self {
        match c {
            ModeCfg::Weather => Self::Weather,
            ModeCfg::Forced => Self::Forced,
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
    // PR-pinned-registers: validate every pinned-register entry up
    // front. Catches malformed paths and type/value mismatches before
    // the shell ever tries to write to the bus.
    for entry in &cfg.dbus_pinned_registers {
        entry.validate()?;
    }
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
            zigbee2mqtt: Zigbee2MqttConfig::default(),
            dbus_pinned_registers: Vec::new(),
            knobs: KnobsDefaultsConfig::default(),
            location: LocationConfig::default(),
            lg_thinq: LgThinqConfig::default(),
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
    fn example_config_parses_cleanly() {
        // Guards against typos / unknown-field regressions in the
        // shipped example after edits to the [knobs] section etc.
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config.example.toml");
        let cfg = load(&path).unwrap_or_else(|e| panic!("example config failed to load: {e:#}"));
        // Smoke-check a couple of values so the parser actually walked it.
        assert_eq!(cfg.dashboard.port, 8910);
        assert_eq!(cfg.forecast.timezone, "Europe/London");
    }

    #[test]
    fn knobs_section_absent_keeps_safe_defaults() {
        let c: Config = toml::from_str("").unwrap();
        let mut k = Knobs::safe_defaults();
        let before = k;
        c.knobs.apply_to(&mut k);
        assert_eq!(k, before, "absent [knobs] must not perturb safe_defaults");
    }

    #[test]
    fn knobs_section_overrides_floats_and_enums() {
        let t = r#"
            [knobs]
            export_soc_threshold     = 75.0
            grid_export_limit_w      = 4000
            discharge_time           = "23:00"
            debug_full_charge        = "force"
            forecast_disagreement_strategy = "max"
            charge_battery_extended_mode   = "forced"
            charge_car_extended_mode       = "disabled"
            export_soc_threshold_mode      = "forced"
            inverter_safe_discharge_enable = true
        "#;
        let c: Config = toml::from_str(t).unwrap();
        let mut k = Knobs::safe_defaults();
        c.knobs.apply_to(&mut k);
        assert!((k.export_soc_threshold - 75.0).abs() < f64::EPSILON);
        assert_eq!(k.grid_export_limit_w, 4000);
        assert_eq!(k.discharge_time, CoreDischargeTime::At2300);
        assert_eq!(k.debug_full_charge, CoreDebugFullCharge::Force);
        assert_eq!(
            k.forecast_disagreement_strategy,
            CoreForecastDisagreementStrategy::Max
        );
        assert_eq!(
            k.charge_battery_extended_mode,
            CoreChargeBatteryExtendedMode::Forced
        );
        assert_eq!(
            k.charge_car_extended_mode,
            CoreExtendedChargeMode::Disabled
        );
        assert_eq!(k.export_soc_threshold_mode, CoreMode::Forced);
        assert!(k.inverter_safe_discharge_enable);
        // Untouched fields keep safe_defaults.
        let d = Knobs::safe_defaults();
        assert!((k.discharge_soc_target - d.discharge_soc_target).abs() < f64::EPSILON);
        assert_eq!(k.grid_import_limit_w, d.grid_import_limit_w);
    }

    #[test]
    fn knobs_unknown_field_rejected() {
        let t = r"
            [knobs]
            allow_battery_to_car = true
        ";
        let err = toml::from_str::<Config>(t).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("allow_battery_to_car") || msg.contains("unknown field"),
            "expected unknown-field rejection, got: {msg}"
        );
    }

    #[test]
    fn knobs_invalid_discharge_time_rejected() {
        let t = r#"[knobs]
discharge_time = "noon"
"#;
        let err = toml::from_str::<Config>(t).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("noon") && msg.contains("discharge_time"),
            "expected discharge_time enum failure, got: {msg}"
        );
    }

    #[test]
    fn outdoor_temperature_local_absent_yields_silent_defaults() {
        // Open-Meteo is the sole outdoor_temperature source when the
        // section is absent.
        let c: Config = toml::from_str("").unwrap();
        assert!(c.outdoor_temperature_local.sources.is_empty());
        assert!((c.outdoor_temperature_local.min_celsius - -50.0).abs() < f64::EPSILON);
        assert!((c.outdoor_temperature_local.max_celsius - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn outdoor_temperature_local_parses_multi_source_section() {
        let t = r#"
            [outdoor_temperature_local]
            min_celsius = -20.0
            max_celsius = 50.0

            [[outdoor_temperature_local.sources]]
            topic  = "matter/1/attributes/107_1026_0"
            format = "matter_centi_celsius"

            [[outdoor_temperature_local.sources]]
            topic  = "zigbee2mqtt/nodon-snsr-outdoor"
            format = "json_field"
            field  = "temperature"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(c.outdoor_temperature_local.sources.len(), 2);
        match &c.outdoor_temperature_local.sources[0] {
            OutdoorTempSource::MatterCentiCelsius { topic } => {
                assert_eq!(topic, "matter/1/attributes/107_1026_0");
            }
            other @ OutdoorTempSource::JsonField { .. } => {
                panic!("expected MatterCentiCelsius, got {other:?}")
            }
        }
        match &c.outdoor_temperature_local.sources[1] {
            OutdoorTempSource::JsonField { topic, field } => {
                assert_eq!(topic, "zigbee2mqtt/nodon-snsr-outdoor");
                assert_eq!(field, "temperature");
            }
            other @ OutdoorTempSource::MatterCentiCelsius { .. } => {
                panic!("expected JsonField, got {other:?}")
            }
        }
        assert!((c.outdoor_temperature_local.min_celsius - -20.0).abs() < f64::EPSILON);
        assert!((c.outdoor_temperature_local.max_celsius - 50.0).abs() < f64::EPSILON);
    }

    // PR-pinned-registers ----------------------------------------------------

    #[test]
    fn dbus_pinned_registers_parses_full_set() {
        let t = r#"
            [[dbus_pinned_registers]]
            path  = "com.victronenergy.vebus.ttyS3:/Devices/0/Settings/PowerAssistEnabled"
            type  = "bool"
            value = true

            [[dbus_pinned_registers]]
            path  = "com.victronenergy.settings:/Settings/CGwacs/MaxFeedInPower"
            type  = "float"
            value = 5000.0

            [[dbus_pinned_registers]]
            path  = "com.victronenergy.settings:/Settings/CGwacs/Hub4Mode"
            type  = "int"
            value = 1

            [[dbus_pinned_registers]]
            path  = "com.victronenergy.settings:/Settings/Foo/Bar"
            type  = "string"
            value = "hello"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(c.dbus_pinned_registers.len(), 4);
        for entry in &c.dbus_pinned_registers {
            entry.validate().unwrap_or_else(|e| panic!("{e}"));
        }
        // Spot-check a split + value coercion.
        let r0 = &c.dbus_pinned_registers[0];
        let (svc, p) = r0.split_path();
        assert_eq!(svc, "com.victronenergy.vebus.ttyS3");
        assert_eq!(p, "/Devices/0/Settings/PowerAssistEnabled");
        assert_eq!(
            PinnedValue::from_validated(r0.value_type, &r0.value).unwrap(),
            PinnedValue::Bool(true),
        );
    }

    #[test]
    fn dbus_pinned_registers_accepts_integer_for_float_type() {
        // Operator convenience: `value = 5000` (Integer) for a
        // `type = "float"` entry coerces to f64 5000.0.
        let r = DbusPinnedRegister {
            path: "com.victronenergy.settings:/Settings/X".to_string(),
            value_type: PinnedType::Float,
            value: toml::Value::Integer(5000),
        };
        r.validate().unwrap();
        assert_eq!(
            PinnedValue::from_validated(r.value_type, &r.value).unwrap(),
            PinnedValue::Float(5000.0),
        );
    }

    #[test]
    fn dbus_pinned_registers_rejects_type_mismatch() {
        let r = DbusPinnedRegister {
            path: "com.victronenergy.settings:/Settings/X".to_string(),
            value_type: PinnedType::Bool,
            value: toml::Value::Integer(1),
        };
        let err = r.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not compatible") && msg.contains("\"bool\""),
            "expected type-mismatch error mentioning bool, got: {msg}"
        );
    }

    #[test]
    fn dbus_pinned_registers_rejects_malformed_path_no_colon() {
        let r = DbusPinnedRegister {
            path: "com.victronenergy.settings".to_string(), // no colon
            value_type: PinnedType::Int,
            value: toml::Value::Integer(0),
        };
        let err = r.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains(':'), "error must mention the missing separator: {msg}");
    }

    #[test]
    fn dbus_pinned_registers_rejects_non_victron_service() {
        let r = DbusPinnedRegister {
            path: "org.freedesktop.DBus:/Settings/X".to_string(),
            value_type: PinnedType::Int,
            value: toml::Value::Integer(0),
        };
        let err = r.validate().unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("com.victronenergy."), "{msg}");
    }

    #[test]
    fn dbus_pinned_registers_rejects_empty_path_or_service() {
        let r1 = DbusPinnedRegister {
            path: ":/Settings/X".to_string(),
            value_type: PinnedType::Int,
            value: toml::Value::Integer(0),
        };
        assert!(r1.validate().is_err());
        let r2 = DbusPinnedRegister {
            path: "com.victronenergy.settings:".to_string(),
            value_type: PinnedType::Int,
            value: toml::Value::Integer(0),
        };
        assert!(r2.validate().is_err());
        let r3 = DbusPinnedRegister {
            path: "com.victronenergy.settings:Settings/X".to_string(), // no leading /
            value_type: PinnedType::Int,
            value: toml::Value::Integer(0),
        };
        assert!(r3.validate().is_err());
    }

    #[test]
    fn dbus_pinned_registers_rejects_nan_inf_float() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let r = DbusPinnedRegister {
                path: "com.victronenergy.settings:/Settings/X".to_string(),
                value_type: PinnedType::Float,
                value: toml::Value::Float(bad),
            };
            let err = r.validate().unwrap_err();
            let msg = format!("{err:#}");
            assert!(
                msg.contains("not finite"),
                "expected 'not finite' error for {bad}, got: {msg}"
            );
        }
    }

    #[test]
    fn pinned_value_display_uses_lossy_form() {
        assert_eq!(format!("{}", PinnedValue::Bool(true)), "true");
        assert_eq!(format!("{}", PinnedValue::Int(5000)), "5000");
        assert_eq!(format!("{}", PinnedValue::Float(5000.0)), "5000");
        assert_eq!(format!("{}", PinnedValue::Float(5000.5)), "5000.5");
        assert_eq!(
            format!("{}", PinnedValue::String("foo".to_string())),
            "\"foo\""
        );
    }

    // PR-baseline-forecast --------------------------------------------------

    #[test]
    fn baseline_disabled_by_default() {
        let c: Config = toml::from_str("").unwrap();
        assert!(!c.forecast.baseline.is_configured());
        assert!(!c.forecast.baseline.enabled);
        assert_eq!(c.forecast.baseline.cadence, Duration::from_secs(3600));
    }

    #[test]
    fn baseline_parses_install_time_section() {
        let t = r#"
            [location]
            latitude = 51.5
            longitude = -0.1

            [forecast.baseline]
            enabled = true
            cadence = "30m"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert!(c.forecast.baseline.is_configured());
        assert!(c.location.is_configured());
        assert_eq!(c.location.latitude, 51.5);
        assert_eq!(c.forecast.baseline.cadence, Duration::from_secs(30 * 60));
    }

    #[test]
    fn baseline_is_configured_requires_enabled_and_cadence() {
        let mut cfg = BaselineProviderConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(!cfg.is_configured(), "disabled");
        cfg.enabled = true;
        assert!(cfg.is_configured());
        cfg.cadence = Duration::ZERO;
        assert!(!cfg.is_configured(), "cadence=0 must disable");
    }

    #[test]
    fn dbus_services_default_matches_venus_3_70_discovery() {
        let s = DbusServices::default_venus_3_70();
        assert!(s.battery.contains("socketcan_can0"));
        assert!(s.evcharger.contains("evcharger.cgwacs_ttyUSB0_mb2"));
        assert!(s.vebus.contains("vebus.ttyS3"));
    }
}
