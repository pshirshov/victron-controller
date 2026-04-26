// PR-rename-entities: translation table from snake_case canonical keys
// (the names that appear as struct field names in the snapshot JSON, e.g.
// `snap.knobs.export_soc_threshold`) to the user-facing dotted hierarchical
// names shown on the dashboard and used as MQTT topic-tails.
//
// The Rust struct field names stay snake_case (valid Rust identifiers; the
// baboon model is untouched). Only the user-visible surface — dashboard
// cell text, popup titles, MQTT topic paths, HA discovery unique_ids — uses
// the dotted form.
//
// Renderer convention (PR-rename-entities-D02):
//   - `data-entity-id` on every entity link stays canonical (snake_case).
//     That's what the inspector dispatch uses to read snapshot fields.
//   - The link's visible text uses `displayNameOf(canonical)`.
//   - The description registry, KNOB_SPEC and forecastProviderLabels are
//     keyed on the dotted form; lookups translate canonical → dotted.

const DISPLAY_NAMES: Record<string, string> = {
  // --- Sensors (20) ---
  battery_soc: "battery.soc",
  battery_soh: "battery.soh",
  battery_installed_capacity: "battery.capacity.installed",
  battery_dc_power: "battery.power.dc",
  mppt_power_0: "solar.mppt.0.power",
  mppt_power_1: "solar.mppt.1.power",
  soltaro_power: "solar.soltaro.power",
  power_consumption: "house.power.consumption",
  consumption_current: "house.current.consumption",
  grid_power: "grid.power",
  grid_voltage: "grid.voltage",
  grid_current: "grid.current",
  offgrid_power: "inverter.offgrid.power",
  offgrid_current: "inverter.offgrid.current",
  vebus_input_current: "inverter.input.current",
  evcharger_ac_power: "evcharger.ac.power",
  evcharger_ac_current: "evcharger.ac.current",
  ess_state: "inverter.ess.state",
  outdoor_temperature: "weather.temperature.outdoor",
  session_kwh: "evcharger.session.energy",
  ev_soc: "ev.soc",
  ev_charge_target: "ev.charge.target",

  // --- Knobs (33) ---
  force_disable_export: "grid.export.force-disable",
  export_soc_threshold: "battery.soc.threshold.export.forced-value",
  export_soc_threshold_mode: "battery.soc.threshold.export.mode",
  discharge_soc_target: "battery.soc.target.discharge.forced-value",
  discharge_soc_target_mode: "battery.soc.target.discharge.mode",
  battery_soc_target: "battery.soc.target.charge.forced-value",
  battery_soc_target_mode: "battery.soc.target.charge.mode",
  disable_night_grid_discharge: "grid.night.discharge.disable.forced-value",
  disable_night_grid_discharge_mode: "grid.night.discharge.disable.mode",
  inverter_safe_discharge_enable: "inverter.safe-discharge.enable",
  full_charge_discharge_soc_target: "battery.soc.target.full-charge.discharge",
  full_charge_export_soc_threshold: "battery.soc.threshold.full-charge.export",
  discharge_time: "battery.discharge.time",
  debug_full_charge: "debug.full-charge.mode",
  pessimism_multiplier_modifier: "forecast.pessimism.modifier",
  charge_car_boost: "evcharger.boost.enable",
  charge_car_extended_mode: "evcharger.extended.mode",
  zappi_current_target: "evcharger.current.target",
  zappi_limit: "evcharger.session.limit",
  zappi_emergency_margin: "evcharger.current.margin",
  grid_export_limit_w: "grid.export.limit",
  grid_import_limit_w: "grid.import.limit",
  allow_battery_to_car: "battery.export.car.allow",
  eddi_enable_soc: "eddi.soc.enable",
  eddi_disable_soc: "eddi.soc.disable",
  eddi_dwell_s: "eddi.dwell.seconds",
  weathersoc_winter_temperature_threshold: "weathersoc.threshold.winter-temperature",
  weathersoc_low_energy_threshold: "weathersoc.threshold.energy.low",
  weathersoc_ok_energy_threshold: "weathersoc.threshold.energy.ok",
  weathersoc_high_energy_threshold: "weathersoc.threshold.energy.high",
  weathersoc_too_much_energy_threshold: "weathersoc.threshold.energy.too-much",
  forecast_disagreement_strategy: "forecast.disagreement.strategy",
  charge_battery_extended_mode: "schedule.extended.charge.mode",
  writes_enabled: "writes-enabled",

  // --- Bookkeeping (14) ---
  next_full_charge_iso: "schedule.full-charge.next",
  above_soc_date_iso: "battery.soc.above-threshold.date",
  prev_ess_state: "inverter.ess.state.previous",
  zappi_active: "evcharger.active",
  charge_to_full_required: "schedule.full-charge.required",
  soc_end_of_day_target: "battery.soc.target.end-of-day",
  effective_export_soc_threshold: "battery.soc.threshold.export.effective",
  battery_selected_soc_target: "battery.soc.target.selected",
  charge_battery_extended_today: "schedule.extended.charge.today",
  charge_battery_extended_today_date_iso: "schedule.extended.charge.today.date",
  weather_soc_export_soc_threshold: "weathersoc.derived.threshold.export",
  weather_soc_discharge_soc_target: "weathersoc.derived.target.discharge",
  weather_soc_battery_soc_target: "weathersoc.derived.target.charge",
  weather_soc_disable_night_grid_discharge: "weathersoc.derived.grid.night.discharge.disable",
  // PR-auto-extended-charge.
  auto_extended_today: "evcharger.extended.auto.today",
  auto_extended_today_date_iso: "evcharger.extended.auto.today.date",

  // --- Actuated (6) — `.target` distinguishes from same-named decisions. ---
  grid_setpoint: "grid.setpoint",
  input_current_limit: "inverter.input.current-limit",
  zappi_mode: "evcharger.mode.target",
  eddi_mode: "eddi.mode.target",
  schedule_0: "schedule.0",
  schedule_1: "schedule.1",

  // --- Decisions (7) — keys collide with actuated/cores (zappi_mode,
  // eddi_mode, weather_soc, grid_setpoint, input_current_limit, schedule_*).
  // The user-visible dotted form disambiguates: actuated → `.target`,
  // decision → `.decision`, core → bare. The displayNames map can only
  // hold ONE value per snake_case key — these collisions resolve via the
  // entity-type the renderer passes; see `displayNameOfTyped` below.
  // The default (`displayNameOf`) returns the actuated form — that's what
  // appears in the actuated table; decision/core rows pass type explicitly.

  // --- Forecasts (3) ---
  solcast: "forecast.solcast",
  forecast_solar: "forecast.solar",
  open_meteo: "forecast.open-meteo",
};

// Per-type overrides for collision keys. When the same canonical name
// appears in multiple entity classes, the dotted display name differs
// (e.g. `zappi_mode` actuated → `evcharger.mode.target` vs decision
// → `evcharger.mode.decision` vs core → `evcharger.mode`).
type TypeOverride = Partial<Record<string, string>>;

const DISPLAY_NAMES_BY_TYPE: Record<string, TypeOverride> = {
  decision: {
    grid_setpoint: "setpoint.decision",
    input_current_limit: "current-limit.decision",
    schedule_0: "schedule.0.decision",
    schedule_1: "schedule.1.decision",
    zappi_mode: "evcharger.mode.decision",
    eddi_mode: "eddi.mode.decision",
    weather_soc: "weathersoc",
  },
  core: {
    zappi_active: "evcharger.active",
    setpoint: "setpoint",
    current_limit: "current-limit",
    schedules: "schedules",
    zappi_mode: "evcharger.mode",
    eddi_mode: "eddi.mode",
    weather_soc: "weathersoc",
    sensor_broadcast: "broadcast.sensor",
  },
  // Timer ids in the snapshot already arrive in dotted form (TimerId::name()
  // returns dotted post-PR-rename-entities), so the displayNames table is a
  // pass-through for timers — no override needed.
};

/// Translate a snake_case canonical key into the user-visible dotted
/// display name, defaulting to the canonical key itself when the entry
/// isn't registered (defensive — drift from backend renames degrades
/// visibly rather than crashing).
export function displayNameOf(canonical: string): string {
  return DISPLAY_NAMES[canonical] ?? canonical;
}

/// Type-aware lookup for canonical keys that appear in multiple classes
/// (decision/actuated/core collisions). Falls back to `displayNameOf`.
export function displayNameOfTyped(canonical: string, type: string): string {
  const byType = DISPLAY_NAMES_BY_TYPE[type];
  if (byType && canonical in byType) {
    const v = byType[canonical];
    if (v !== undefined) return v;
  }
  return displayNameOf(canonical);
}
