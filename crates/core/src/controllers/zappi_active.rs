//! Single canonical classifier for "is the Zappi actively charging?".
//!
//! PR-DAG-B: invoked by `ZappiActiveCore` once per tick, which writes
//! `world.derived.zappi_active`. Every consumer (`run_setpoint`,
//! `run_current_limit`, `run_schedules`) reads that field — the two
//! controllers cannot disagree for the same tick.
//!
//! Semantics: match the legacy current-limit classifier 1:1 for the
//! state-machine terms (mode / plug / status / waiting-for-EV timeout),
//! and switch the fallback from `zappi_amps > 1 A` to
//! `evcharger_ac_power > 500 W` per SPEC §5.8 (§5.11 fallback on stale
//! myenergi data). The amps-based fallback false-fired on Hoymiles
//! export through the EV branch (~12 A at 2.8 kW).
//!
//! Either input (typed zappi state / evcharger AC power) may be
//! `Stale` / `Unknown`. When both are unusable the classifier returns
//! `false` — no evidence of active charging — matching the
//! current-limit controller's freshness-gate conservative default.

use crate::Clock;
use crate::myenergi::{ZappiMode, ZappiPlugState, ZappiStatus};
use crate::world::World;

/// How long `WaitingForEv` must have persisted before we treat the
/// Zappi as effectively inactive (the car isn't drawing, just parked).
const WAIT_TIMEOUT_MIN: f64 = 5.0;

/// SPEC §5.8: fallback threshold for deciding "active via power" when
/// the typed Zappi state doesn't decisively flag charging. Replaces the
/// legacy 1 A / ~230 W fallback that false-fired on Hoymiles exports.
const ZAPPI_POWER_FALLBACK_W: f64 = 500.0;

/// Classify the Zappi as actively drawing power. Both inputs may be
/// unusable; the result is conservatively `false` in that case.
#[must_use]
pub fn classify_zappi_active(world: &World, clock: &dyn Clock) -> bool {
    let typed = &world.typed_sensors.zappi_state;
    let evpow = &world.sensors.evcharger_ac_power;

    // Power-based fallback: SPEC §5.8. Valid classifier on its own when
    // power is fresh.
    let power_active = evpow.is_usable()
        && evpow.value.is_some_and(|w| w > ZAPPI_POWER_FALLBACK_W);

    let Some(state) = typed.value.filter(|_| typed.is_usable()) else {
        // Typed state not usable — use the power-based fallback alone.
        return power_active;
    };

    // Explicitly inactive modes.
    if matches!(state.zappi_mode, ZappiMode::Off) {
        return false;
    }

    // Plug not connected at all — can't be charging.
    if matches!(
        state.zappi_plug_state,
        ZappiPlugState::EvDisconnected | ZappiPlugState::Fault
    ) {
        return false;
    }

    // `ZappiStatus::Complete` — the car has finished charging for this
    // session. The legacy classifier treats this as inactive even while
    // the plug remains connected.
    if matches!(state.zappi_status, ZappiStatus::Complete) {
        return false;
    }

    // WAIT_TIMEOUT_MIN: if `WaitingForEv` has persisted beyond the
    // timeout, the car isn't drawing — treat as inactive. A-04 / A-24:
    // `zappi_last_change_signature` is a monotonic `Instant` stamped by
    // the poller when the `(zmo, sta, pst)` tuple flipped, so this delta
    // is immune to DST and to the local-vs-UTC mix that previously
    // fired the timeout on every invocation in BST.
    if matches!(state.zappi_plug_state, ZappiPlugState::WaitingForEv) {
        let now = clock.monotonic();
        let delta_min =
            now.duration_since(state.zappi_last_change_signature).as_secs_f64() / 60.0;
        if delta_min > WAIT_TIMEOUT_MIN {
            return false;
        }
    }

    // Otherwise: active mode + plug connected + status not Complete +
    // within wait-timeout. Active.
    if matches!(
        state.zappi_mode,
        ZappiMode::Fast | ZappiMode::Eco | ZappiMode::EcoPlus
    ) {
        return true;
    }

    // Fall through — any edge case we didn't catch (e.g. mode defaults
    // to Off but shouldn't here, since we already handled it). Use
    // power fallback as a safety net.
    power_active
}
