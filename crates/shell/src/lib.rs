//! Async shell for the victron-controller service.
//!
//! Layout:
//!
//! - [`config`] — TOML config loader.
//! - [`clock`] — real-clock implementation of [`victron_controller_core::Clock`].
//! - [`dbus`] — system-bus subscriber + writer.
//! - [`runtime`] — the event-loop that owns the [`World`] and calls
//!   [`process`] on each event.
//!
//! `main.rs` is the binary entry point; everything reusable lives here.

pub mod clock;
pub mod config;
pub mod dashboard;
pub mod dbus;
pub mod diagnostics;
pub mod forecast;
pub mod mqtt;
pub mod myenergi;
pub mod runtime;

/// Cadence at which `main.rs` republishes `controller.uptime-s`. Kept
/// in the crate root so HA discovery (`mqtt::discovery`) and the
/// publisher loop (`main.rs`) can't drift.
pub const APP_UPTIME_PUBLISH_PERIOD_S: u64 = 30;

/// HA `expire_after` for the uptime sensor. Three publish periods —
/// one missed tick is slack, two missed ticks marks the controller as
/// dead. Computed at build time so the discovery payload and the
/// publisher cadence stay in lockstep.
pub const APP_UPTIME_EXPIRE_AFTER_S: u64 = APP_UPTIME_PUBLISH_PERIOD_S * 3;
