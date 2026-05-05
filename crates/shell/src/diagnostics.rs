//! PR-DIAG-1: process + host memory diagnostics.
//!
//! Two responsibilities:
//!
//! 1. Sample `/proc/self/status`, `/proc/meminfo`, jemalloc stats, and
//!    process uptime on a fixed cadence.
//! 2. Hand the sampled values out to (a) the dashboard convert path, so
//!    they appear in the WorldSnapshot's `diagnostics` block, and (b)
//!    the MQTT publisher in `main.rs`, which republishes them on the
//!    same cadence as a separate set of `controller.*` topics.
//!
//! Sampler runs in its own task — never blocks the world-tick loop. A
//! stuck `/proc` read at most freezes diagnostics; world state and
//! actuation continue.

use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::warn;

/// Snapshot of all diagnostics fields. Cheap to clone (10 × i64).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub process_uptime_s: i64,
    pub process_rss_bytes: i64,
    pub process_vm_hwm_bytes: i64,
    pub process_vm_size_bytes: i64,
    pub jemalloc_allocated_bytes: i64,
    pub jemalloc_resident_bytes: i64,
    pub host_mem_total_bytes: i64,
    pub host_mem_available_bytes: i64,
    pub host_swap_used_bytes: i64,
    pub sampled_at_epoch_ms: i64,
}

/// Shared handle written by the sampler task and read by the dashboard
/// convert layer + MQTT publisher. `Mutex` is fine — readers and the
/// sampler each hold it for microseconds.
#[derive(Debug, Clone)]
pub struct DiagnosticsHandle {
    inner: Arc<Mutex<Diagnostics>>,
}

impl Default for DiagnosticsHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticsHandle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Diagnostics::default())),
        }
    }

    /// Latest sampled values. Returns the zero-initialised default
    /// before the sampler has run for the first time.
    #[must_use]
    pub fn snapshot(&self) -> Diagnostics {
        // poisoned-lock recovery: the sampler is the only writer and
        // panics in it would mean a bigger problem; falling back to
        // the inner value keeps the dashboard snapshot path going.
        match self.inner.lock() {
            Ok(g) => *g,
            Err(p) => *p.into_inner(),
        }
    }

    fn store(&self, d: Diagnostics) {
        match self.inner.lock() {
            Ok(mut g) => *g = d,
            Err(p) => *p.into_inner() = d,
        }
    }
}

/// Cadence at which `spawn_diagnostics_sampler` refreshes the handle.
/// Memory metrics drift slowly; 60 s gives one sample per minute, which
/// is plenty for a leak-hunt time series and keeps `/proc` reads off
/// the hot path.
pub const DIAGNOSTICS_SAMPLE_PERIOD: Duration = Duration::from_secs(60);

/// Spawn the sampler task. `process_start` is captured by `main` as the
/// first statement so uptime tracks "process running" time, not
/// "diagnostics initialised" time.
pub fn spawn_diagnostics_sampler(
    handle: DiagnosticsHandle,
    process_start: Instant,
) {
    // jemalloc stats are gated behind an "epoch" — readings are only
    // refreshed when `epoch.advance()` is called. Resolve the MIBs
    // once outside the loop; every sample reuses them.
    let jemalloc = JemallocReader::resolve();

    tokio::spawn(async move {
        // Take the first sample immediately so the dashboard's
        // Diagnostics group isn't blank for the first minute.
        let first = collect(process_start, jemalloc.as_ref());
        handle.store(first);

        let mut interval = tokio::time::interval(DIAGNOSTICS_SAMPLE_PERIOD);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Discard the immediate first tick — we already sampled above.
        interval.tick().await;

        loop {
            interval.tick().await;
            let d = collect(process_start, jemalloc.as_ref());
            handle.store(d);
        }
    });
}

fn collect(process_start: Instant, jemalloc: Option<&JemallocReader>) -> Diagnostics {
    let now_epoch_ms = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis()),
    )
    .unwrap_or(i64::MAX);

    let uptime_s = i64::try_from(process_start.elapsed().as_secs()).unwrap_or(i64::MAX);

    let proc_status = read_proc_self_status();
    let meminfo = read_proc_meminfo();
    let (alloc, resident) = jemalloc.map_or((0, 0), JemallocReader::sample);

    Diagnostics {
        process_uptime_s: uptime_s,
        process_rss_bytes: proc_status.vm_rss_bytes,
        process_vm_hwm_bytes: proc_status.vm_hwm_bytes,
        process_vm_size_bytes: proc_status.vm_size_bytes,
        jemalloc_allocated_bytes: i64::try_from(alloc).unwrap_or(i64::MAX),
        jemalloc_resident_bytes: i64::try_from(resident).unwrap_or(i64::MAX),
        host_mem_total_bytes: meminfo.mem_total_bytes,
        host_mem_available_bytes: meminfo.mem_available_bytes,
        host_swap_used_bytes: meminfo.swap_used_bytes,
        sampled_at_epoch_ms: now_epoch_ms,
    }
}

#[derive(Debug, Default)]
#[allow(clippy::struct_field_names)]
struct ProcStatus {
    vm_rss_bytes: i64,
    vm_hwm_bytes: i64,
    vm_size_bytes: i64,
}

fn read_proc_self_status() -> ProcStatus {
    let raw = match fs::read_to_string("/proc/self/status") {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "diagnostics: /proc/self/status read failed");
            return ProcStatus::default();
        }
    };
    let mut out = ProcStatus::default();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            out.vm_rss_bytes = parse_kb_line(rest);
        } else if let Some(rest) = line.strip_prefix("VmHWM:") {
            out.vm_hwm_bytes = parse_kb_line(rest);
        } else if let Some(rest) = line.strip_prefix("VmSize:") {
            out.vm_size_bytes = parse_kb_line(rest);
        }
    }
    out
}

#[derive(Debug, Default)]
#[allow(clippy::struct_field_names)]
struct MemInfo {
    mem_total_bytes: i64,
    mem_available_bytes: i64,
    swap_used_bytes: i64,
}

fn read_proc_meminfo() -> MemInfo {
    let raw = match fs::read_to_string("/proc/meminfo") {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "diagnostics: /proc/meminfo read failed");
            return MemInfo::default();
        }
    };
    let mut total = 0_i64;
    let mut available = 0_i64;
    let mut swap_total = 0_i64;
    let mut swap_free = 0_i64;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = parse_kb_line(rest);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = parse_kb_line(rest);
        } else if let Some(rest) = line.strip_prefix("SwapTotal:") {
            swap_total = parse_kb_line(rest);
        } else if let Some(rest) = line.strip_prefix("SwapFree:") {
            swap_free = parse_kb_line(rest);
        }
    }
    MemInfo {
        mem_total_bytes: total,
        mem_available_bytes: available,
        // SwapTotal == 0 on swap-less hosts; subtraction stays 0.
        swap_used_bytes: (swap_total - swap_free).max(0),
    }
}

/// Parse the "<whitespace><number> kB" tail of a /proc status line and
/// convert kilobytes to bytes. Returns 0 on any parse failure — these
/// reads are observability-only, not control-path.
fn parse_kb_line(rest: &str) -> i64 {
    let trimmed = rest.trim();
    let num_str = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("");
    num_str
        .parse::<i64>()
        .ok()
        .and_then(|kb| kb.checked_mul(1024))
        .unwrap_or(0)
}

/// jemalloc stats reader. Resolving MIBs is a one-shot lookup; reads
/// after that are zero-allocation. Returns None if jemalloc isn't the
/// global allocator (e.g. on test runs that link only the lib, not
/// `main.rs`).
struct JemallocReader {
    epoch: tikv_jemalloc_ctl::epoch_mib,
    allocated: tikv_jemalloc_ctl::stats::allocated_mib,
    resident: tikv_jemalloc_ctl::stats::resident_mib,
}

impl JemallocReader {
    fn resolve() -> Option<Self> {
        let epoch = tikv_jemalloc_ctl::epoch::mib().ok()?;
        let allocated = tikv_jemalloc_ctl::stats::allocated::mib().ok()?;
        let resident = tikv_jemalloc_ctl::stats::resident::mib().ok()?;
        Some(Self {
            epoch,
            allocated,
            resident,
        })
    }

    fn sample(&self) -> (usize, usize) {
        // `epoch.advance()` refreshes the cached stats; without it
        // every read returns the value at the previous advance.
        let _ = self.epoch.advance();
        let a = self.allocated.read().unwrap_or(0);
        let r = self.resident.read().unwrap_or(0);
        (a, r)
    }
}

impl std::fmt::Debug for JemallocReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JemallocReader").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kb_line_handles_leading_whitespace_and_unit_suffix() {
        assert_eq!(parse_kb_line("\t  1024 kB"), 1024 * 1024);
        assert_eq!(parse_kb_line("  0 kB"), 0);
        // No unit, just a number — still parses.
        assert_eq!(parse_kb_line("  4096"), 4096 * 1024);
        // Garbage → 0 (observability-only path; no panics).
        assert_eq!(parse_kb_line("  not-a-number"), 0);
        assert_eq!(parse_kb_line(""), 0);
    }

    #[test]
    fn read_proc_self_status_returns_nonzero_on_linux() {
        // Sanity check on the host running the test — every Linux
        // process has nonzero VmRSS / VmSize.
        let s = read_proc_self_status();
        assert!(s.vm_rss_bytes > 0, "VmRSS should be > 0 for self");
        assert!(s.vm_size_bytes > 0, "VmSize should be > 0 for self");
        assert!(s.vm_hwm_bytes >= s.vm_rss_bytes,
            "VmHWM should be >= VmRSS (peak >= current)");
    }

    #[test]
    fn read_proc_meminfo_returns_total_and_available() {
        let m = read_proc_meminfo();
        assert!(m.mem_total_bytes > 0, "MemTotal should be > 0");
        assert!(m.mem_available_bytes > 0, "MemAvailable should be > 0");
        assert!(m.mem_available_bytes <= m.mem_total_bytes,
            "MemAvailable <= MemTotal");
        assert!(m.swap_used_bytes >= 0);
    }
}
