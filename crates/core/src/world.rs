//! The single top-level state container for the pure core. See SPEC §2.3.6.

use chrono::{NaiveDate, NaiveDateTime};
use std::time::Instant;

use crate::controllers::schedules::ScheduleSpec;
use crate::knobs::Knobs;
use crate::myenergi::{EddiMode, ZappiMode, ZappiState};
use crate::tass::{Actual, Actuated};
use crate::types::{Decision, ForecastProvider};

/// All scalar sensor readings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sensors {
    pub battery_soc: Actual<f64>,
    pub battery_soh: Actual<f64>,
    pub battery_installed_capacity: Actual<f64>,
    pub battery_dc_power: Actual<f64>,
    pub mppt_power_0: Actual<f64>,
    pub mppt_power_1: Actual<f64>,
    pub soltaro_power: Actual<f64>,
    pub power_consumption: Actual<f64>,
    pub grid_power: Actual<f64>,
    pub grid_voltage: Actual<f64>,
    pub grid_current: Actual<f64>,
    pub consumption_current: Actual<f64>,
    pub offgrid_power: Actual<f64>,
    pub offgrid_current: Actual<f64>,
    pub vebus_input_current: Actual<f64>,
    pub evcharger_ac_power: Actual<f64>,
    pub evcharger_ac_current: Actual<f64>,
    pub ess_state: Actual<f64>,
    pub outdoor_temperature: Actual<f64>,
}

impl Sensors {
    #[must_use]
    pub fn unknown(now: Instant) -> Self {
        Self {
            battery_soc: Actual::unknown(now),
            battery_soh: Actual::unknown(now),
            battery_installed_capacity: Actual::unknown(now),
            battery_dc_power: Actual::unknown(now),
            mppt_power_0: Actual::unknown(now),
            mppt_power_1: Actual::unknown(now),
            soltaro_power: Actual::unknown(now),
            power_consumption: Actual::unknown(now),
            grid_power: Actual::unknown(now),
            grid_voltage: Actual::unknown(now),
            grid_current: Actual::unknown(now),
            consumption_current: Actual::unknown(now),
            offgrid_power: Actual::unknown(now),
            offgrid_current: Actual::unknown(now),
            vebus_input_current: Actual::unknown(now),
            evcharger_ac_power: Actual::unknown(now),
            evcharger_ac_current: Actual::unknown(now),
            ess_state: Actual::unknown(now),
            outdoor_temperature: Actual::unknown(now),
        }
    }
}

/// Per-provider forecast snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForecastSnapshot {
    pub today_kwh: f64,
    pub tomorrow_kwh: f64,
    pub fetched_at: Instant,
}

/// Non-scalar sensor state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedSensors {
    pub zappi_state: Actual<ZappiState>,
    pub eddi_mode: Actual<EddiMode>,
    pub forecast_solcast: Option<ForecastSnapshot>,
    pub forecast_forecast_solar: Option<ForecastSnapshot>,
    pub forecast_open_meteo: Option<ForecastSnapshot>,
}

impl TypedSensors {
    #[must_use]
    pub fn unknown(now: Instant) -> Self {
        Self {
            zappi_state: Actual::unknown(now),
            eddi_mode: Actual::unknown(now),
            forecast_solcast: None,
            forecast_forecast_solar: None,
            forecast_open_meteo: None,
        }
    }

    #[must_use]
    pub fn forecast(&self, p: ForecastProvider) -> Option<&ForecastSnapshot> {
        match p {
            ForecastProvider::Solcast => self.forecast_solcast.as_ref(),
            ForecastProvider::ForecastSolar => self.forecast_forecast_solar.as_ref(),
            ForecastProvider::OpenMeteo => self.forecast_open_meteo.as_ref(),
        }
    }
}

/// Cross-cutting bookkeeping. Persisted to retained MQTT when changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bookkeeping {
    pub next_full_charge: Option<NaiveDateTime>,
    pub above_soc_date: Option<NaiveDate>,
    pub prev_ess_state: Option<i32>,
    pub charge_to_full_required: bool,
    pub soc_end_of_day_target: f64,
    /// Effective export SoC threshold (charge-to-full overrides when active).
    pub effective_export_soc_threshold: f64,
    pub battery_selected_soc_target: f64,
    /// Last Eddi mode transition time (used by Eddi controller's dwell check).
    pub eddi_last_transition_at: Option<Instant>,
    /// True if today's weather_soc decided the night charge should
    /// extend through NightExtended (05:00-08:00). Set by `run_weather_soc`
    /// at 01:55; reset to false on every calendar-day rollover (by
    /// `apply_tick`). Read by `run_schedules` as the ONLY signal driving
    /// `charge_battery_extended` (combined with a manual override knob).
    pub charge_battery_extended_today: bool,
    /// Calendar date `charge_battery_extended_today` was last set for, so
    /// the tick-level reset knows when to clear.
    pub charge_battery_extended_today_date: Option<NaiveDate>,
    /// A-21: last calendar date `run_weather_soc` fired its knob proposals.
    /// Prevents the 60-tick flood in the 01:55:00–01:55:59 window.
    /// Stamped only on successful knob application (γ-hold permitting);
    /// not persisted to retained MQTT today, so a reboot inside the 01:55
    /// minute may re-fire — accepted tradeoff.
    pub last_weather_soc_run_date: Option<NaiveDate>,
}

impl Bookkeeping {
    #[must_use]
    pub const fn fresh_boot() -> Self {
        Self {
            next_full_charge: None,
            above_soc_date: None,
            prev_ess_state: None,
            charge_to_full_required: false,
            soc_end_of_day_target: 80.0,
            effective_export_soc_threshold: 80.0,
            battery_selected_soc_target: 80.0,
            eddi_last_transition_at: None,
            charge_battery_extended_today: false,
            charge_battery_extended_today_date: None,
            last_weather_soc_run_date: None,
        }
    }
}

/// Pure per-tick derivations. Owned by derivation cores (see
/// `core_dag::cores`), not by `Bookkeeping`. Recomputed every tick from
/// sensors; never retained on MQTT. Consumed by actuator cores that
/// declare a `depends_on` edge on the producing derivation core.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DerivedState {
    /// Written by `ZappiActiveCore` at the top of every tick; read by
    /// `SetpointCore`, `CurrentLimitCore`, and `SchedulesCore`.
    pub zappi_active: bool,
}

/// Latest "why?" explanation per controller. Overwritten on every
/// evaluation, so the snapshot always matches the live target state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Decisions {
    pub grid_setpoint: Option<Decision>,
    pub input_current_limit: Option<Decision>,
    pub schedule_0: Option<Decision>,
    pub schedule_1: Option<Decision>,
    pub zappi_mode: Option<Decision>,
    pub eddi_mode: Option<Decision>,
    pub weather_soc: Option<Decision>,
}

/// Provenance record: which owner last wrote a knob, and when.
///
/// A-55: per-knob granularity. Previously a single `Option<Instant>`
/// covered *every* knob, so writing `battery_soc_target` from the
/// dashboard suppressed HaMqtt/WeatherSocPlanner on *all* knobs for
/// the hold window — if the user touched one knob, HA stopped being
/// able to drive the others. The per-knob map only holds the ones
/// that have ever been written from the dashboard.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KnobProvenance {
    pub last_dashboard_write: std::collections::HashMap<crate::types::KnobId, Instant>,
}

impl KnobProvenance {
    #[must_use]
    pub fn fresh_boot() -> Self {
        Self {
            last_dashboard_write: std::collections::HashMap::new(),
        }
    }
}

/// Top-level world state.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    // Actuated
    pub grid_setpoint: Actuated<i32>,
    pub input_current_limit: Actuated<f64>,
    pub zappi_mode: Actuated<ZappiMode>,
    pub eddi_mode: Actuated<EddiMode>,
    pub schedule_0: Actuated<ScheduleSpec>,
    pub schedule_1: Actuated<ScheduleSpec>,

    // Knobs (plain values; γ γ hold tracked via knob_provenance)
    pub knobs: Knobs,
    pub knob_provenance: KnobProvenance,

    // Sensors
    pub sensors: Sensors,
    pub typed_sensors: TypedSensors,

    // Derived / cross-controller
    pub bookkeeping: Bookkeeping,

    /// Per-tick derivations, written by derivation cores at the top of
    /// each tick. See [`DerivedState`].
    pub derived: DerivedState,

    /// Latest human-readable explanation for each controller. See
    /// SPEC §5.12 and `types::Decision`.
    pub decisions: Decisions,
}

impl World {
    /// Fresh-boot world: all sensors `Unknown`, all actuated entities
    /// `Unset`, knobs at [`Knobs::safe_defaults`], bookkeeping empty.
    #[must_use]
    pub fn fresh_boot(now: Instant) -> Self {
        Self {
            grid_setpoint: Actuated::new(now),
            input_current_limit: Actuated::new(now),
            zappi_mode: Actuated::new(now),
            eddi_mode: Actuated::new(now),
            schedule_0: Actuated::new(now),
            schedule_1: Actuated::new(now),
            knobs: Knobs::safe_defaults(),
            knob_provenance: KnobProvenance::fresh_boot(),
            sensors: Sensors::unknown(now),
            typed_sensors: TypedSensors::unknown(now),
            bookkeeping: Bookkeeping::fresh_boot(),
            derived: DerivedState::default(),
            decisions: Decisions::default(),
        }
    }
}
