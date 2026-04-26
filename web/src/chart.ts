// PR-soc-chart / PR-soc-chart-segments: hand-rolled SVG renderer for
// the battery-SoC chart.
//
// Traces:
//   1. History           — recorded samples (last 48 h, every 15 min) as
//                          a solid polyline.
//   2. Now marker        — vertical dashed line at the current snapshot
//                          time.
//   3. Projection        — piecewise-linear extrapolation forward, one
//                          polyline per segment with class
//                          `trace-projection-<kind>`.
//   4. Reference targets — two horizontal lines drawn underneath the
//                          projection: discharge floor and charge ceiling.
//
// Interactivity: a mouse-/touch-driven hairline with a tooltip showing
// the time + SoC at that x-position. No charting library; everything is
// hand-written SVG so we don't ship any new TS deps.

import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";

type HistorySample = { epoch_ms: number; soc_pct: number };

type SegmentKind =
  | "Natural"
  | "Idle"
  | "ScheduledCharge"
  | "FullChargePush"
  | "Clamped"
  // PR-soc-chart-solar.
  | "SolarCharge"
  | "Drain";

type ProjectionSegment = {
  start_epoch_ms: number;
  end_epoch_ms: number;
  start_soc_pct: number;
  end_soc_pct: number;
  kind: SegmentKind;
};

type Projection = {
  segments: ProjectionSegment[];
  net_power_w: number | null;
  capacity_wh: number | null;
  charge_rate_w: number | null;
};

type SocChart = {
  history: HistorySample[];
  projection: Projection;
  now_epoch_ms: number;
  now_soc_pct: number | null;
  discharge_target_pct: number | null;
  charge_target_pct: number | null;
};

// SVG layout. We measure the host container's width at render time and
// build a viewBox that matches its actual pixel dimensions (1 SVG
// unit = 1 px). This avoids the `preserveAspectRatio="none"` stretch
// that would distort circles into ellipses on wide containers. Height
// stays fixed at 220 px.
const VB_W_FALLBACK = 800;
const VB_H = 220;
const PAD_L = 40;
const PAD_R = 8;
const PAD_T = 8;
const PAD_B = 30;
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

const VALID_KINDS: ReadonlySet<string> = new Set([
  "Natural",
  "Idle",
  "ScheduledCharge",
  "FullChargePush",
  "Clamped",
  // PR-soc-chart-solar.
  "SolarCharge",
  "Drain",
]);

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

function asKind(v: unknown): SegmentKind | null {
  if (typeof v !== "string") return null;
  return VALID_KINDS.has(v) ? (v as SegmentKind) : null;
}

function readChart(snap: WorldSnapshot): SocChart | null {
  const raw = (snap as unknown as { soc_chart?: unknown }).soc_chart;
  if (!raw || typeof raw !== "object") return null;
  const c = raw as Record<string, unknown>;
  const history = (c.history as Array<{ epoch_ms: unknown; soc_pct: unknown }>) ?? [];
  const projRaw = (c.projection as Record<string, unknown>) ?? {};
  const segmentsRaw = (projRaw.segments as Array<Record<string, unknown>>) ?? [];
  const segments: ProjectionSegment[] = [];
  for (const sRaw of segmentsRaw) {
    const start_epoch_ms = asNum(sRaw.start_epoch_ms);
    const end_epoch_ms = asNum(sRaw.end_epoch_ms);
    const start_soc_pct = asNum(sRaw.start_soc_pct);
    const end_soc_pct = asNum(sRaw.end_soc_pct);
    const kind = asKind(sRaw.kind);
    if (
      start_epoch_ms === null ||
      end_epoch_ms === null ||
      start_soc_pct === null ||
      end_soc_pct === null ||
      kind === null
    ) {
      continue;
    }
    segments.push({ start_epoch_ms, end_epoch_ms, start_soc_pct, end_soc_pct, kind });
  }
  const proj: Projection = {
    segments,
    net_power_w: asNum(projRaw.net_power_w),
    capacity_wh: asNum(projRaw.capacity_wh),
    charge_rate_w: asNum(projRaw.charge_rate_w),
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
    discharge_target_pct: asNum(c.discharge_target_pct),
    charge_target_pct: asNum(c.charge_target_pct),
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

function xToSvg(epochMs: number, x0: number, x1: number, plotW: number): number {
  if (x1 === x0) return PAD_L;
  const t = (epochMs - x0) / (x1 - x0);
  return PAD_L + Math.max(0, Math.min(1, t)) * plotW;
}

function yToSvg(soc: number): number {
  // Y axis is 0..100; SVG y grows downward.
  const t = Math.max(0, Math.min(100, soc)) / 100;
  return PAD_T + (1 - t) * PLOT_H;
}

function svgFromX(svgX: number, x0: number, x1: number, plotW: number): number {
  // Inverse of xToSvg — clamp to the plot rect.
  const cx = Math.max(PAD_L, Math.min(PAD_L + plotW, svgX));
  const t = (cx - PAD_L) / plotW;
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

// Find the projection segment containing `epochMs`. Returns null if no
// segment matches (e.g. before the first segment or after the last).
function segmentAt(
  segments: ProjectionSegment[],
  epochMs: number,
): ProjectionSegment | null {
  for (const s of segments) {
    if (epochMs >= s.start_epoch_ms && epochMs <= s.end_epoch_ms) return s;
  }
  return null;
}

// Linearly interpolate SoC inside a segment.
function interpolateSegment(seg: ProjectionSegment, epochMs: number): number {
  const span = seg.end_epoch_ms - seg.start_epoch_ms;
  if (span <= 0) return seg.start_soc_pct;
  const t = (epochMs - seg.start_epoch_ms) / span;
  return seg.start_soc_pct + (seg.end_soc_pct - seg.start_soc_pct) * t;
}

const KIND_SUFFIX: Record<SegmentKind, string> = {
  Natural: "natural",
  Idle: "idle",
  ScheduledCharge: "scheduled charge",
  FullChargePush: "full-charge push",
  Clamped: "clamped",
  // PR-soc-chart-solar.
  SolarCharge: "solar charge",
  Drain: "drain",
};

const KIND_CSS: Record<SegmentKind, string> = {
  Natural: "trace-projection-natural",
  Idle: "trace-projection-idle",
  ScheduledCharge: "trace-projection-scheduledcharge",
  FullChargePush: "trace-projection-fullchargepush",
  Clamped: "trace-projection-clamped",
  // PR-soc-chart-solar.
  SolarCharge: "trace-projection-solarcharge",
  Drain: "trace-projection-drain",
};

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

  // Measure host width so the viewBox matches actual rendered pixels.
  // 1 SVG unit = 1 px → circles stay round and aspect doesn't distort
  // when the container is wider than the historical 800-unit fallback.
  // Use ResizeObserver below to re-render on container resize.
  const vbW = Math.max(320, Math.round(host.clientWidth || VB_W_FALLBACK));
  const plotW = vbW - PAD_L - PAD_R;

  const nowMs = chart.now_epoch_ms;
  const segments = chart.projection.segments;

  // X domain. When history is empty (boot state), fall back to a
  // 13-hour window centered on `now`. Right edge stretches to the end
  // of the last projection segment (or now+12h if there's nothing).
  const firstHistMs =
    chart.history.length > 0 ? chart.history[0].epoch_ms : nowMs - 1 * HOUR_MS;
  let rightMs = nowMs + HOUR_MS;
  if (segments.length > 0) {
    const lastSeg = segments[segments.length - 1];
    if (lastSeg.end_epoch_ms > rightMs) rightMs = lastSeg.end_epoch_ms;
  } else {
    rightMs = Math.max(rightMs, nowMs + DEFAULT_PROJECTION_HORIZON_MS);
  }
  const x0 = Math.min(firstHistMs, nowMs - HOUR_MS);
  const x1 = Math.max(rightMs, nowMs + HOUR_MS);

  // --- assemble SVG --------------------------------------------------
  const parts: string[] = [];
  parts.push(
    `<svg viewBox="0 0 ${vbW} ${VB_H}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Battery SoC history and projection">`,
  );

  // Plot rect (transparent — gives the hairline a hit area).
  parts.push(
    `<rect class="plot-bg" x="${PAD_L}" y="${PAD_T}" width="${plotW}" height="${PLOT_H}" fill="transparent" />`,
  );

  // Y gridlines at 0, 25, 50, 75, 100.
  const yTicks = [0, 25, 50, 75, 100];
  for (const v of yTicks) {
    const y = yToSvg(v);
    parts.push(
      `<line class="axis-grid" x1="${PAD_L}" y1="${y}" x2="${PAD_L + plotW}" y2="${y}" />`,
    );
    parts.push(
      `<text class="axis-label" x="${PAD_L - 4}" y="${y + 3}" text-anchor="end">${v}%</text>`,
    );
  }

  // X gridlines + labels at adaptive step. Align to local-clock
  // boundaries so labels sit on round HH:MM values.
  const step = pickXStep(x1 - x0);
  const midnight = new Date(x0);
  midnight.setHours(0, 0, 0, 0);
  const startTick = midnight.getTime() + Math.ceil((x0 - midnight.getTime()) / step) * step;
  for (let t = startTick; t <= x1; t += step) {
    const x = xToSvg(t, x0, x1, plotW);
    parts.push(
      `<line class="axis-grid" x1="${x}" y1="${PAD_T}" x2="${x}" y2="${PAD_T + PLOT_H}" />`,
    );
    parts.push(
      `<text class="axis-label" x="${x}" y="${PAD_T + PLOT_H + 14}" text-anchor="middle">${fmtClock(t)}</text>`,
    );
  }

  // Reference target lines — drawn before the projection so projection
  // segments overlay on top. Skipped when null.
  const drawTargetLine = (
    pct: number,
    cssClass: string,
    labelPrefix: string,
  ): void => {
    const y = yToSvg(pct);
    parts.push(
      `<line class="${cssClass}" x1="${PAD_L}" y1="${y}" x2="${PAD_L + plotW}" y2="${y}" />`,
    );
    parts.push(
      `<text class="target-label" x="${PAD_L + plotW - 4}" y="${y - 3}" text-anchor="end">${labelPrefix} ${pct.toFixed(0)}%</text>`,
    );
  };
  if (chart.discharge_target_pct !== null) {
    drawTargetLine(chart.discharge_target_pct, "target-line-discharge", "discharge");
  }
  if (chart.charge_target_pct !== null) {
    drawTargetLine(chart.charge_target_pct, "target-line-charge", "charge");
  }

  // History polyline.
  if (chart.history.length >= 2) {
    const points = chart.history
      .map((s) => `${xToSvg(s.epoch_ms, x0, x1, plotW).toFixed(2)},${yToSvg(s.soc_pct).toFixed(2)}`)
      .join(" ");
    parts.push(`<polyline class="trace-history" points="${points}" />`);
  } else if (chart.history.length === 1) {
    const s = chart.history[0];
    parts.push(
      `<circle class="trace-history" cx="${xToSvg(s.epoch_ms, x0, x1, plotW)}" cy="${yToSvg(s.soc_pct)}" r="2" />`,
    );
  }

  // Empty-history note.
  if (chart.history.length === 0) {
    parts.push(
      `<text class="terminus-label" x="${PAD_L + plotW / 2}" y="${PAD_T + PLOT_H / 2}" text-anchor="middle">no history yet</text>`,
    );
  }

  // Projection segments — one polyline per segment so CSS can style
  // each `kind` independently.
  for (const seg of segments) {
    const xa = xToSvg(seg.start_epoch_ms, x0, x1, plotW);
    const ya = yToSvg(seg.start_soc_pct);
    const xb = xToSvg(seg.end_epoch_ms, x0, x1, plotW);
    const yb = yToSvg(seg.end_soc_pct);
    parts.push(
      `<polyline class="${KIND_CSS[seg.kind]}" data-kind="${escAttr(seg.kind)}" points="${xa.toFixed(2)},${ya.toFixed(2)} ${xb.toFixed(2)},${yb.toFixed(2)}" />`,
    );
  }

  // Terminus annotation on the LAST segment (the one that ends at the
  // chart's right edge or terminates in a Clamped tail). Skip when no
  // segments.
  if (segments.length > 0) {
    const last = segments[segments.length - 1];
    const xb = xToSvg(last.end_epoch_ms, x0, x1, plotW);
    const yb = yToSvg(last.end_soc_pct);
    const annotX = Math.min(xb + 4, PAD_L + plotW - 60);
    const labelPct = `${last.end_soc_pct.toFixed(0)}%`;
    parts.push(
      `<text class="terminus-label" x="${annotX}" y="${Math.max(PAD_T + 12, yb - 4)}" text-anchor="start">→ ${labelPct} at ${fmtClock(last.end_epoch_ms)}</text>`,
    );
  } else if (chart.now_soc_pct !== null) {
    // No projection — emit an "idle" marker at the now SoC.
    const yFlat = yToSvg(chart.now_soc_pct);
    const xNow = xToSvg(nowMs, x0, x1, plotW);
    parts.push(
      `<text class="terminus-label" x="${Math.min(xNow + 8, PAD_L + plotW - 30)}" y="${Math.max(PAD_T + 12, yFlat - 4)}" text-anchor="start">no projection</text>`,
    );
  }

  // Now marker (drawn last over the projection start so it's visible).
  const xNow = xToSvg(nowMs, x0, x1, plotW);
  parts.push(
    `<line class="now-marker" x1="${xNow}" y1="${PAD_T}" x2="${xNow}" y2="${PAD_T + PLOT_H}" />`,
  );

  // Hairline + tooltip placeholders, hidden until mousemove.
  parts.push(
    `<line class="hairline" id="soc-chart-hairline" x1="0" y1="${PAD_T}" x2="0" y2="${PAD_T + PLOT_H}" style="display:none" />`,
  );
  parts.push(
    `<text class="hairline-label" id="soc-chart-hairline-label" x="${PAD_L + plotW - 4}" y="${PAD_T + 12}" text-anchor="end" style="display:none"></text>`,
  );

  parts.push(`</svg>`);
  host.innerHTML = parts.join("");

  // Wire mouse + touch handlers. The SVG is fully replaced on every
  // applySnapshot, so we re-attach each render.
  installHairlineHandlers(host, chart, x0, x1, vbW, plotW);
  // Re-render on container resize so the viewBox tracks actual width.
  // `applySnapshot` re-renders us anyway every snapshot tick, so we
  // only need to handle the one-shot resize between ticks.
  installResizeObserver(host, snap);
}

function installHairlineHandlers(
  host: HTMLElement,
  chart: SocChart,
  x0: number,
  x1: number,
  vbW: number,
  plotW: number,
): void {
  const svg = host.querySelector("svg");
  if (!svg) return;
  const hairline = svg.querySelector<SVGLineElement>("#soc-chart-hairline");
  const label = svg.querySelector<SVGTextElement>("#soc-chart-hairline-label");
  if (!hairline || !label) return;

  const onMove = (clientX: number) => {
    const rect = svg.getBoundingClientRect();
    if (rect.width === 0) return;
    const svgX = ((clientX - rect.left) / rect.width) * vbW;
    if (svgX < PAD_L || svgX > PAD_L + plotW) {
      hairline.style.display = "none";
      label.style.display = "none";
      return;
    }
    const epochMs = svgFromX(svgX, x0, x1, plotW);
    let soc: number | null = null;
    let kindSuffix = "";
    if (epochMs <= chart.now_epoch_ms) {
      const nearest = nearestHistory(chart.history, epochMs);
      soc = nearest ? nearest.soc_pct : null;
    } else {
      const seg = segmentAt(chart.projection.segments, epochMs);
      if (seg !== null) {
        soc = interpolateSegment(seg, epochMs);
        kindSuffix = ` (${KIND_SUFFIX[seg.kind]})`;
      }
    }
    hairline.setAttribute("x1", String(svgX));
    hairline.setAttribute("x2", String(svgX));
    hairline.style.display = "";
    const socText = soc === null ? "—" : `${soc.toFixed(1)}%`;
    label.textContent = `${fmtClockSec(epochMs)} — ${socText}${kindSuffix}`;
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
}

// Re-render on container resize so the viewBox tracks actual width.
// Snapshot ticks already re-render every ~1s, so this only matters for
// the moments between ticks (e.g. user rotating their phone or
// resizing the browser). We attach the observer once per host element
// and remember the last snapshot for redraws driven by the observer.
let resizeObserver: ResizeObserver | null = null;
let lastSnapshotForResize: WorldSnapshot | null = null;
let lastObservedWidth = 0;
function installResizeObserver(host: HTMLElement, snap: WorldSnapshot): void {
  lastSnapshotForResize = snap;
  if (resizeObserver !== null) return;
  if (typeof ResizeObserver === "undefined") return;
  resizeObserver = new ResizeObserver((entries) => {
    if (lastSnapshotForResize === null) return;
    for (const e of entries) {
      const w = Math.round(e.contentRect.width);
      if (Math.abs(w - lastObservedWidth) >= 4) {
        lastObservedWidth = w;
        renderSocChart(lastSnapshotForResize);
        return;
      }
    }
  });
  resizeObserver.observe(host);
}
