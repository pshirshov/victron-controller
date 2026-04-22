// victron-controller dashboard — vanilla JS, no build step.
//
// Polls /api/snapshot every 2 seconds, renders five sections, lets
// the user POST commands to /api/command to mutate knobs.

const REFRESH_MS = 2000;

function fmtNum(v, digits = 1) {
  if (v === null || v === undefined) return '—';
  if (typeof v !== 'number' || !isFinite(v)) return String(v);
  return v.toFixed(digits);
}

function fmtOpt(v, mapper = (x) => x) {
  if (v === null || v === undefined) return '<span class="dim">—</span>';
  return mapper(v);
}

function fmtEpoch(ms) {
  if (!ms) return '—';
  const d = new Date(ms);
  const now = Date.now();
  const dt = (now - ms) / 1000;
  if (dt < 60) return `${dt.toFixed(0)} s ago`;
  if (dt < 3600) return `${(dt / 60).toFixed(0)} min ago`;
  return d.toLocaleString();
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

function renderSensors(snap) {
  const tbody = document.querySelector('#sensors-table tbody');
  const entries = Object.entries(snap.sensors).sort(([a], [b]) => a.localeCompare(b));
  tbody.innerHTML = entries.map(([name, a]) => {
    const valText = a.value === null ? '—' : fmtNum(a.value, 2);
    return `<tr>
      <td class="mono">${name}</td>
      <td class="mono">${valText}</td>
      <td class="freshness-${a.freshness}">${a.freshness} <span class="dim">(${fmtEpoch(a.since_epoch_ms)})</span></td>
    </tr>`;
  }).join('');
}

function renderActuated(snap) {
  const tbody = document.querySelector('#actuated-table tbody');
  const rows = [
    ['grid_setpoint', snap.actuated.grid_setpoint, 'i32'],
    ['input_current_limit', snap.actuated.input_current_limit, 'f64'],
    ['zappi_mode', snap.actuated.zappi_mode, 'enum'],
    ['eddi_mode', snap.actuated.eddi_mode, 'enum'],
    ['schedule_0', snap.actuated.schedule_0, 'schedule'],
    ['schedule_1', snap.actuated.schedule_1, 'schedule'],
  ];
  tbody.innerHTML = rows.map(([name, a, kind]) => {
    let targetText, actualText, actualFresh, actualSince;
    if (kind === 'schedule') {
      targetText = a.target ? JSON.stringify(a.target) : '—';
      actualText = a.actual ? JSON.stringify(a.actual) : '—';
      actualFresh = a.actual_freshness;
      actualSince = a.actual_since_epoch_ms;
    } else if (kind === 'enum') {
      targetText = a.target_value ?? '—';
      actualText = a.actual_value ?? '—';
      actualFresh = a.actual_freshness;
      actualSince = a.actual_since_epoch_ms;
    } else {
      targetText = a.target_value === null ? '—' : fmtNum(a.target_value, kind === 'i32' ? 0 : 2);
      actualText = a.actual.value === null ? '—' : fmtNum(a.actual.value, kind === 'i32' ? 0 : 2);
      actualFresh = a.actual.freshness;
      actualSince = a.actual.since_epoch_ms;
    }
    return `<tr>
      <td class="mono">${name}</td>
      <td class="mono">${targetText}</td>
      <td>${a.target_owner}</td>
      <td class="phase-${a.target_phase}">${a.target_phase}</td>
      <td class="mono">${actualText}</td>
      <td class="freshness-${actualFresh}">${actualFresh} <span class="dim">(${fmtEpoch(actualSince)})</span></td>
    </tr>`;
  }).join('');
}

function renderBookkeeping(snap) {
  const tbody = document.querySelector('#bk-table tbody');
  const entries = Object.entries(snap.bookkeeping);
  tbody.innerHTML = entries.map(([name, val]) => {
    let disp;
    if (val === null || val === undefined) disp = '—';
    else if (typeof val === 'boolean') disp = val ? '<span class="freshness-Fresh">true</span>' : 'false';
    else if (typeof val === 'number') disp = fmtNum(val, 2);
    else disp = String(val);
    return `<tr><td class="mono">${name}</td><td>${disp}</td></tr>`;
  }).join('');
}

function renderForecasts(snap) {
  const tbody = document.querySelector('#forecasts-table tbody');
  const providers = [
    ['solcast', snap.forecasts.solcast],
    ['forecast_solar', snap.forecasts.forecast_solar],
    ['open_meteo', snap.forecasts.open_meteo],
  ];
  tbody.innerHTML = providers.map(([name, f]) => {
    if (!f) return `<tr><td class="mono">${name}</td><td class="dim">no data</td><td class="dim">—</td><td class="dim">—</td></tr>`;
    return `<tr>
      <td class="mono">${name}</td>
      <td class="mono">${fmtNum(f.today_kwh, 1)}</td>
      <td class="mono">${fmtNum(f.tomorrow_kwh, 1)}</td>
      <td class="dim">${fmtEpoch(f.fetched_at_epoch_ms)}</td>
    </tr>`;
  }).join('');
}

// ---------------------------------------------------------------------------
// Knobs — with inline editing
// ---------------------------------------------------------------------------

// Per-knob metadata: { type, cmdVariant, stringify, parse, inputType, options? }
const KNOB_SPEC = {
  force_disable_export: boolKnob(),
  export_soc_threshold: floatKnob(0, 100, 1),
  discharge_soc_target: floatKnob(0, 100, 1),
  battery_soc_target: floatKnob(0, 100, 1),
  full_charge_discharge_soc_target: floatKnob(0, 100, 1),
  full_charge_export_soc_threshold: floatKnob(0, 100, 1),
  discharge_time: enumKnob('SetDischargeTime', ['At0200', 'At2300']),
  debug_full_charge: enumKnob('SetDebugFullCharge', ['Forbid', 'Force', 'None_']),
  pessimism_multiplier_modifier: floatKnob(0, 2, 0.05),
  disable_night_grid_discharge: boolKnob(),
  charge_car_boost: boolKnob(),
  charge_car_extended: boolKnob(),
  zappi_current_target: floatKnob(6, 32, 0.5),
  zappi_limit: floatKnob(1, 100, 1),
  zappi_emergency_margin: floatKnob(0, 10, 0.5),
  grid_export_limit_w: intKnob(0, 10000, 50),
  allow_battery_to_car: boolKnob(),
  eddi_enable_soc: floatKnob(50, 100, 1),
  eddi_disable_soc: floatKnob(50, 100, 1),
  eddi_dwell_s: intKnob(0, 3600, 5),
  weathersoc_winter_temperature_threshold: floatKnob(-30, 40, 0.5),
  weathersoc_low_energy_threshold: floatKnob(0, 500, 1),
  weathersoc_ok_energy_threshold: floatKnob(0, 500, 1),
  weathersoc_high_energy_threshold: floatKnob(0, 500, 1),
  weathersoc_too_much_energy_threshold: floatKnob(0, 500, 1),
  forecast_disagreement_strategy: enumKnob(
    'SetForecastDisagreementStrategy',
    ['Max', 'Min', 'Mean', 'SolcastIfAvailableElseMean']
  ),
};

function boolKnob() { return { kind: 'bool' }; }
function floatKnob(min, max, step) { return { kind: 'float', min, max, step }; }
function intKnob(min, max, step) { return { kind: 'int', min, max, step }; }
function enumKnob(cmdVariant, options) { return { kind: 'enum', cmdVariant, options }; }

function renderKnobs(snap) {
  // Kill switch is rendered separately.
  const kill = document.getElementById('kill-switch');
  const enabled = snap.knobs.writes_enabled;
  kill.innerHTML = `
    <div>Kill switch: <strong class="${enabled ? 'freshness-Fresh' : 'freshness-Unknown'}">${enabled ? 'writes ENABLED' : 'writes DISABLED (observer mode)'}</strong></div>
    <button onclick="postKillSwitch(${!enabled})">${enabled ? 'Disable writes' : 'Enable writes'}</button>
  `;

  const tbody = document.querySelector('#knobs-table tbody');
  const entries = Object.entries(snap.knobs)
    .filter(([name]) => name !== 'writes_enabled')
    .sort(([a], [b]) => a.localeCompare(b));
  tbody.innerHTML = entries.map(([name, val]) => {
    const spec = KNOB_SPEC[name];
    const setHtml = spec ? renderSetControl(name, val, spec) : '';
    const valStr = typeof val === 'boolean' ? (val ? 'true' : 'false') : (typeof val === 'number' ? fmtNum(val, 3) : String(val));
    return `<tr>
      <td class="mono">${name}</td>
      <td class="mono">${valStr}</td>
      <td>${setHtml}</td>
    </tr>`;
  }).join('');
}

function renderSetControl(name, currentValue, spec) {
  switch (spec.kind) {
    case 'bool':
      return `<button class="secondary" onclick="postBoolKnob('${name}', ${!currentValue})">
        set to ${!currentValue}
      </button>`;
    case 'float':
    case 'int': {
      const id = `knob-input-${name}`;
      return `<input id="${id}" type="number" step="${spec.step}" min="${spec.min}" max="${spec.max}" value="${currentValue}">
              <button onclick="postNumKnob('${name}', '${spec.kind}')">set</button>`;
    }
    case 'enum': {
      const id = `knob-input-${name}`;
      const optsHtml = spec.options.map((o) => {
        const selected = o === currentValue ? ' selected' : '';
        return `<option value="${o}"${selected}>${o}</option>`;
      }).join('');
      return `<select id="${id}">${optsHtml}</select>
              <button onclick="postEnumKnob('${name}', '${spec.cmdVariant}')">set</button>`;
    }
    default:
      return '';
  }
}

// ---------------------------------------------------------------------------
// Command POSTs
// ---------------------------------------------------------------------------

async function postCommand(cmd) {
  try {
    const resp = await fetch('/api/command', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify(cmd),
    });
    const body = await resp.json();
    if (!resp.ok || !body.accepted) {
      setError(body.error_message || `HTTP ${resp.status}`);
    } else {
      setError('');
    }
    // Force refresh so the UI shows the new value immediately.
    setTimeout(refresh, 100);
  } catch (e) {
    setError(e.message);
  }
}

window.postBoolKnob = (name, value) => {
  postCommand({ SetBoolKnob: { knob_name: name, value } });
};
window.postNumKnob = (name, kind) => {
  const el = document.getElementById(`knob-input-${name}`);
  const value = parseFloat(el.value);
  if (isNaN(value)) return setError(`invalid number for ${name}`);
  if (kind === 'int') {
    postCommand({ SetUintKnob: { knob_name: name, value: Math.round(value) } });
  } else {
    postCommand({ SetFloatKnob: { knob_name: name, value } });
  }
};
window.postEnumKnob = (name, cmdVariant) => {
  const el = document.getElementById(`knob-input-${name}`);
  const value = el.value;
  postCommand({ [cmdVariant]: { value } });
};
window.postKillSwitch = (value) => {
  postCommand({ SetKillSwitch: { value } });
};

// ---------------------------------------------------------------------------
// Refresh loop
// ---------------------------------------------------------------------------

function setError(msg) {
  document.getElementById('last-error').textContent = msg || '';
}

async function refresh() {
  const ind = document.getElementById('refresh-indicator');
  ind.textContent = 'refreshing…';
  try {
    const resp = await fetch('/api/snapshot');
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const snap = await resp.json();
    document.getElementById('captured-at').textContent = snap.captured_at_naive_iso;
    document.getElementById('writes-badge').textContent = snap.knobs.writes_enabled ? 'WRITES ON' : 'OBSERVER';
    document.getElementById('writes-badge').className = 'badge ' + (snap.knobs.writes_enabled ? 'on' : 'off');
    renderSensors(snap);
    renderActuated(snap);
    renderBookkeeping(snap);
    renderForecasts(snap);
    renderKnobs(snap);
    setError('');
    ind.textContent = `updated ${new Date().toLocaleTimeString()}`;
  } catch (e) {
    setError(e.message);
    ind.textContent = 'stale';
  }
}

refresh();
setInterval(refresh, REFRESH_MS);
