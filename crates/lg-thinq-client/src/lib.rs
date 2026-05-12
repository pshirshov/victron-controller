//! LG ThinQ Connect client for the Victron controller's LG heat-pump
//! actuator (HM051M.U43 system_boiler).
//!
//! Scope: tightly bounded to what the controller needs — list devices,
//! fetch state, post control commands, and subscribe to AWS-IoT MQTT
//! push events. Region is configurable; only the four real EMEA/AIC/KIC
//! domain prefixes are baked in.
//!
//! Auth is Personal-Access-Token (Bearer). The MQTT push side requires
//! a client certificate that LG issues against a CSR — that flow is
//! handled by [`provision`] and the resulting bundle is cached on disk.

pub mod api;
pub mod auth;
pub mod cert;
pub mod error;
pub mod heat_pump;
pub mod mqtt;
pub mod provision;
pub mod region;

pub use api::ThinqApi;
pub use error::{Error, Result};
pub use heat_pump::{
    HeatPumpControl, HeatPumpState, HotWaterMode, OperationMode,
};
pub use provision::{CertBundle, provision};
pub use region::{Country, DomainPrefix};
