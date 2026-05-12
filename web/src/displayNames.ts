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
  // PR-ZD-1: zigbee2mqtt power sensors and MPPT operation modes.
  heat_pump_power: "house.heat-pump.power",
  cooker_power: "house.cooker.power",
  mppt_0_operation_mode: "solar.mppt.0.mode.operation",
  mppt_1_operation_mode: "solar.mppt.1.mode.operation",
  // PR-LG-THINQ-B: plain temperature readbacks from the heat pump.
  lg_dhw_actual_c: "lg.dhw.temperature.current",
  lg_heating_water_actual_c: "lg.heating-water.temperature.current",

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
  // PR-WSOC-TABLE-1: bucket-boundary kWh knob.
  weathersoc_very_sunny_threshold: "weathersoc.threshold.energy.very-sunny",
  // PR-WSOC-TABLE-1: 6×2 lookup table (read-only widget on the dashboard).
  weather_soc_table: "weathersoc.table",
  forecast_disagreement_strategy: "forecast.disagreement.strategy",
  charge_battery_extended_mode: "schedule.extended.charge.mode",
  writes_enabled: "writes-enabled",
  // PR-baseline-forecast.
  baseline_winter_start_mm_dd: "forecast.baseline.winter.start.mmdd",
  baseline_winter_end_mm_dd: "forecast.baseline.winter.end.mmdd",
  baseline_wh_per_hour_winter: "forecast.baseline.wh-per-hour.winter",
  baseline_wh_per_hour_summer: "forecast.baseline.wh-per-hour.summer",
  // PR-keep-batteries-charged.
  keep_batteries_charged_during_full_charge: "ess.full-charge.keep-batteries-charged",
  sunrise_sunset_offset_min: "ess.full-charge.sunrise-sunset-offset-min",
  full_charge_defer_to_next_sunday: "full-charge.defer-to-next-sunday",
  full_charge_snap_back_max_weekday: "full-charge.snap-back-max-weekday",
  // PR-ZD-2: compensated battery-drain feedback loop.
  zappi_battery_drain_threshold_w: "zappi.battery-drain.threshold-w",
  zappi_battery_drain_relax_step_w: "zappi.battery-drain.relax-step-w",
  zappi_battery_drain_kp: "zappi.battery-drain.kp",
  zappi_battery_drain_target_w: "zappi.battery-drain.target-w",
  zappi_battery_drain_hard_clamp_w: "zappi.battery-drain.hard-clamp-w",
  // PR-ZDP-1: MPPT curtailment probe.
  zappi_battery_drain_mppt_probe_w: "zappi.battery-drain.mppt-probe-w",
  // PR-ACT-RETRY-1: universal actuator retry threshold.
  actuator_retry_s: "actuator.retry.s",
  // PR-LG-THINQ-B: four heat-pump knobs.
  lg_heat_pump_power: "lg.heat-pump.power",
  lg_dhw_power: "lg.dhw.power",
  lg_heating_water_target_c: "lg.heating-water.target-c",
  lg_dhw_target_c: "lg.dhw.target-c",

  // --- Controller observables (PR-ZDO-2) ---
  controller_zappi_drain_compensated_w: "controller.zappi-drain.compensated-w",
  controller_zappi_drain_tighten_active: "controller.zappi-drain.tighten-active",
  controller_zappi_drain_hard_clamp_active: "controller.zappi-drain.hard-clamp-active",

  // --- Bookkeeping (14) ---
  next_full_charge_iso: "schedule.full-charge.next",
  above_soc_date_iso: "battery.soc.above-threshold.date",
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

  // --- Actuated (7) — `.target` distinguishes from same-named decisions. ---
  grid_setpoint: "grid.setpoint",
  input_current_limit: "inverter.input.current-limit",
  zappi_mode: "evcharger.mode.target",
  eddi_mode: "eddi.mode.target",
  schedule_0: "schedule.0",
  schedule_1: "schedule.1",
  // PR-keep-batteries-charged.
  ess_state_target: "ess.state.target",

  // --- Decisions (7) — keys collide with actuated/cores (zappi_mode,
  // eddi_mode, weather_soc, grid_setpoint, input_current_limit, schedule_*).
  // The user-visible dotted form disambiguates: actuated → `.target`,
  // decision → `.decision`, core → bare. The displayNames map can only
  // hold ONE value per snake_case key — these collisions resolve via the
  // entity-type the renderer passes; see `displayNameOfTyped` below.
  // The default (`displayNameOf`) returns the actuated form — that's what
  // appears in the actuated table; decision/core rows pass type explicitly.

  // --- Forecasts (4) ---
  solcast: "forecast.solcast",
  forecast_solar: "forecast.solar",
  open_meteo: "forecast.open-meteo",
  baseline: "forecast.baseline",
};

// PR-WSOC-EDIT-1: 48 cell knobs. The dashboard receives the cell knobs
// as dotted MQTT names (see `knob_name` in the Rust shell), and the
// dashboard's own snake_case canonical for them is the dotted form
// itself (the cells never appear as struct fields on `snap.knobs`).
// These entries make `displayNameOfTyped(<dotted>, "knob")` resolve to
// the same dotted KNOB_SPEC key, so the entity inspector + KNOB_SPEC
// lookup agree.
const WEATHER_SOC_BUCKETS_FOR_NAMES = [
  "very-sunny",
  "sunny",
  "mid",
  "low",
  "dim",
  "very-dim",
] as const;
const WEATHER_SOC_TEMPS_FOR_NAMES = ["warm", "cold"] as const;
const WEATHER_SOC_FIELDS_FOR_NAMES = [
  "export-soc-threshold",
  "battery-soc-target",
  "discharge-soc-target",
  "extended",
] as const;
for (const bucket of WEATHER_SOC_BUCKETS_FOR_NAMES) {
  for (const temp of WEATHER_SOC_TEMPS_FOR_NAMES) {
    for (const field of WEATHER_SOC_FIELDS_FOR_NAMES) {
      const dotted = `weathersoc.table.${bucket}.${temp}.${field}`;
      DISPLAY_NAMES[dotted] = dotted;
    }
  }
}

// Per-type overrides for collision keys. When the same canonical name
// appears in multiple entity classes, the dotted display name differs
// (e.g. `zappi_mode` actuated → `evcharger.mode.target` vs decision
// → `evcharger.mode.decision` vs core → `evcharger.mode`).
type TypeOverride = Partial<Record<string, string>>;

const DISPLAY_NAMES_BY_TYPE: Record<string, TypeOverride> = {
  // PR-LG-THINQ-B-D07: the four LG actuated keys collide with same-named
  // knob keys in DISPLAY_NAMES (lg_heat_pump_power etc. → knob dotted form).
  // Override here so `displayNameOfTyped(key, "actuated")` returns the
  // correct actuated dotted name while the knob form stays unchanged.
  actuated: {
    lg_heat_pump_power: "lg.heat-pump.power.target",
    lg_dhw_power: "lg.dhw.power.target",
    lg_heating_water_target_c: "lg.heating-water.target",
    lg_dhw_target_c: "lg.dhw.target",
  },
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
