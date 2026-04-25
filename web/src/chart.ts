// PR-soc-chart: hand-rolled SVG renderer for the battery-SoC chart.
//
// Three traces on a single SVG:
//   1. History    — recorded samples (last 48 h, every 15 min) as a
//                   solid polyline.
//   2. Now marker — vertical dashed line at the current snapshot time.
//   3. Projection — straight-line linear extrapolation forward, ending
//                   at SoC = 100 % (filling), 10 % (depleting), the
//                   +24 h clamp, or a flat horizon when slope is None.
//
// Interactivity: a mouse-/touch-driven hairline with a tooltip showing
// the time + SoC at that x-position. No charting library; everything is
// hand-written SVG so we don't ship any new TS deps.

import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";

type HistorySample = { epoch_ms: number; soc_pct: number };
type Projection = {
  slope_pct_per_hour: number | null;
  terminus_epoch_ms: number | null;
  terminus_soc_pct: number | null;
  net_power_w: number | null;
  capacity_wh: number | null;
};
type SocChart = {
  history: HistorySample[];
  projection: Projection;
  now_epoch_ms: number;
  now_soc_pct: number | null;
};

// SVG layout. viewBox is fluid via preserveAspectRatio="none" so the
// chart scales to the wrapping div's width.
const VB_W = 800;
const VB_H = 220;
const PAD_L = 40;
const PAD_R = 8;
const PAD_T = 8;
const PAD_B = 30;
const PLOT_W = VB_W - PAD_L - PAD_R;
const PLOT_H = VB_H - PAD_T - PAD_B;

const HOUR_MS = 3_600_000;
const DEFAULT_PROJECTION_HORIZON_MS = 12 * HOUR_MS;

// Adaptive X-axis label step: pick the smallest step that yields ≤ 10
// labels across the visible domain. Mirrors the spec's "~6-10 labels".
const X_STEPS_MS: number[] = [
  15 * 60 * 1000,        // 15 min
  30 * 60 * 1000,        // 30 min
  HOUR_MS,
  2 * HOUR_MS,
  4 * HOUR_MS,
  8 * HOUR_MS,
  12 * HOUR_MS,
  24 * HOUR_MS,
];

function asNum(v: unknown): number | null {
  if (v === null || v === undefined) return null;
  if (typeof v === "number" && isFinite(v)) return v;
  // bigint or string — coerce.
  if (typeof v === "bigint") return Number(v);
  if (typeof v === "string") {
    const n = Number(v);
    return isFinite(n) ? n : null;
  }
  return null;
}

function readChart(snap: WorldSnapshot): SocChart | null {
  const raw = (snap as unknown as { soc_chart?: unknown }).soc_chart;
  if (!raw || typeof raw !== "object") return null;
  const c = raw as Record<string, unknown>;
  const history = (c.history as Array<{ epoch_ms: unknown; soc_pct: unknown }>) ?? [];
  const projRaw = (c.projection as Record<string, unknown>) ?? {};
  const proj: Projection = {
    slope_pct_per_hour: asNum(projRaw.slope_pct_per_hour),
    terminus_epoch_ms: asNum(projRaw.terminus_epoch_ms),
    terminus_soc_pct: asNum(projRaw.terminus_soc_pct),
    net_power_w: asNum(projRaw.net_power_w),
    capacity_wh: asNum(projRaw.capacity_wh),
  };
  return {
    history: history
      .map((s) => ({
        epoch_ms: asNum(s.epoch_ms) ?? 0,
        soc_pct: asNum(s.soc_pct) ?? 0,
      }))
      .filter((s) => s.epoch_ms > 0),
    projection: proj,
    now_epoch_ms: asNum(c.now_epoch_ms) ?? Date.now(),
    now_soc_pct: asNum(c.now_soc_pct),
  };
}

function fmtClock(ms: number): string {
  const d = new Date(ms);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}

function fmtClockSec(ms: number): string {
  const d = new Date(ms);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

function pickXStep(domainMs: number): number {
  // Aim for ~6-10 labels across `domainMs`. Pick the smallest step where
  // ceil(domain / step) ≤ 10.
  for (const step of X_STEPS_MS) {
    if (Math.ceil(domainMs / step) <= 10) return step;
  }
  return X_STEPS_MS[X_STEPS_MS.length - 1];
}

function xToSvg(epochMs: number, x0: number, x1: number): number {
  if (x1 === x0) return PAD_L;
  const t = (epochMs - x0) / (x1 - x0);
  return PAD_L + Math.max(0, Math.min(1, t)) * PLOT_W;
}

function yToSvg(soc: number): number {
  // Y axis is 0..100; SVG y grows downward.
  const t = Math.max(0, Math.min(100, soc)) / 100;
  return PAD_T + (1 - t) * PLOT_H;
}

function svgFromX(svgX: number, x0: number, x1: number): number {
  // Inverse of xToSvg — clamp to the plot rect.
  const cx = Math.max(PAD_L, Math.min(PAD_L + PLOT_W, svgX));
  const t = (cx - PAD_L) / PLOT_W;
  return x0 + t * (x1 - x0);
}

// Find the closest history sample to `epochMs`. O(n) over ≤192 points.
function nearestHistory(history: HistorySample[], epochMs: number): HistorySample | null {
  if (history.length === 0) return null;
  let best = history[0];
  let bestD = Math.abs(history[0].epoch_ms - epochMs);
  for (let i = 1; i < history.length; i++) {
    const d = Math.abs(history[i].epoch_ms - epochMs);
    if (d < bestD) {
      bestD = d;
      best = history[i];
    }
  }
  return best;
}

// SoC at `epochMs` along the projection line, given (now, slope).
// Returns null when the inputs aren't usable.
function projectionSocAt(
  epochMs: number,
  nowMs: number,
  nowSoc: number | null,
  slopePctPerHour: number | null,
): number | null {
  if (nowSoc === null || slopePctPerHour === null) return null;
  const dh = (epochMs - nowMs) / HOUR_MS;
  return nowSoc + slopePctPerHour * dh;
}

function escAttr(s: string): string {
  return s.replace(/[&<>"']/g, (ch) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch]!),
  );
}

export function renderSocChart(snap: WorldSnapshot): void {
  const host = document.getElementById("soc-chart");
  if (!host) return;
  const chart = readChart(snap);
  if (!chart) {
    host.innerHTML = "";
    return;
  }

  const nowMs = chart.now_epoch_ms;
  const nowSoc = chart.now_soc_pct;
  const projection = chart.projection;
  const slope = projection.slope_pct_per_hour;
  const projectionTerminusMs = projection.terminus_epoch_ms;
  const projectionTerminusSoc = projection.terminus_soc_pct;

  // X domain. When history is empty (boot state), fall back to a
  // 13-hour window centered on `now`.
  const firstHistMs =
    chart.history.length > 0 ? chart.history[0].epoch_ms : nowMs - 1 * HOUR_MS;
  // The right edge is the latest of (now + 1h, projection terminus, or
  // now + 12h when slope is None). We want a visible right margin past
  // the now-marker even with no projection.
  let rightMs = nowMs + HOUR_MS;
  if (projectionTerminusMs !== null && projectionTerminusMs > rightMs) {
    rightMs = projectionTerminusMs;
  }
  if (slope === null) {
    rightMs = Math.max(rightMs, nowMs + DEFAULT_PROJECTION_HORIZON_MS);
  }
  const x0 = Math.min(firstHistMs, nowMs - HOUR_MS);
  const x1 = Math.max(rightMs, nowMs + HOUR_MS);

  // --- assemble SVG --------------------------------------------------
  const parts: string[] = [];
  parts.push(
    `<svg viewBox="0 0 ${VB_W} ${VB_H}" preserveAspectRatio="none" role="img" aria-label="Battery SoC history and projection">`,
  );

  // Plot rect (transparent — gives the hairline a hit area).
  parts.push(
    `<rect class="plot-bg" x="${PAD_L}" y="${PAD_T}" width="${PLOT_W}" height="${PLOT_H}" fill="transparent" />`,
  );

  // Y gridlines at 0, 25, 50, 75, 100.
  const yTicks = [0, 25, 50, 75, 100];
  for (const v of yTicks) {
    const y = yToSvg(v);
    parts.push(
      `<line class="axis-grid" x1="${PAD_L}" y1="${y}" x2="${PAD_L + PLOT_W}" y2="${y}" />`,
    );
    parts.push(
      `<text class="axis-label" x="${PAD_L - 4}" y="${y + 3}" text-anchor="end">${v}%</text>`,
    );
  }

  // X gridlines + labels at adaptive step. Align to local-clock
  // boundaries so labels sit on round HH:MM values.
  const step = pickXStep(x1 - x0);
  // First tick ≥ x0 aligned to a step from the local-clock midnight.
  const midnight = new Date(x0);
  midnight.setHours(0, 0, 0, 0);
  const startTick = midnight.getTime() + Math.ceil((x0 - midnight.getTime()) / step) * step;
  for (let t = startTick; t <= x1; t += step) {
    const x = xToSvg(t, x0, x1);
    parts.push(
      `<line class="axis-grid" x1="${x}" y1="${PAD_T}" x2="${x}" y2="${PAD_T + PLOT_H}" />`,
    );
    parts.push(
      `<text class="axis-label" x="${x}" y="${PAD_T + PLOT_H + 14}" text-anchor="middle">${fmtClock(t)}</text>`,
    );
  }

  // History polyline.
  if (chart.history.length >= 2) {
    const points = chart.history
      .map((s) => `${xToSvg(s.epoch_ms, x0, x1).toFixed(2)},${yToSvg(s.soc_pct).toFixed(2)}`)
      .join(" ");
    parts.push(`<polyline class="trace-history" points="${points}" />`);
  } else if (chart.history.length === 1) {
    // Single point — render as a small circle so it's visible.
    const s = chart.history[0];
    parts.push(
      `<circle class="trace-history" cx="${xToSvg(s.epoch_ms, x0, x1)}" cy="${yToSvg(s.soc_pct)}" r="2" />`,
    );
  }

  // Empty-history note.
  if (chart.history.length === 0) {
    parts.push(
      `<text class="terminus-label" x="${PAD_L + PLOT_W / 2}" y="${PAD_T + PLOT_H / 2}" text-anchor="middle">no history yet</text>`,
    );
  }

  // Projection line.
  if (nowSoc !== null) {
    if (slope !== null && projectionTerminusMs !== null && projectionTerminusSoc !== null) {
      const x1p = xToSvg(nowMs, x0, x1);
      const y1p = yToSvg(nowSoc);
      const x2p = xToSvg(projectionTerminusMs, x0, x1);
      const y2p = yToSvg(projectionTerminusSoc);
      parts.push(
        `<line class="trace-projection" x1="${x1p}" y1="${y1p}" x2="${x2p}" y2="${y2p}" />`,
      );
      // Annotation: "→ 100% at HH:MM" / "→ 10% at HH:MM".
      const targetSocLabel =
        Math.abs(projectionTerminusSoc - 100) < 0.1
          ? "100%"
          : Math.abs(projectionTerminusSoc - 10) < 0.1
            ? "10%"
            : `${projectionTerminusSoc.toFixed(0)}%`;
      const annotX = Math.min(x2p + 4, PAD_L + PLOT_W - 60);
      parts.push(
        `<text class="terminus-label" x="${annotX}" y="${Math.max(PAD_T + 12, y2p - 4)}" text-anchor="start">→ ${targetSocLabel} at ${fmtClock(projectionTerminusMs)}</text>`,
      );
    } else {
      // Slope None (idle) — flat horizontal line out to +12h.
      const x1p = xToSvg(nowMs, x0, x1);
      const yFlat = yToSvg(nowSoc);
      const x2p = xToSvg(nowMs + DEFAULT_PROJECTION_HORIZON_MS, x0, x1);
      parts.push(
        `<line class="trace-projection" x1="${x1p}" y1="${yFlat}" x2="${x2p}" y2="${yFlat}" />`,
      );
      parts.push(
        `<text class="terminus-label" x="${Math.min(x2p + 4, PAD_L + PLOT_W - 30)}" y="${Math.max(PAD_T + 12, yFlat - 4)}" text-anchor="start">idle</text>`,
      );
    }
  }

  // Now marker (drawn last over the projection start so it's visible).
  const xNow = xToSvg(nowMs, x0, x1);
  parts.push(
    `<line class="now-marker" x1="${xNow}" y1="${PAD_T}" x2="${xNow}" y2="${PAD_T + PLOT_H}" />`,
  );

  // Hairline + tooltip placeholders, hidden until mousemove.
  parts.push(
    `<line class="hairline" id="soc-chart-hairline" x1="0" y1="${PAD_T}" x2="0" y2="${PAD_T + PLOT_H}" style="display:none" />`,
  );
  parts.push(
    `<text class="hairline-label" id="soc-chart-hairline-label" x="${PAD_L + PLOT_W - 4}" y="${PAD_T + 12}" text-anchor="end" style="display:none"></text>`,
  );

  parts.push(`</svg>`);
  host.innerHTML = parts.join("");

  // Wire mouse + touch handlers. The SVG is fully replaced on every
  // applySnapshot, so we re-attach each render.
  installHairlineHandlers(host, chart, x0, x1);
}

function installHairlineHandlers(
  host: HTMLElement,
  chart: SocChart,
  x0: number,
  x1: number,
): void {
  const svg = host.querySelector("svg");
  if (!svg) return;
  const hairline = svg.querySelector<SVGLineElement>("#soc-chart-hairline");
  const label = svg.querySelector<SVGTextElement>("#soc-chart-hairline-label");
  if (!hairline || !label) return;

  const onMove = (clientX: number) => {
    const rect = svg.getBoundingClientRect();
    if (rect.width === 0) return;
    // Map clientX → viewBox x.
    const svgX = ((clientX - rect.left) / rect.width) * VB_W;
    if (svgX < PAD_L || svgX > PAD_L + PLOT_W) {
      hairline.style.display = "none";
      label.style.display = "none";
      return;
    }
    const epochMs = svgFromX(svgX, x0, x1);
    let soc: number | null = null;
    if (epochMs <= chart.now_epoch_ms) {
      const nearest = nearestHistory(chart.history, epochMs);
      soc = nearest ? nearest.soc_pct : null;
    } else {
      soc = projectionSocAt(
        epochMs,
        chart.now_epoch_ms,
        chart.now_soc_pct,
        chart.projection.slope_pct_per_hour,
      );
    }
    hairline.setAttribute("x1", String(svgX));
    hairline.setAttribute("x2", String(svgX));
    hairline.style.display = "";
    const socText = soc === null ? "—" : `${soc.toFixed(1)}%`;
    label.textContent = `${fmtClockSec(epochMs)} — ${socText}`;
    label.style.display = "";
  };

  const leave = () => {
    hairline.style.display = "none";
    label.style.display = "none";
  };

  svg.addEventListener("mousemove", (ev) => onMove(ev.clientX));
  svg.addEventListener("mouseleave", leave);
  svg.addEventListener(
    "touchstart",
    (ev) => {
      if (ev.touches.length > 0) onMove(ev.touches[0].clientX);
    },
    { passive: true },
  );
  svg.addEventListener(
    "touchmove",
    (ev) => {
      if (ev.touches.length > 0) onMove(ev.touches[0].clientX);
    },
    { passive: true },
  );
  svg.addEventListener("touchend", leave);
  svg.addEventListener("touchcancel", leave);

  // Suppress an unused-binding warning for the helper above, in case
  // this branch never fires under tsc's strict mode.
  void escAttr;
}
