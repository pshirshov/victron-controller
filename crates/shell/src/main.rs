//! victron-controller binary entry point.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::signal;
use tokio::sync::mpsc;
use tracing::{error, info};

use victron_controller_core::Topology;
use victron_controller_shell::clock::RealClock;
use victron_controller_shell::config::{self, Config, DbusServices};
use victron_controller_shell::dbus::{Subscriber, Writer};
use victron_controller_shell::runtime::Runtime;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    init_tracing();
    let cfg_path = config_path_from_args();
    info!("loading config: {}", cfg_path.display());
    let cfg: Config = config::load(&cfg_path).with_context(|| "load config")?;

    let services = cfg
        .dbus
        .services
        .clone()
        .unwrap_or_else(DbusServices::default_venus_3_70);

    // D-Bus subscriber → event channel → runtime → D-Bus writer.
    let (tx, rx) = mpsc::channel(256);

    info!("connecting D-Bus subscriber");
    let subscriber = Subscriber::connect(&services)
        .await
        .context("connect D-Bus subscriber")?;

    info!("connecting D-Bus writer (dry_run={})", !cfg.dbus.writes_enabled);
    let writer = Writer::connect(services, !cfg.dbus.writes_enabled)
        .await
        .context("connect D-Bus writer")?;

    let topology = Topology::defaults();
    let runtime = Runtime::new(writer, topology, Instant::now());

    // Spawn subscriber + runtime; they are linked via `tx`/`rx` so when
    // either ends the other naturally stops.
    let tx_for_sub = tx.clone();
    let subscriber_task = tokio::spawn(async move {
        if let Err(e) = subscriber.run(tx_for_sub).await {
            error!(error = %e, "subscriber terminated with error");
        }
    });
    drop(tx); // runtime owns no Sender → rx.recv() returns None after subscriber ends

    let tick_period = cfg.tuning.tick_period;
    let runtime_task = tokio::spawn(async move {
        if let Err(e) = runtime.run(rx, tick_period).await {
            error!(error = %e, "runtime terminated with error");
        }
    });

    // Wait for Ctrl-C or either task finishing.
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("received Ctrl-C; shutting down");
        }
        _ = subscriber_task => {
            info!("subscriber task ended");
        }
        _ = runtime_task => {
            info!("runtime task ended");
        }
    }

    // `RealClock` is Copy; just reference to silence "unused import"
    // in the crate. (The runtime already has its own copy.)
    let _ = RealClock;

    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(true)
        .init();
}

fn config_path_from_args() -> PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--config" {
            if let Some(p) = args.next() {
                return PathBuf::from(p);
            }
        }
    }
    // Default: the SPEC §10.1 location. Harmless if absent — load()
    // returns Config::default() in that case.
    PathBuf::from("/data/etc/victron-controller/config.toml")
}
