// Static registry of human-readable entity descriptions.
//
// PR-rename-entities: keyed by the user-facing dotted hierarchical name
// (the same form shown on the dashboard and used as MQTT topic-tails).
// `render.ts::descriptionSection` translates the canonical snake_case
// id (matches snapshot field names) into the dotted form via
// `displayNameOfTyped` before lookup. Used inside the entity inspector
// popup for every entity type (PR-entity-inspectors).
//
// Frontend-only: drift from backend renames is acceptable risk —
// missing keys simply render without a description.

export const entityDescriptions: Record<string, string> = {
  // --- Sensors (20) ---
  "battery.soc": "Pylontech battery state of charge (%).",
  "battery.soh": "Pylontech battery state of health (%). Reseed-driven; rarely changes.",
  "battery.capacity.installed":
    "Installed Pylontech battery capacity (kWh / Ah, per Victron's reading). Effectively static.",
  "battery.power.dc":
    "Battery DC-side instantaneous power (W). Negative = charging, positive = discharging.",
  "solar.mppt.0.power": "MPPT solar charger 0 instantaneous DC power (W). Idle = 0 at night.",
  "solar.mppt.1.power": "MPPT solar charger 1 instantaneous DC power (W). Idle = 0 at night.",
  "solar.soltaro.power":
    "AC-coupled Soltaro battery instantaneous power (W). Negative = charging, positive = discharging.",
  "house.power.consumption":
    "Total household AC consumption (W) reported by Victron. Includes house loads + EV branch.",
  "grid.power":
    "Instantaneous grid power (W) at the meter. Negative = export to grid, positive = import.",
  "grid.voltage": "Grid line voltage (V). Slow-moving sanity signal.",
  "grid.current": "Grid line current (A). Sign matches grid.power.",
  "house.current.consumption": "AC current (A) drawn by household consumption.",
  "inverter.offgrid.power": "Off-grid (inverter output) instantaneous AC power (W).",
  "inverter.offgrid.current": "Off-grid (inverter output) instantaneous AC current (A).",
  "inverter.input.current":
    "VE.Bus input current limit readback (A) — confirms the actuated inverter.input.current-limit reached the inverter.",
  "evcharger.ac.power":
    "Net EV-branch meter (W). Combined Zappi + Hoymiles microinverters; cannot be split per design.",
  "evcharger.ac.current": "Net EV-branch current (A). Sign matches evcharger.ac.power.",
  "inverter.ess.state":
    "Victron ESS state machine code (Keep batteries charged / Optimised w/ or w/o BatteryLife / external control). Reseed-driven from Settings.",
  "weather.temperature.outdoor":
    "Outdoor temperature (°C) sourced from Open-Meteo current weather. Reseed-driven (~30 min).",
  "evcharger.session.energy":
    "Cumulative energy delivered to the EV in the current Zappi session (kWh). Sourced from myenergi 'che' field; resets when the session ends.",
  // PR-ZD-1: zigbee2mqtt power sensors and MPPT operation modes.
  "house.heat-pump.power":
    "Heat pump instantaneous power (W). Sourced from zigbee2mqtt (nodon-mtr-heat-pump). Stale if the device is unavailable or the topic is unconfigured.",
  "house.cooker.power":
    "Cooker/stove instantaneous power (W). Sourced from zigbee2mqtt (nodon-mtr-stove). Stale if the device is unavailable or the topic is unconfigured.",
  "solar.mppt.0.mode.operation":
    "Operation mode of MPPT charger 0 (com.victronenergy.solarcharger.ttyUSB1, DI 289). 0=Off, 1=Voltage-or-current-limited (curtailed by inverter), 2=MPPT-tracking (running unconstrained). Observability only — not coupled into the control loop.",
  "solar.mppt.1.mode.operation":
    "Operation mode of MPPT charger 1 (com.victronenergy.solarcharger.ttyS2, DI 274). 0=Off, 1=Voltage-or-current-limited (curtailed by inverter), 2=MPPT-tracking (running unconstrained). Observability only — not coupled into the control loop.",

  // --- Controller observables (PR-ZDO-2) ---
  "controller.zappi-drain.compensated-w":
    "Compensated battery drain (W) the M-ZAPPI-DRAIN soft loop saw on the most recent controller tick. max(0, -battery_dc_power - heat_pump - cooker). Broadcast-only — also visible on the Detail tab chart.",
  "controller.zappi-drain.tighten-active":
    "True when the soft-loop tightening branch fired this tick (drain > threshold && Zappi active && !allow_battery_to_car). Broadcast-only.",
  "controller.zappi-drain.hard-clamp-active":
    "True when the Fast-mode hard clamp engaged this tick (Zappi target Fast + drain > hard_clamp_w). Broadcast-only.",

  // --- Actuated entities ---
  "grid.setpoint":
    "Commanded AC power setpoint at the grid tie. Negative = export, positive = import. Idle baseline 10 W.",
  "inverter.input.current-limit":
    "Commanded VE.Bus input current limit (A). Caps grid import to the inverter; main lever for grid-charge throttling.",
  "evcharger.mode.target":
    "Commanded Zappi EV-charger mode (Eco / EcoPlus / Fast / Stopped). Driven by solar surplus + tariff bands.",
  "eddi.mode.target":
    "Commanded Eddi diverter mode (Normal / Stopped). Driven locally by battery SoC hysteresis (default Stopped, 96/94).",
  "schedule.0":
    "Victron ESS schedule slot 0. Encodes start time, duration, target SoC, and enabled/disabled bit (days = ±7).",
  "schedule.1":
    "Victron ESS schedule slot 1. Encodes start time, duration, target SoC, and enabled/disabled bit (days = ±7).",

  // --- Decisions (per-controller "why?" explanations) ---
  "weathersoc":
    "Weather-SoC planner: pre-dawn job that picks the night-charge target SoC from forecast totals and outdoor temperature.",

  // --- Bookkeeping fields ---
  "schedule.full-charge.next":
    "Next scheduled weekly full-charge timestamp (ISO 8601). Rolls forward each Sunday 17:00 unless overridden by debug.full-charge.mode.",
  "battery.soc.above-threshold.date":
    "Last calendar date the battery crossed the export threshold (ISO 8601). Used by full-charge gating logic.",
  "inverter.ess.state.previous":
    "Previous Victron ESS state code observed (Victron BatteryLife codes: 0=Unknown · 1=Restart · 2=Default · 3=BatteryLife · 9=KeepBatteriesCharged · 10=Optimized · 11=ExternalControl). Used to detect transitions for bookkeeping side effects.",
  "evcharger.active":
    "Derived flag: true when the EV is genuinely charging (combines mode/plug/time-in-state/power thresholds). Read by setpoint, current-limit, and schedules controllers.",
  "schedule.full-charge.required":
    "True when the weekly full-charge plan is armed for the upcoming night. Forces export threshold to 100% and discharge target down.",
  "battery.soc.target.end-of-day":
    "Effective end-of-evening SoC target (%) selected by the schedules controller from the active knob set.",
  "battery.soc.threshold.export.effective":
    "Effective SoC threshold (%) above which battery export is allowed. Equals battery.soc.threshold.export.forced-value normally; raised to battery.soc.threshold.full-charge.export during full-charge.",
  "battery.soc.target.selected":
    "Effective night-charge SoC target (%) selected per current policy (legacy, full-charge, or weather-SoC).",
  "schedule.extended.charge.today":
    "True if today's weathersoc planner decided the night charge should extend through the NightExtended (05:00–08:00) window. Reset on calendar-day rollover.",
  "schedule.extended.charge.today.date":
    "Calendar date schedule.extended.charge.today was last set for, so the tick-level reset knows when to clear.",

  // --- Knobs (export / discharge policy) ---
  "grid.export.force-disable":
    "When true, setpoint is forced to idle 10 W and grid export is suppressed (kill switch for export).",
  "battery.soc.threshold.export.forced-value":
    "Battery SoC (%) at or above which export is allowed under normal policy.",
  "battery.soc.target.discharge.forced-value":
    "Evening-controller target SoC (%) at end-of-day under normal policy.",
  "battery.soc.target.charge.forced-value":
    "Night-time scheduled charge target SoC (%) under normal policy.",
  "battery.soc.target.full-charge.discharge":
    "Evening target SoC (%) during the weekly full-charge cycle (lower than normal, to make room).",
  "battery.soc.threshold.full-charge.export":
    "Export SoC threshold (%) during the weekly full-charge cycle (typically 100 to forbid export).",
  "battery.discharge.time":
    "End-of-evening discharge cutoff time. At0200 = continue through 02:00; At2300 = truncate at 23:00 (for tariffs with a 23:00 transition).",
  "debug.full-charge.mode":
    "Manual override for the weekly full-charge cycle. Auto = follow schedule; Force = run on next eval; Forbid = skip.",
  "forecast.pessimism.modifier":
    "Multiplier applied to forecast-derived planning estimates. <1 = optimistic, >1 = pessimistic.",
  "grid.night.discharge.disable.forced-value":
    "When true, suppresses grid discharge during the night band. Inverse of legacy charge_battery_extended derivation.",

  // --- Knobs (Zappi / EV) ---
  "evcharger.boost.enable":
    "Boost mode for EV charging — overrides solar-only logic to prioritise getting the car charged.",
  "evcharger.extended.mode":
    "Extended-charge mode for the EV (NightExtended 05:00–08:00 window). Auto = daily 04:30 evaluation enables when ev_soc<40 OR ev_charge_target>80; Forced = always on; Disabled = always off.",
  "evcharger.current.target":
    "Target Zappi charge current (A) under controller-driven modes.",
  "evcharger.session.limit":
    "Per-session EV charge ceiling (kWh). Once the car has drawn ≥ this in the current session, mode is forced Off (only when ≤65 kWh).",
  "evcharger.current.margin":
    "Headroom (kWh) reserved before the evcharger.session.limit cutoff fires. Smooths handoff.",

  // --- Knobs (grid / battery-to-car / Eddi) ---
  "grid.export.limit":
    "Hard cap on negative setpoint magnitude (grid-side export limit, W).",
  "grid.import.limit":
    "Hard cap on positive setpoint magnitude (grid-side import limit, W).",
  "battery.export.car.allow":
    "Permit DC battery to discharge into the EV during Zappi-active windows. Always boots false; never persisted.",
  "eddi.soc.enable":
    "Eddi target becomes Normal when battery SoC ≥ this (%). Default 96.",
  "eddi.soc.disable":
    "Eddi target becomes Stopped when battery SoC ≤ this (%). Default 94 (hysteresis with eddi.soc.enable).",
  "eddi.dwell.seconds":
    "Minimum dwell time (s) at the current Eddi state before re-evaluation.",

  // --- Knobs (weathersoc planner) ---
  "weathersoc.threshold.winter-temperature":
    "Outdoor temperature (°C) below which weathersoc switches to the winter heuristic.",
  "weathersoc.threshold.energy.low":
    "Forecast total energy threshold (kWh) below which weathersoc treats the day as low-yield.",
  "weathersoc.threshold.energy.ok":
    "Forecast total energy threshold (kWh) for an OK-yield day in weathersoc.",
  "weathersoc.threshold.energy.high":
    "Forecast total energy threshold (kWh) for a high-yield day in weathersoc.",
  "weathersoc.threshold.energy.too-much":
    "Forecast total energy threshold (kWh) above which weathersoc backs off the night charge entirely.",

  // --- Knobs (ops) ---
  "writes-enabled":
    "Master kill switch. When false, the service runs in observer mode — no actuation, decisions still computed.",
  "forecast.disagreement.strategy":
    "Fusion strategy when forecast providers disagree: Max / Mean / Min / SolcastIfAvailableElseMean.",
  "schedule.extended.charge.mode":
    "Override for the schedule.extended.charge derivation: Auto / Forced / Disabled.",
  "inverter.safe-discharge.enable":
    "When true, applies the legacy 4020 W safety margin below the inverter's full discharge rating to avoid an observed 'forced grid charge during 4.8 kW+ discharge' glitch on some MultiPlus firmware. Default false — the margin is OFF and the inverter discharges at its full rated capacity. Flip to true if your specific firmware reproduces the glitch.",

  // --- Knobs (baseline forecast — PR-baseline-forecast) ---
  "forecast.baseline.winter.start.mmdd":
    "Winter range start (inclusive), encoded as MMDD (e.g. 1101 for Nov 1). Year-wrapping range together with the end knob.",
  "forecast.baseline.winter.end.mmdd":
    "Winter range end (inclusive), encoded as MMDD (e.g. 301 for Mar 1). Year-wrapping range together with the start knob.",
  "forecast.baseline.wh-per-hour.winter":
    "Average per-daylight-hour Wh during winter. Used by the locally-computed baseline forecast as a rough fallback when all cloud providers are stale.",
  "forecast.baseline.wh-per-hour.summer":
    "Average per-daylight-hour Wh during summer. Used by the locally-computed baseline forecast as a rough fallback when all cloud providers are stale.",

  // --- Actuated (ESS state) ---
  "ess.state.target":
    "Target Victron ESS state (`/Settings/CGwacs/BatteryLife/State`). 9 (KeepBatteriesCharged) on full-charge days inside the [sunrise+offset, sunset-offset] window; 10 (Optimized) at all other times.",

  // --- Knobs (ESS state) ---
  "ess.full-charge.keep-batteries-charged":
    "When true and bookkeeping.charge_to_full_required is set, the controller writes ESS state 9 (KeepBatteriesCharged) inside the daylight window [sunrise + offset, sunset - offset]. Outside the window — or whenever this knob is off, or it's not a full-charge day — the controller writes 10 (Optimized).",
  "ess.full-charge.sunrise-sunset-offset-min":
    "Symmetric inset (minutes) applied to local sunrise and sunset to delimit the keep-batteries-charged override window. Default 60.",
  "full-charge.defer-to-next-sunday":
    "When on, the SoC ≥ 99.99 weekly rollover always lands on the Sunday at-or-after now+7 days — never snaps back to the current week's Sunday. Default off (legacy: Mon/Tue/Wed snap back). Manual edits to the next-full-charge bookkeeping value are not retroactively reinterpreted.",
  "full-charge.snap-back-max-weekday":
    "Inclusive weekday cap (Sun=0, Mon=1, ..., Sat=6) for the snap-back branch of the SoC ≥ 99.99 rollover. When the resulting weekday ≤ cap, the date snaps to this week's Sunday; otherwise it pushes to next Sunday. Range 1..=5; default 3 means Mon/Tue/Wed snap back and Thu/Fri/Sat push forward. Ignored when defer-to-next-sunday is on.",

  // PR-ZD-2: compensated battery-drain feedback loop knobs.
  "zappi.battery-drain.threshold-w":
    "Compensated battery-drain threshold (W). When `compensated_drain = max(0, -battery_dc_power - heat_pump - cooker)` exceeds this value while Zappi is active and `allow_battery_to_car=false`, the controller tightens the grid setpoint to halt battery discharge into the EV. Higher values allow sub-threshold transients through; lower values produce a more aggressive response.",
  "zappi.battery-drain.relax-step-w":
    "Setpoint-relax step (W per controller tick). When compensated drain is below the threshold, the controller relaxes the grid setpoint toward `-solar_export` at this step size per tick. Smaller values produce slower convergence to the export-friendly setpoint.",
  "zappi.battery-drain.kp":
    "Proportional gain on the compensated-drain controller. The controller raises the setpoint by `kp * (drain - threshold)` per tick when drain exceeds threshold. Default 1.0; lower (e.g. 0.3) for a softer response, higher for snappier reaction.",
  "zappi.battery-drain.target-w":
    "Reference for the compensated-drain controller (W). Currently inert — the math uses `threshold` as reference. Reserved for a future PI extension.",
  "zappi.battery-drain.hard-clamp-w":
    "Fast-mode hard-clamp threshold (W). When Zappi is in Fast mode and `allow_battery_to_car=false`, if compensated drain exceeds this value, the controller raises the proposed setpoint by the excess as a separate safety net on top of the soft loop. Defaults to 200 W. Eco / Eco+ / Off modes bypass this clamp.",

  // --- TASS cores (PR-tass-dag-view + PR-rename-entities) ---
  setpoint:
    "Grid setpoint controller — chooses the AC setpoint at the grid tie each tick (idle 10 W or commanded values).",
  "current-limit":
    "VE.Bus input-current-limit controller — caps grid import to the inverter; primary lever for grid-charge throttling.",
  schedules:
    "ESS schedule controller — populates schedule.0 / schedule.1 with start/duration/SoC/enabled bits per current policy.",
  "broadcast.sensor":
    "Sensor broadcast core — runs after every actuator core; publishes the tick's sensor + bookkeeping snapshot to MQTT.",

  // --- Forecast providers ---
  "forecast.solcast": "Solcast forecast provider (free tier, paid for accuracy on this site).",
  "forecast.solar": "Forecast.Solar forecast provider (free tier).",
  "forecast.open-meteo": "Open-Meteo forecast provider (free).",
  "forecast.baseline":
    "Local pessimistic baseline (sunrise/sunset × Wh-per-hour). Used as a last-resort fallback when no cloud provider is fresh.",
};

// Bookkeeping field → list of cores that write to it (PR-entity-inspectors).
// Hand-curated; mirrors the writer set documented in
// crates/core/src/process.rs and the per-controller modules.
// PR-rename-entities: keys + values use the dotted display form.
export const bookkeepingWriters: Record<string, string[]> = {
  "schedule.full-charge.next": ["setpoint"],
  "battery.soc.above-threshold.date": ["schedules"],
  "inverter.ess.state.previous": ["current-limit"],
  "evcharger.active": ["evcharger.active"],
  "schedule.full-charge.required": ["setpoint"],
  "battery.soc.target.end-of-day": ["setpoint"],
  "battery.soc.threshold.export.effective": ["setpoint"],
  "battery.soc.target.selected": ["schedules"],
  "schedule.extended.charge.today": ["weathersoc"],
  "schedule.extended.charge.today.date": ["weathersoc"],
};

// Forecast provider display labels, by dotted display name.
export const forecastProviderLabels: Record<string, string> = {
  "forecast.solcast": "Solcast",
  "forecast.solar": "Forecast.Solar",
  "forecast.open-meteo": "Open-Meteo",
  "forecast.baseline": "Baseline",
};
