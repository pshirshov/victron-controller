//! Dashboard HTTP server: exposes a JSON snapshot of the `World` and
//! accepts typed commands from the browser-side UI.
//!
//! The wire format is defined in `models/dashboard.baboon`; generated
//! Rust structs live in the `victron-controller-dashboard-model` crate.
//! This module is the adapter: it maps `core::World` into the baboon
//! types, and maps `baboon::Command` back into `core::Event`s that
//! the runtime consumes.

pub mod convert;
pub mod server;

pub use server::DashboardServer;
