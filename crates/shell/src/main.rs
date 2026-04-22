//! victron-controller binary entry point.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::signal;
use tokio::signal::unix::{signal as unix_signal, SignalKind};
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};

use victron_controller_core::world::World;
use victron_controller_core::Topology;
use victron_controller_shell::clock::RealClock;
use victron_controller_shell::config::{self, Config, DbusServices};
use victron_controller_shell::dashboard::DashboardServer;
use victron_controller_shell::dbus::{Subscriber, Writer};
use victron_controller_shell::forecast::{self, ForecastSolarClient, OpenMeteoClient, SolcastClient};
use victron_controller_shell::mqtt::{
    self, log_channel, publish_ha_discovery, spawn_log_publisher, MqttLogLayer,
};
use victron_controller_shell::myenergi::{Client as MyenergiClient, Poller as MyenergiPoller,
    Writer as MyenergiWriter};
use victron_controller_shell::runtime::Runtime;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    // Create the log channel FIRST, before `init_tracing` — the tracing
    // subscriber composes an MqttLogLayer around the sender end so
    // every log line forwards to the mpsc queue. The publisher task
    // is spawned later, after MQTT connects, to drain the receiver.
    let (log_tx, log_rx) = log_channel();
    init_tracing(log_tx);

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

    let myenergi_client = MyenergiClient::new(cfg.myenergi.clone());
    let myenergi_writer = MyenergiWriter::new(myenergi_client.clone());
    let myenergi_poller = MyenergiPoller::new(myenergi_client, cfg.myenergi.poll_period);

    // MQTT (optional; skipped when host is empty).
    let (mqtt_publisher, mqtt_subscriber) = match mqtt::connect(&cfg.mqtt).await? {
        Some((p, s)) => {
            info!("publishing HA discovery config");
            if let Err(e) = publish_ha_discovery(&p.client_handle(), &cfg.mqtt.topic_root).await {
                error!(error = %e, "HA discovery publish failed (non-fatal)");
            }
            // Drain the log channel onto MQTT from here on. Records
            // buffered during pre-connect init will be emitted first.
            spawn_log_publisher(log_rx, p.client_handle(), cfg.mqtt.topic_root.clone());
            info!("mqtt log publisher started");
            (Some(p), Some(s))
        }
        None => {
            // No MQTT → just drop log records (the stdout layer still
            // fires). Drain the receiver so try_send doesn't block.
            tokio::spawn(async move {
                let mut rx = log_rx;
                while rx.recv().await.is_some() { /* discard */ }
            });
            (None, None)
        }
    };

    let topology = Topology::defaults();
    let world = Arc::new(Mutex::new(World::fresh_boot(Instant::now())));
    let runtime = Runtime::new(world.clone(), writer, myenergi_writer, mqtt_publisher, topology);

    // Spawn subscriber + myenergi poller + runtime; all linked via
    // `tx`/`rx` so when the runtime's receiver closes, producers exit.
    let tx_for_sub = tx.clone();
    let subscriber_task = tokio::spawn(async move {
        if let Err(e) = subscriber.run(tx_for_sub).await {
            error!(error = %e, "subscriber terminated with error");
        }
    });

    let tx_for_my = tx.clone();
    let myenergi_task = tokio::spawn(async move {
        if let Err(e) = myenergi_poller.run(tx_for_my).await {
            error!(error = %e, "myenergi poller terminated with error");
        }
    });

    // Forecast fetchers — one task per configured provider.
    let http = forecast::http_client();
    let mut forecast_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
    let solcast = SolcastClient::new(
        http.clone(),
        cfg.forecast.solcast.api_key.clone(),
        cfg.forecast.solcast.site_ids.clone(),
    );
    if solcast.is_configured() {
        let tx_f = tx.clone();
        let cadence = cfg.forecast.solcast.cadence;
        forecast_tasks.push(tokio::spawn(async move {
            let _ = forecast::run_scheduler(Box::new(solcast), cadence, tx_f).await;
        }));
    } else {
        info!("forecast: Solcast disabled (no api_key or site_ids)");
    }

    let fs_planes: Vec<_> = cfg
        .forecast
        .forecast_solar
        .planes
        .iter()
        .copied()
        .map(Into::into)
        .collect();
    let fs_client = ForecastSolarClient::new(
        http.clone(),
        cfg.forecast.forecast_solar.latitude,
        cfg.forecast.forecast_solar.longitude,
        fs_planes,
    );
    if fs_client.is_configured() {
        let tx_f = tx.clone();
        let cadence = cfg.forecast.forecast_solar.cadence;
        forecast_tasks.push(tokio::spawn(async move {
            let _ = forecast::run_scheduler(Box::new(fs_client), cadence, tx_f).await;
        }));
    } else {
        info!("forecast: Forecast.Solar disabled (no planes configured)");
    }

    let om_planes: Vec<_> = cfg
        .forecast
        .open_meteo
        .planes
        .iter()
        .copied()
        .map(Into::into)
        .collect();
    let om_client = OpenMeteoClient::new(
        http,
        cfg.forecast.open_meteo.latitude,
        cfg.forecast.open_meteo.longitude,
        om_planes,
    );
    if om_client.is_configured() {
        let tx_f = tx.clone();
        let cadence = cfg.forecast.open_meteo.cadence;
        forecast_tasks.push(tokio::spawn(async move {
            let _ = forecast::run_scheduler(Box::new(om_client), cadence, tx_f).await;
        }));
    } else {
        info!("forecast: Open-Meteo disabled (no planes configured)");
    }

    // NB: rumqttc's EventLoop is !Send on some feature configs, so the
    // MQTT subscriber cannot be `tokio::spawn`ed like the other
    // producers — it has to run inline on the main task. The `select!`
    // below includes it as a branch.
    let tx_for_mq = tx.clone();
    let mqtt_sub_fut = async move {
        if let Some(sub) = mqtt_subscriber {
            if let Err(e) = sub.run(tx_for_mq).await {
                error!(error = %e, "mqtt subscriber terminated with error");
            }
        } else {
            std::future::pending::<()>().await;
        }
    };

    // Dashboard HTTP server.
    let dashboard_bind: std::net::SocketAddr = format!(
        "{}:{}",
        cfg.dashboard.bind, cfg.dashboard.port
    )
    .parse()
    .context("parse dashboard bind addr")?;
    let dashboard = DashboardServer::new(dashboard_bind, world.clone(), tx.clone());
    let dashboard_task = tokio::spawn(async move {
        if let Err(e) = dashboard.run().await {
            error!(error = %e, "dashboard server terminated with error");
        }
    });

    drop(tx); // runtime owns no Sender → rx.recv() returns None after all producers exit

    let tick_period = cfg.tuning.tick_period;
    let runtime_task = tokio::spawn(async move {
        if let Err(e) = runtime.run(rx, tick_period).await {
            error!(error = %e, "runtime terminated with error");
        }
    });

    // SIGTERM is sent by daemontools' `svc -d` and by systemd. Ctrl-C
    // is SIGINT. Handle both so the service exits cleanly under
    // supervision.
    let mut sigterm = unix_signal(SignalKind::terminate())
        .context("install SIGTERM handler")?;

    // Wait for any shutdown signal or either task finishing.
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("received SIGINT (Ctrl-C); shutting down");
        }
        _ = sigterm.recv() => {
            info!("received SIGTERM; shutting down");
        }
        _ = subscriber_task => {
            info!("subscriber task ended");
        }
        _ = myenergi_task => {
            info!("myenergi task ended");
        }
        () = mqtt_sub_fut => {
            info!("mqtt subscriber ended");
        }
        _ = runtime_task => {
            info!("runtime task ended");
        }
        _ = dashboard_task => {
            info!("dashboard task ended");
        }
    }

    // `RealClock` is Copy; just reference to silence "unused import"
    // in the crate. (The runtime already has its own copy.)
    let _ = RealClock;

    Ok(())
}

fn init_tracing(log_tx: tokio::sync::mpsc::Sender<mqtt::LogRecord>) {
    use tracing_subscriber::{prelude::*, EnvFilter};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(MqttLogLayer::new(log_tx))
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
