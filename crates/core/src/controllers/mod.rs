//! Pure controllers — one per Node-RED tab. Each is a side-effect-free
//! function over inputs + [`crate::Clock`], producing a decision.
//!
//! The higher-level `process()` loop (future module) wires these into the
//! [`crate::World`] and turns their decisions into `Effect`s.

pub mod current_limit;
pub mod schedules;
pub mod setpoint;
pub mod tariff_band;

pub use tariff_band::{TariffBand, TariffBandKind, TariffBandSubKind, tariff_band};
