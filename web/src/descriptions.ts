// Static registry of human-readable entity descriptions, keyed by the
// canonical name as it appears in the dashboard tables (sensor name,
// knob name, actuated-entity name, bookkeeping field, decision name,
// forecast-provider name). Used as the `title=` attribute on every
// row's first cell so users get a native browser tooltip on hover.
//
// Frontend-only: drift from backend renames is acceptable risk —
// missing keys simply render with no tooltip.

export const entityDescriptions: Record<string, string> = {
  // --- Sensors (20 — see crates/dashboard-model/.../sensors.rs) ---
  battery_soc: "Pylontech battery state of charge (%).",
  battery_soh: "Pylontech battery state of health (%). Reseed-driven; rarely changes.",
  battery_installed_capacity:
    "Installed Pylontech battery capacity (kWh / Ah, per Victron's reading). Effectively static.",
  battery_dc_power:
    "Battery DC-side instantaneous power (W). Negative = charging, positive = discharging.",
  mppt_power_0: "MPPT solar charger 0 instantaneous DC power (W). Idle = 0 at night.",
  mppt_power_1: "MPPT solar charger 1 instantaneous DC power (W). Idle = 0 at night.",
  soltaro_power:
    "AC-coupled Soltaro battery instantaneous power (W). Negative = charging, positive = discharging.",
  power_consumption:
    "Total household AC consumption (W) reported by Victron. Includes house loads + EV branch.",
  grid_power:
    "Instantaneous grid power (W) at the meter. Negative = export to grid, positive = import.",
  grid_voltage: "Grid line voltage (V). Slow-moving sanity signal.",
  grid_current: "Grid line current (A). Sign matches grid_power.",
  consumption_current: "AC current (A) drawn by household consumption.",
  offgrid_power: "Off-grid (inverter output) instantaneous AC power (W).",
  offgrid_current: "Off-grid (inverter output) instantaneous AC current (A).",
  vebus_input_current:
    "VE.Bus input current limit readback (A) — confirms the actuated input_current_limit reached the inverter.",
  evcharger_ac_power:
    "Net EV-branch meter (W). Combined Zappi + Hoymiles microinverters; cannot be split per design.",
  evcharger_ac_current: "Net EV-branch current (A). Sign matches evcharger_ac_power.",
  ess_state:
    "Victron ESS state machine code (Keep batteries charged / Optimised w/ or w/o BatteryLife / external control). Reseed-driven from Settings.",
  outdoor_temperature:
    "Outdoor temperature (°C) sourced from Open-Meteo current weather. Reseed-driven (~30 min).",
  session_kwh:
    "Cumulative energy delivered to the EV in the current Zappi session (kWh). Sourced from myenergi 'che' field; resets when the session ends.",

  // --- Actuated entities ---
  grid_setpoint:
    "Commanded AC power setpoint at the grid tie. Negative = export, positive = import. Idle baseline 10 W.",
  input_current_limit:
    "Commanded VE.Bus input current limit (A). Caps grid import to the inverter; main lever for grid-charge throttling.",
  zappi_mode:
    "Commanded Zappi EV-charger mode (Eco / EcoPlus / Fast / Stopped). Driven by solar surplus + tariff bands.",
  eddi_mode:
    "Commanded Eddi diverter mode (Normal / Stopped). Driven locally by battery SoC hysteresis (default Stopped, 96/94).",
  schedule_0:
    "Victron ESS schedule slot 0. Encodes start time, duration, target SoC, and enabled/disabled bit (days = ±7).",
  schedule_1:
    "Victron ESS schedule slot 1. Encodes start time, duration, target SoC, and enabled/disabled bit (days = ±7).",

  // --- Decisions (per-controller "why?" explanations) ---
  weather_soc:
    "Weather-SoC planner: pre-dawn job that picks the night-charge target SoC from forecast totals and outdoor temperature.",

  // --- Bookkeeping fields ---
  next_full_charge_iso:
    "Next scheduled weekly full-charge timestamp (ISO 8601). Rolls forward each Sunday 17:00 unless overridden by debug_full_charge.",
  above_soc_date_iso:
    "Last calendar date the battery crossed the export threshold (ISO 8601). Used by full-charge gating logic.",
  prev_ess_state:
    "Previous Victron ESS state code observed (Victron BatteryLife codes: 0=Unknown · 1=Restart · 2=Default · 3=BatteryLife · 9=KeepBatteriesCharged · 10=Optimized · 11=ExternalControl). Used to detect transitions for bookkeeping side effects.",
  zappi_active:
    "Derived flag: true when the EV is genuinely charging (combines mode/plug/time-in-state/power thresholds). Read by setpoint, current-limit, and schedules controllers.",
  charge_to_full_required:
    "True when the weekly full-charge plan is armed for the upcoming night. Forces export threshold to 100% and discharge target down.",
  soc_end_of_day_target:
    "Effective end-of-evening SoC target (%) selected by the schedules controller from the active knob set.",
  effective_export_soc_threshold:
    "Effective SoC threshold (%) above which battery export is allowed. Equals export_soc_threshold normally; raised to full_charge_export_soc_threshold during full-charge.",
  battery_selected_soc_target:
    "Effective night-charge SoC target (%) selected per current policy (legacy, full-charge, or weather-SoC).",
  charge_battery_extended_today:
    "True if today's weather_soc decided the night charge should extend through the NightExtended (05:00–08:00) window. Reset on calendar-day rollover.",
  charge_battery_extended_today_date_iso:
    "Calendar date charge_battery_extended_today was last set for, so the tick-level reset knows when to clear.",

  // --- Knobs (export / discharge policy) ---
  force_disable_export:
    "When true, setpoint is forced to idle 10 W and grid export is suppressed (kill switch for export).",
  export_soc_threshold:
    "Battery SoC (%) at or above which export is allowed under normal policy.",
  discharge_soc_target:
    "Evening-controller target SoC (%) at end-of-day under normal policy.",
  battery_soc_target:
    "Night-time scheduled charge target SoC (%) under normal policy.",
  full_charge_discharge_soc_target:
    "Evening target SoC (%) during the weekly full-charge cycle (lower than normal, to make room).",
  full_charge_export_soc_threshold:
    "Export SoC threshold (%) during the weekly full-charge cycle (typically 100 to forbid export).",
  discharge_time:
    "End-of-evening discharge cutoff time. At0200 = continue through 02:00; At2300 = truncate at 23:00 (for tariffs with a 23:00 transition).",
  debug_full_charge:
    "Manual override for the weekly full-charge cycle. None_ = follow schedule; Force = run on next eval; Forbid = skip.",
  pessimism_multiplier_modifier:
    "Multiplier applied to forecast-derived planning estimates. <1 = optimistic, >1 = pessimistic.",
  disable_night_grid_discharge:
    "When true, suppresses grid discharge during the night band. Inverse of legacy charge_battery_extended derivation.",

  // --- Knobs (Zappi / EV) ---
  charge_car_boost:
    "Boost mode for EV charging — overrides solar-only logic to prioritise getting the car charged.",
  charge_car_extended:
    "Extended charging mode for the EV (longer/looser thresholds).",
  zappi_current_target:
    "Target Zappi charge current (A) under controller-driven modes.",
  zappi_limit:
    "Per-session EV charge ceiling (kWh). Once the car has drawn ≥ this in the current session, mode is forced Off (only when ≤65 kWh).",
  zappi_emergency_margin:
    "Headroom (kWh) reserved before the zappi_limit cutoff fires. Smooths handoff.",

  // --- Knobs (grid / battery-to-car / Eddi) ---
  grid_export_limit_w:
    "Hard cap on negative setpoint magnitude (grid-side export limit, W).",
  grid_import_limit_w:
    "Hard cap on positive setpoint magnitude (grid-side import limit, W).",
  allow_battery_to_car:
    "Permit DC battery to discharge into the EV during Zappi-active windows. Always boots false; never persisted.",
  eddi_enable_soc:
    "Eddi target becomes Normal when battery SoC ≥ this (%). Default 96.",
  eddi_disable_soc:
    "Eddi target becomes Stopped when battery SoC ≤ this (%). Default 94 (hysteresis with eddi_enable_soc).",
  eddi_dwell_s:
    "Minimum dwell time (s) at the current Eddi state before re-evaluation.",

  // --- Knobs (weather-SoC planner) ---
  weathersoc_winter_temperature_threshold:
    "Outdoor temperature (°C) below which weather-SoC switches to the winter heuristic.",
  weathersoc_low_energy_threshold:
    "Forecast total energy threshold (kWh) below which weather-SoC treats the day as low-yield.",
  weathersoc_ok_energy_threshold:
    "Forecast total energy threshold (kWh) for an OK-yield day in weather-SoC.",
  weathersoc_high_energy_threshold:
    "Forecast total energy threshold (kWh) for a high-yield day in weather-SoC.",
  weathersoc_too_much_energy_threshold:
    "Forecast total energy threshold (kWh) above which weather-SoC backs off the night charge entirely.",

  // --- Knobs (ops) ---
  writes_enabled:
    "Master kill switch. When false, the service runs in observer mode — no actuation, decisions still computed.",
  forecast_disagreement_strategy:
    "Fusion strategy when forecast providers disagree: Max / Mean / Min / SolcastIfAvailableElseMean.",
  charge_battery_extended_mode:
    "Override for the charge_battery_extended derivation: Auto / Forced / Disabled.",

  // --- Forecast providers ---
  solcast: "Solcast forecast provider (free tier, paid for accuracy on this site).",
  forecast_solar: "Forecast.Solar forecast provider (free tier).",
  open_meteo: "Open-Meteo forecast provider (free).",
};
