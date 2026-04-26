//! PR-pinned-registers: periodic re-reader for the configured set of
//! pinned D-Bus registers.
//!
//! Each entry in `[[dbus_pinned_registers]]` is read once an hour via
//! `com.victronenergy.BusItem.GetValue`, the typed value is converted
//! into a `PinnedValue`, and an `Event::PinnedRegisterReading` is sent
//! down the runtime's event channel. The core then handles drift
//! detection + corrective writes; this module is purely the read
//! side.
//!
//! Connect / SetValue use is not handled here — drift writes flow
//! through the shared `dbus::Writer` chokepoint via
//! `Effect::WriteDbusPinned` so the `[dbus] writes_enabled` gate
//! continues to work.

use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use zbus::zvariant::Value;
use zbus::{Connection, Proxy};

use victron_controller_core::types::{Event, PinnedValue};

use crate::config::{DbusPinnedRegister, PinnedType};

/// Pinned-register re-read cadence. Hard-coded per the PR-pinned-registers
/// spec — the registers in scope (PowerAssist enables, Hub4 mode,
/// MaxFeedInPower, …) only change on Victron firmware updates, so an
/// hourly cadence is plenty for "did the last firmware push wipe our
/// settings?" detection.
pub const PINNED_REGISTER_CHECK_PERIOD: Duration = Duration::from_secs(3600);

/// Per-call timeout. Same 2 s budget as the subscriber's GetItems
/// timeout — a healthy Venus replies in <50 ms; 2 s is 40× headroom.
const GET_VALUE_TIMEOUT: Duration = Duration::from_secs(2);

/// Initial delay before the first read pass. Lets the subscriber
/// connect + the world settle into a known state before we ship the
/// first hour of readings; also avoids a startup-time burst overlapping
/// the first knob-publish.
const INITIAL_DELAY: Duration = Duration::from_secs(60);

/// Spawn the pinned-register reader. Runs forever; reconnects D-Bus on
/// each pass so a transient bus hiccup doesn't wedge the loop.
pub async fn run(
    registers: Vec<DbusPinnedRegister>,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    if registers.is_empty() {
        debug!("pinned-register reader: no [[dbus_pinned_registers]] configured; idle");
        // Park forever so the spawned task cleanly exits when the
        // event channel closes.
        std::future::pending::<()>().await;
        return Ok(());
    }
    info!(
        count = registers.len(),
        period_s = PINNED_REGISTER_CHECK_PERIOD.as_secs(),
        "pinned-register reader started"
    );

    tokio::time::sleep(INITIAL_DELAY).await;
    loop {
        match read_pass(&registers, &tx).await {
            Ok(()) => {}
            Err(e) => {
                warn!(error = %format!("{e:#}"), "pinned-register reader pass failed");
            }
        }
        if tx.is_closed() {
            return Ok(());
        }
        tokio::time::sleep(PINNED_REGISTER_CHECK_PERIOD).await;
    }
}

/// One read pass: connect, GetValue every register, emit events.
async fn read_pass(
    registers: &[DbusPinnedRegister],
    tx: &mpsc::Sender<Event>,
) -> Result<()> {
    let conn = Connection::system()
        .await
        .context("connecting to system D-Bus for pinned-register read")?;
    for entry in registers {
        let (service, path) = entry.split_path();
        match read_one(&conn, service, path, entry.value_type).await {
            Ok(value) => {
                let at = chrono::Local::now().naive_local();
                if tx
                    .send(Event::PinnedRegisterReading {
                        path: entry.path.clone(),
                        value,
                        at,
                    })
                    .await
                    .is_err()
                {
                    // Receiver gone — bail out cleanly so the spawn finishes.
                    return Ok(());
                }
            }
            Err(e) => {
                warn!(
                    path = %entry.path,
                    error = %format!("{e:#}"),
                    "pinned-register GetValue failed"
                );
            }
        }
    }
    Ok(())
}

async fn read_one(
    conn: &Connection,
    service: &str,
    path: &str,
    value_type: PinnedType,
) -> Result<PinnedValue> {
    let proxy = Proxy::new(conn, service, path, "com.victronenergy.BusItem")
        .await
        .context("building GetValue proxy")?;
    let v: zbus::zvariant::OwnedValue = tokio::time::timeout(
        GET_VALUE_TIMEOUT,
        proxy.call("GetValue", &()),
    )
    .await
    .with_context(|| format!("GetValue timed out on {service}{path}"))?
    .with_context(|| format!("GetValue call on {service}{path}"))?;
    extract_pinned(&v, value_type)
        .with_context(|| format!("decoding GetValue reply for {service}{path}"))
}

/// Coerce a `zvariant::Value` into the configured `PinnedType`. Bool /
/// Int / Float reads accept the wide range of integer widths Victron
/// emits (the `extract_scalar` precedent in `subscriber.rs` documents
/// the same set). Returns an error for unexpected shapes — the caller
/// logs and skips that register's reading.
fn extract_pinned(v: &Value<'_>, value_type: PinnedType) -> Result<PinnedValue> {
    match value_type {
        PinnedType::Bool => match v {
            Value::Bool(b) => Ok(PinnedValue::Bool(*b)),
            // Victron's settings service returns `Int(0/1)` even when
            // the user wrote a Python boolean — accept that as the
            // bool-typed reading.
            Value::I32(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::U32(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::I16(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::U16(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::U8(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::I64(n) => Ok(PinnedValue::Int(*n)),
            other => Err(anyhow::anyhow!(
                "expected bool/int wire shape, got {other:?}"
            )),
        },
        PinnedType::Int => match v {
            Value::I32(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::U32(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::I16(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::U16(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::U8(n) => Ok(PinnedValue::Int(i64::from(*n))),
            Value::I64(n) => Ok(PinnedValue::Int(*n)),
            Value::U64(n) => i64::try_from(*n)
                .map(PinnedValue::Int)
                .map_err(|_| anyhow::anyhow!("u64 value {n} does not fit i64")),
            Value::Bool(b) => Ok(PinnedValue::Int(i64::from(*b))),
            other => Err(anyhow::anyhow!(
                "expected int wire shape, got {other:?}"
            )),
        },
        PinnedType::Float => match v {
            Value::F64(f) if f.is_finite() => Ok(PinnedValue::Float(*f)),
            #[allow(clippy::cast_precision_loss)]
            Value::I64(n) => Ok(PinnedValue::Float(*n as f64)),
            #[allow(clippy::cast_precision_loss)]
            Value::U64(n) => Ok(PinnedValue::Float(*n as f64)),
            Value::I32(n) => Ok(PinnedValue::Float(f64::from(*n))),
            Value::U32(n) => Ok(PinnedValue::Float(f64::from(*n))),
            Value::I16(n) => Ok(PinnedValue::Float(f64::from(*n))),
            Value::U16(n) => Ok(PinnedValue::Float(f64::from(*n))),
            Value::U8(n) => Ok(PinnedValue::Float(f64::from(*n))),
            other => Err(anyhow::anyhow!(
                "expected float wire shape, got {other:?}"
            )),
        },
        PinnedType::String => match v {
            Value::Str(s) => Ok(PinnedValue::String(s.as_str().to_string())),
            other => Err(anyhow::anyhow!(
                "expected string wire shape, got {other:?}"
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pinned_bool_accepts_int_zero_one() {
        let v = Value::I32(1);
        // Victron returns int over the wire even for bool writes — the
        // subscriber accepts it as Int and `PinnedValue::approx_eq`
        // handles the bool↔int(0/1) coercion at compare time.
        let out = extract_pinned(&v, PinnedType::Bool).unwrap();
        assert_eq!(out, PinnedValue::Int(1));
    }

    #[test]
    fn extract_pinned_bool_accepts_native_bool() {
        let v = Value::Bool(true);
        let out = extract_pinned(&v, PinnedType::Bool).unwrap();
        assert_eq!(out, PinnedValue::Bool(true));
    }

    #[test]
    fn extract_pinned_float_rejects_nonfinite() {
        let v = Value::F64(f64::NAN);
        assert!(extract_pinned(&v, PinnedType::Float).is_err());
    }

    #[test]
    fn extract_pinned_int_widens_smaller_widths() {
        let v = Value::U16(5000);
        let out = extract_pinned(&v, PinnedType::Int).unwrap();
        assert_eq!(out, PinnedValue::Int(5000));
    }

    #[test]
    fn extract_pinned_string_unwraps_str() {
        let v = Value::Str("foo".into());
        let out = extract_pinned(&v, PinnedType::String).unwrap();
        assert_eq!(out, PinnedValue::String("foo".to_string()));
    }
}
