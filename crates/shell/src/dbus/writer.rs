//! Executes `Effect::WriteDbus` effects.
//!
//! Each write is a `SetValue` call on `com.victronenergy.BusItem`.
//! Integer paths get signed `i32`, float paths get `f64`. Errors are
//! logged but not retried — the controller will re-propose on its
//! next tick.

use anyhow::{Context, Result};
use tracing::{debug, error, warn};
use zbus::zvariant::Value;
use zbus::{Connection, Proxy};

use victron_controller_core::types::{DbusTarget, DbusValue, ScheduleField};

use crate::config::DbusServices;

#[derive(Debug)]
pub struct Writer {
    conn: Connection,
    services: DbusServices,
    /// When false, writes are logged but not emitted. Honours the
    /// config-file `[dbus] writes_enabled` knob *in addition* to the
    /// runtime kill switch.
    dry_run: bool,
}

impl Writer {
    pub async fn connect(services: DbusServices, dry_run: bool) -> Result<Self> {
        let conn = Connection::system()
            .await
            .context("connecting to the system D-Bus")?;
        Ok(Self {
            conn,
            services,
            dry_run,
        })
    }

    pub async fn write(&self, target: DbusTarget, value: DbusValue) {
        let (svc, path) = match self.resolve(target) {
            Some(v) => v,
            None => {
                warn!(?target, "no resolved (service, path) for DbusTarget");
                return;
            }
        };
        if self.dry_run {
            debug!(%svc, %path, ?value, "DRY-RUN WriteDbus (dbus.writes_enabled=false)");
            return;
        }
        match self.set_value(&svc, &path, value).await {
            Ok(()) => debug!(%svc, %path, ?value, "WriteDbus ok"),
            Err(e) => error!(%svc, %path, ?value, error = %e, "WriteDbus failed"),
        }
    }

    fn resolve(&self, target: DbusTarget) -> Option<(String, String)> {
        let s = &self.services;
        match target {
            DbusTarget::GridSetpoint => Some((
                s.settings.clone(),
                "/Settings/CGwacs/AcPowerSetPoint".to_string(),
            )),
            DbusTarget::InputCurrentLimit => Some((
                s.vebus.clone(),
                "/Ac/In/1/CurrentLimit".to_string(),
            )),
            DbusTarget::Schedule { index, field } => {
                let field_name = match field {
                    ScheduleField::Start => "Start",
                    ScheduleField::Duration => "Duration",
                    ScheduleField::Soc => "Soc",
                    ScheduleField::Days => "Day",
                    ScheduleField::AllowDischarge => "AllowDischarge",
                };
                Some((
                    s.settings.clone(),
                    format!(
                        "/Settings/CGwacs/BatteryLife/Schedule/Charge/{index}/{field_name}"
                    ),
                ))
            }
        }
    }

    async fn set_value(&self, service: &str, path: &str, value: DbusValue) -> Result<()> {
        let proxy = Proxy::new(&self.conn, service, path, "com.victronenergy.BusItem")
            .await
            .context("building SetValue proxy")?;
        let v: Value<'_> = match value {
            DbusValue::Int(i) => Value::I32(i),
            DbusValue::Float(f) => Value::F64(f),
        };
        // SetValue returns an i32 status code; 0 = success.
        let status: i32 = proxy
            .call("SetValue", &(v,))
            .await
            .context("SetValue call")?;
        if status == 0 {
            Ok(())
        } else {
            Err(anyhow::anyhow!("SetValue returned status {status}"))
        }
    }
}
