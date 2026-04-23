// Render helpers — convert WorldSnapshot into HTML.

import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";
import type { ActualF64 } from "./model/victron_controller/dashboard/ActualF64.js";
import type { ActuatedI32 } from "./model/victron_controller/dashboard/ActuatedI32.js";
import type { ActuatedF64 } from "./model/victron_controller/dashboard/ActuatedF64.js";
import type { ActuatedEnumName } from "./model/victron_controller/dashboard/ActuatedEnumName.js";
import type { ActuatedSchedule } from "./model/victron_controller/dashboard/ActuatedSchedule.js";

function fmtNum(v: number | null | undefined, digits = 1): string {
  if (v === null || v === undefined) return "—";
  if (!isFinite(v)) return String(v);
  return v.toFixed(digits);
}

function fmtEpoch(ms: number): string {
  if (!ms) return "—";
  const dt = (Date.now() - ms) / 1000;
  if (dt < 60) return `${dt.toFixed(0)} s ago`;
  if (dt < 3600) return `${(dt / 60).toFixed(0)} min ago`;
  return new Date(ms).toLocaleString();
}

function esc(s: string): string {
  return s.replace(/[&<>]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" } as Record<string, string>)[ch]!);
}

export function renderSensors(snap: WorldSnapshot) {
  const tbody = document.querySelector("#sensors-table tbody") as HTMLElement;
  const entries = Object.entries(snap.sensors).sort(([a], [b]) => a.localeCompare(b));
  tbody.innerHTML = entries
    .map(([name, a]) => {
      const act = a as ActualF64;
      const valText = act.value === null ? "—" : fmtNum(act.value, 2);
      return `<tr>
        <td class="mono">${esc(name)}</td>
        <td class="mono">${valText}</td>
        <td class="freshness-${act.freshness}">${act.freshness} <span class="dim">(${fmtEpoch(
          act.since_epoch_ms as unknown as number
        )})</span></td>
      </tr>`;
    })
    .join("");
}

export function renderActuated(snap: WorldSnapshot) {
  const tbody = document.querySelector("#actuated-table tbody") as HTMLElement;
  const a = snap.actuated;
  const row = (name: string, target: string, owner: string, phase: string, actual: string, fresh: string, since: number) =>
    `<tr>
      <td class="mono">${name}</td>
      <td class="mono">${target}</td>
      <td>${owner}</td>
      <td class="phase-${phase}">${phase}</td>
      <td class="mono">${actual}</td>
      <td class="freshness-${fresh}">${fresh} <span class="dim">(${fmtEpoch(since)})</span></td>
    </tr>`;
  const gs: ActuatedI32 = a.grid_setpoint;
  const cl: ActuatedF64 = a.input_current_limit;
  const zm: ActuatedEnumName = a.zappi_mode;
  const em: ActuatedEnumName = a.eddi_mode;
  const s0: ActuatedSchedule = a.schedule_0;
  const s1: ActuatedSchedule = a.schedule_1;

  const rows = [
    row(
      "grid_setpoint",
      gs.target_value === null ? "—" : String(gs.target_value),
      String(gs.target_owner),
      String(gs.target_phase),
      gs.actual.value === null ? "—" : String(gs.actual.value),
      String(gs.actual.freshness),
      gs.actual.since_epoch_ms as unknown as number
    ),
    row(
      "input_current_limit",
      cl.target_value === null ? "—" : fmtNum(cl.target_value, 2),
      String(cl.target_owner),
      String(cl.target_phase),
      cl.actual.value === null ? "—" : fmtNum(cl.actual.value, 2),
      String(cl.actual.freshness),
      cl.actual.since_epoch_ms as unknown as number
    ),
    row(
      "zappi_mode",
      zm.target_value ?? "—",
      String(zm.target_owner),
      String(zm.target_phase),
      zm.actual_value ?? "—",
      String(zm.actual_freshness),
      zm.actual_since_epoch_ms as unknown as number
    ),
    row(
      "eddi_mode",
      em.target_value ?? "—",
      String(em.target_owner),
      String(em.target_phase),
      em.actual_value ?? "—",
      String(em.actual_freshness),
      em.actual_since_epoch_ms as unknown as number
    ),
    row(
      "schedule_0",
      s0.target ? esc(JSON.stringify(s0.target)) : "—",
      String(s0.target_owner),
      String(s0.target_phase),
      s0.actual ? esc(JSON.stringify(s0.actual)) : "—",
      String(s0.actual_freshness),
      s0.actual_since_epoch_ms as unknown as number
    ),
    row(
      "schedule_1",
      s1.target ? esc(JSON.stringify(s1.target)) : "—",
      String(s1.target_owner),
      String(s1.target_phase),
      s1.actual ? esc(JSON.stringify(s1.actual)) : "—",
      String(s1.actual_freshness),
      s1.actual_since_epoch_ms as unknown as number
    ),
  ];
  tbody.innerHTML = rows.join("");
}

export function renderBookkeeping(snap: WorldSnapshot) {
  const tbody = document.querySelector("#bk-table tbody") as HTMLElement;
  const entries = Object.entries(snap.bookkeeping);
  tbody.innerHTML = entries
    .map(([name, val]) => {
      let disp: string;
      if (val === null || val === undefined) disp = "—";
      else if (typeof val === "boolean") disp = val ? '<span class="freshness-Fresh">true</span>' : "false";
      else if (typeof val === "number") disp = fmtNum(val, 2);
      else disp = esc(String(val));
      return `<tr><td class="mono">${esc(name)}</td><td>${disp}</td></tr>`;
    })
    .join("");
}

export function renderDecisions(snap: WorldSnapshot) {
  const tbody = document.querySelector("#decisions-table tbody") as HTMLElement;
  const d = snap.decisions;
  const rows: Array<[string, any]> = [
    ["grid_setpoint", d.grid_setpoint],
    ["input_current_limit", d.input_current_limit],
    ["schedule_0", d.schedule_0],
    ["schedule_1", d.schedule_1],
    ["zappi_mode", d.zappi_mode],
    ["eddi_mode", d.eddi_mode],
    ["weather_soc", d.weather_soc],
  ];
  tbody.innerHTML = rows
    .map(([name, dec]) => {
      if (!dec) {
        return `<tr><td class="mono">${name}</td><td class="dim">—</td><td class="dim">—</td></tr>`;
      }
      const factors = (dec.factors as Array<{ name: string; value: string }>)
        .map((f) => `<span class="factor"><b>${esc(f.name)}</b>=${esc(f.value)}</span>`)
        .join(" ");
      return `<tr>
        <td class="mono">${name}</td>
        <td>${esc(dec.summary as string)}</td>
        <td class="factors">${factors}</td>
      </tr>`;
    })
    .join("");
}

export function renderForecasts(snap: WorldSnapshot) {
  const tbody = document.querySelector("#forecasts-table tbody") as HTMLElement;
  const providers: Array<[string, any]> = [
    ["solcast", snap.forecasts.solcast],
    ["forecast_solar", snap.forecasts.forecast_solar],
    ["open_meteo", snap.forecasts.open_meteo],
  ];
  tbody.innerHTML = providers
    .map(([name, f]) => {
      if (!f)
        return `<tr><td class="mono">${name}</td><td class="dim">no data</td><td class="dim">—</td><td class="dim">—</td></tr>`;
      return `<tr>
        <td class="mono">${name}</td>
        <td class="mono">${fmtNum(f.today_kwh, 1)}</td>
        <td class="mono">${fmtNum(f.tomorrow_kwh, 1)}</td>
        <td class="dim">${fmtEpoch(f.fetched_at_epoch_ms)}</td>
      </tr>`;
    })
    .join("");
}
