//! Shared types describing Zappi / Eddi state. Used by the current-limit
//! controller (consumes state) and by the zappi/eddi controllers
//! (produces targets).

use std::time::Instant;

/// Zappi charge-mode target or actual state. Matches the myenergi API
/// `zmo` field's four values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ZappiMode {
    Fast,
    Eco,
    EcoPlus,
    #[default]
    Off,
}

/// Zappi plug state as reported by myenergi `pst`. The legacy NR code
/// maps the raw single/double-char codes to the names below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZappiPlugState {
    /// `A` — no cable plugged in.
    EvDisconnected,
    /// `B1` — cable plugged but EV not presenting itself.
    EvConnected,
    /// `B2` — cable plugged, waiting for EV.
    WaitingForEv,
    /// `C1` — EV ready to charge.
    EvReadyToCharge,
    /// `C2` — actively charging.
    Charging,
    /// `F` — fault.
    Fault,
}

/// Zappi status as reported by myenergi `sta`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZappiStatus {
    Paused,
    DivertingOrCharging,
    Complete,
    /// Anything outside the known set — propagated through for diagnostics.
    Other(u8),
}

impl PartialEq<ZappiStatus> for u8 {
    fn eq(&self, other: &ZappiStatus) -> bool {
        match other {
            ZappiStatus::Paused => *self == 1,
            ZappiStatus::DivertingOrCharging => *self == 3,
            ZappiStatus::Complete => *self == 5,
            ZappiStatus::Other(code) => self == code,
        }
    }
}

/// Normalised Zappi state snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZappiState {
    pub zappi_mode: ZappiMode,
    pub zappi_plug_state: ZappiPlugState,
    pub zappi_status: ZappiStatus,
    /// Monotonic `Instant` at which the mode/plug/status signature last
    /// changed — used by current-limit to detect the "waiting 5+ min with
    /// no EV" timeout. Stamped by the shell-side poller when it observes
    /// a change in the `(zmo, sta, pst)` tuple. Monotonic (not wall-clock)
    /// so DST flips and tz mismatches (myenergi UTC vs local) cannot
    /// poison the delta — see defects A-04 and A-24.
    pub zappi_last_change_signature: Instant,
}

/// Eddi mode target or actual state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EddiMode {
    /// Normal diversion mode.
    Normal,
    /// Diversion suspended.
    #[default]
    Stopped,
}
