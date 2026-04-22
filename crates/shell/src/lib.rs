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
pub mod forecast;
pub mod mqtt;
pub mod myenergi;
pub mod runtime;
