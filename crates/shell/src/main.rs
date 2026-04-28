//! victron-controller binary entry point.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::signal;
use tokio::signal::unix::{signal as unix_signal, SignalKind};
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use victron_controller_core::types::{Event, TimerId, TimerStatus};
use victron_controller_core::world::World;
use victron_controller_core::Topology;
use victron_controller_shell::clock::RealClock;
use victron_controller_shell::config::{self, Config, DbusServices};
use victron_controller_shell::dashboard::{
    DashboardServer, SnapshotBroadcast, SocHistoryStore, SOC_SAMPLE_INTERVAL,
};
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
    let _tracing_guard = init_tracing(log_tx);

    let cfg_path = config_path_from_args();
    info!("loading config: {}", cfg_path.display());
    let cfg: Config = config::load(&cfg_path).with_context(|| "load config")?;

    // PR-hardware-config: install the hardware params into the
    // MQTT-layer OnceLock BEFORE HA discovery / retained-knob ingest,
    // so the per-direction grid_*_limit_w ceilings are in effect for
    // the very first knob_range() call.
    victron_controller_shell::mqtt::set_hardware_params(cfg.hardware.into());

    let services = cfg
        .dbus
        .services
        .clone()
        .unwrap_or_else(DbusServices::default_venus_3_70);

    // D-Bus subscriber → event channel → runtime → D-Bus writer.
    //
    // Capacity sized for the MQTT bootstrap flood: a populated broker can
    // replay several hundred retained knob-state messages (observed 431
    // in the field) before the runtime starts draining — the subscriber
    // and other producers block on `.await send` while this drains, so
    // undersized capacity stalls sensor re-seed and produces the
    // "sensors stale, no logs" symptom (A-70).
    let (tx, rx) = mpsc::channel(4096);

    // Watermark warning: once per minute, log if the channel is > 75%
    // full. Gives operators a heads-up before backpressure bites.
    // PR-URGENT-13-D04: include the previous sample so the warn line
    // shows trend direction — an operator can tell "climbing → imminent
    // stall" from "draining → recovering" without correlating two log
    // lines a minute apart.
    {
        let tx_watch = tx.clone();
        tokio::spawn(async move {
            let max = tx_watch.max_capacity();
            let threshold = max * 3 / 4;
            let mut last_warn: Option<Instant> = None;
            let mut last_in_use: usize = 0;
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                ticker.tick().await;
                if tx_watch.is_closed() {
                    break;
                }
                let remaining = tx_watch.capacity();
                let in_use = max - remaining;
                if in_use > threshold {
                    let now = Instant::now();
                    let should_warn = last_warn.is_none_or(|t| {
                        now.duration_since(t) >= std::time::Duration::from_secs(60)
                    });
                    if should_warn {
                        let trend = match in_use.cmp(&last_in_use) {
                            std::cmp::Ordering::Greater => "climbing",
                            std::cmp::Ordering::Less => "draining",
                            std::cmp::Ordering::Equal => "stable",
                        };
                        // Max channel capacity is 4096, well under isize::MAX
                        // on any realistic target; cast is safe.
                        #[allow(clippy::cast_possible_wrap)]
                        let delta: isize = (in_use as isize) - (last_in_use as isize);
                        tracing::warn!(
                            in_use,
                            max,
                            last_in_use,
                            delta,
                            trend,
                            "event channel > 75% full ({in_use}/{max}, {trend} by {delta})"
                        );
                        last_warn = Some(now);
                    }
                }
                last_in_use = in_use;
            }
        });
    }

    info!("starting D-Bus subscriber loop");
    // Pure config; the actual D-Bus connection is opened inside
    // `run()` and re-opened on every reconnect. That way a broker
    // eviction or transient I/O failure at startup does not abort the
    // binary — daemontools supervises the *process*, but the
    // subscriber now supervises its own connection.
    let mut subscriber = Subscriber::new(&services);
    // Build the reseed-trigger fan-in before the writer so the writer
    // can kick the subscriber for an immediate GetItems on the affected
    // service after every successful SetValue.
    let reseed_trigger = subscriber.reseed_channel();

    // A-39: startup warning when config-level gates disagree with the
    // runtime kill switch. The dashboard badge currently reads only
    // `knobs.writes_enabled` (the runtime kill switch); the two
    // config-file gates `[dbus] writes_enabled` and
    // `[myenergi] writes_enabled` silently no-op writes below that.
    // An operator flipping the badge ON while a config gate is OFF
    // would see "writes on" but nothing actuates. Explicit warning at
    // startup makes the three-gate structure visible in the log.
    if !cfg.dbus.writes_enabled {
        tracing::warn!(
            "config [dbus] writes_enabled = false — D-Bus writes will be suppressed \
             regardless of runtime kill switch (A-39 / SPEC §5 three-gate chain)"
        );
    }
    if !cfg.myenergi.writes_enabled {
        tracing::warn!(
            "config [myenergi] writes_enabled = false — myenergi writes will be suppressed \
             regardless of runtime kill switch (A-39 / SPEC §5 three-gate chain)"
        );
    }
    info!(
        "creating D-Bus writer (dry_run={}, lazy connect)",
        !cfg.dbus.writes_enabled
    );
    let writer =
        Writer::new(services.clone(), !cfg.dbus.writes_enabled).with_reseed_trigger(reseed_trigger);

    let myenergi_client = MyenergiClient::new(cfg.myenergi.clone());
    info!(
        "myenergi writer (dry_run={})",
        !cfg.myenergi.writes_enabled
    );
    let myenergi_writer = MyenergiWriter::new(myenergi_client.clone(), !cfg.myenergi.writes_enabled);
    let myenergi_poller = MyenergiPoller::new(myenergi_client, cfg.myenergi.poll_period);

    // PR-soc-history-persist: build the SoC-history store BEFORE
    // mqtt::connect so it can be threaded into the Subscriber for
    // bootstrap restore. The same Arc is later handed to MetaContext
    // and the periodic sampler.
    let soc_history = SocHistoryStore::new();

    // MQTT (optional; skipped when host is empty).
    let (mqtt_publisher, mqtt_subscriber) =
        match mqtt::connect(
            &cfg.mqtt,
            &cfg.outdoor_temperature_local,
            &cfg.ev,
            Arc::clone(&soc_history),
        )
        .await?
        {
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

    let topology = Topology::with_hardware(cfg.hardware.into());
    let meta = victron_controller_shell::dashboard::convert::MetaContext {
        services: services.clone(),
        open_meteo_cadence: cfg.forecast.open_meteo.cadence,
        controller_params: topology.controller_params,
        matter_outdoor_topic: cfg.outdoor_temperature_local.mqtt_topic.clone(),
        ev_soc_discovery_topic: cfg.ev.soc_topic.clone(),
        ev_charge_target_discovery_topic: cfg.ev.charge_target_topic.clone(),
        soc_history: Arc::clone(&soc_history),
        hardware: topology.hardware,
    };
    let mut world_seed = World::fresh_boot(Instant::now());
    // Apply config-file knob defaults on top of `Knobs::safe_defaults`.
    // Retained MQTT values still win on the next boot; this only
    // changes the seed used before any retained value arrives.
    cfg.knobs.apply_to(&mut world_seed.knobs);
    // PR-pinned-registers: seed `world.pinned_registers` from
    // `[[dbus_pinned_registers]]` so the dashboard surfaces every
    // configured row from boot, even before the first hourly read
    // pass lands. Validation already happened in `config::load`.
    for entry in &cfg.dbus_pinned_registers {
        let key: Arc<str> = Arc::from(entry.path.as_str());
        let target = victron_controller_shell::config::PinnedValue::from_validated(
            entry.value_type,
            &entry.value,
        )
        .expect("pinned-register value validated at config load");
        let target_core = match target {
            victron_controller_shell::config::PinnedValue::Bool(b) => {
                victron_controller_core::types::PinnedValue::Bool(b)
            }
            victron_controller_shell::config::PinnedValue::Int(n) => {
                victron_controller_core::types::PinnedValue::Int(n)
            }
            victron_controller_shell::config::PinnedValue::Float(f) => {
                victron_controller_core::types::PinnedValue::Float(f)
            }
            victron_controller_shell::config::PinnedValue::String(s) => {
                victron_controller_core::types::PinnedValue::String(s)
            }
        };
        world_seed.pinned_registers.insert(
            Arc::clone(&key),
            victron_controller_core::types::PinnedRegisterEntity::new(key, target_core),
        );
    }
    if !cfg.dbus_pinned_registers.is_empty() {
        info!(
            count = cfg.dbus_pinned_registers.len(),
            "pinned-register set seeded into world"
        );
    }
    let world = Arc::new(Mutex::new(world_seed));
    let snapshot_stream = SnapshotBroadcast::new(64);

    // PR-ha-knob-sync: clone the MQTT client handle BEFORE handing the
    // publisher to Runtime, so we can launch the initial-knob-state
    // publish task. The bootstrap path only READS retained values; HA
    // ends up showing "unknown" for any knob the user has never edited.
    // After bootstrap settles we walk every knob and push its current
    // value to retained MQTT — round-trip-stable when the value already
    // matched retained, or fresh-write when only the safe_default
    // existed in the controller.
    let initial_knob_publish_client = mqtt_publisher.as_ref().map(|p| p.client_handle());
    let initial_knob_publish_world = world.clone();
    let initial_knob_publish_topic = cfg.mqtt.topic_root.clone();

    // PR-soc-history-persist: drain serialized wire payloads from the
    // SoC-history store onto a single retained MQTT topic. The store
    // pushes new payloads on every record(); this task publishes them
    // with a 1 s timeout to avoid wedging on a stuck broker (mirrors
    // the log_layer publish timeout pattern).
    if let Some(p) = mqtt_publisher.as_ref() {
        let client = p.client_handle();
        let topic = format!("{}/state/soc_history", cfg.mqtt.topic_root);
        // Buffer of 8: at 1 publish per 15 min steady-state we'd never
        // queue more than one; 8 only matters during bootstrap-restore
        // immediately followed by record(). Drop-on-full is acceptable
        // — the next record republishes the full ring.
        let (history_publish_tx, mut history_publish_rx) = mpsc::channel::<String>(8);
        soc_history.set_publisher(history_publish_tx);
        tokio::spawn(async move {
            while let Some(payload) = history_publish_rx.recv().await {
                match tokio::time::timeout(
                    Duration::from_secs(1),
                    client.publish(
                        &topic,
                        rumqttc::QoS::AtMostOnce,
                        true,
                        payload.into_bytes(),
                    ),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    // Don't use tracing! here — a self-feeding loop via
                    // MqttLogLayer could wedge under broker stalls. Same
                    // pattern as log_layer.rs.
                    Ok(Err(e)) => {
                        eprintln!("mqtt soc_history publish failed on {topic}: {e}");
                    }
                    Err(_) => {
                        eprintln!(
                            "mqtt soc_history publish stuck >1s on {topic}; dropping payload"
                        );
                    }
                }
            }
        });
    }

    let runtime = Runtime::new(
        world.clone(),
        writer,
        myenergi_writer,
        mqtt_publisher,
        topology,
        snapshot_stream.clone(),
        meta.clone(),
    );

    if let Some(client) = initial_knob_publish_client {
        let tx_initial_knob = tx.clone();
        tokio::spawn(async move {
            // Wait long enough for the MQTT subscriber's bootstrap
            // window (BOOTSTRAP_WINDOW = ~750ms) and the post-bootstrap
            // AllowBatteryToCar reset to apply. 3s is conservative.
            tokio::time::sleep(Duration::from_secs(3)).await;
            let payloads = {
                let w = initial_knob_publish_world.lock().await;
                victron_controller_core::process::all_knob_publish_payloads(&w.knobs)
            };
            let mut count = 0_usize;
            for payload in payloads {
                if let Some((subtopic, body, retain)) =
                    victron_controller_shell::mqtt::encode_publish_payload(&payload)
                {
                    let topic = format!("{initial_knob_publish_topic}/{subtopic}");
                    if let Err(e) = client
                        .publish(&topic, rumqttc::QoS::AtLeastOnce, retain, body.as_bytes())
                        .await
                    {
                        warn!(error = %e, %topic, "initial knob publish failed");
                    } else {
                        count += 1;
                    }
                }
            }
            info!(count, "initial knob state published to retained MQTT");
            // PR-timers-section: signal the one-shot InitialKnobPublish
            // timer completion. No `next_fire` — runs once per process.
            let last_fire_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| {
                    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
                });
            let _ = tx_initial_knob
                .send(Event::TimerState {
                    id: TimerId::InitialKnobPublish,
                    last_fire_epoch_ms: last_fire_ms,
                    next_fire_epoch_ms: None,
                    status: TimerStatus::Idle,
                    at: Instant::now(),
                })
                .await;
        });
    }

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

    // PR-pinned-registers: hourly re-reader for the configured pinned
    // D-Bus registers. Idle when `[[dbus_pinned_registers]]` is empty.
    let tx_for_pinned = tx.clone();
    let pinned_registers_for_task = cfg.dbus_pinned_registers.clone();
    let pinned_task = tokio::spawn(async move {
        if let Err(e) = victron_controller_shell::dbus::pinned::run(
            pinned_registers_for_task,
            tx_for_pinned,
        )
        .await
        {
            error!(error = %e, "pinned-register reader terminated with error");
        }
    });

    // Forecast fetchers — one task per configured provider.
    let http = forecast::http_client();
    // A-50: already validated in `config::load()`; unwrap is safe.
    let forecast_tz = cfg
        .forecast
        .parse_timezone()
        .expect("forecast timezone validated at config load");
    let mut forecast_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
    let solcast = SolcastClient::new(
        http.clone(),
        cfg.forecast.solcast.api_key.clone(),
        cfg.forecast.solcast.site_ids.clone(),
        forecast_tz,
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
        forecast_tz,
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
    let om_lat = cfg.forecast.open_meteo.latitude;
    let om_lon = cfg.forecast.open_meteo.longitude;
    let om_cadence = cfg.forecast.open_meteo.cadence;
    let om_client = OpenMeteoClient::new(
        http.clone(),
        om_lat,
        om_lon,
        om_planes,
        cfg.forecast.open_meteo.system_efficiency,
        forecast_tz,
    );
    if om_client.is_configured() {
        let tx_f = tx.clone();
        forecast_tasks.push(tokio::spawn(async move {
            let _ = forecast::run_scheduler(Box::new(om_client), om_cadence, tx_f).await;
        }));
    } else {
        info!("forecast: Open-Meteo disabled (no planes configured)");
    }

    // PR-keep-batteries-charged: always-on sunrise/sunset scheduler.
    // Independent of `[forecast.baseline]` — the scheduler is a single
    // producer of `Event::SunriseSunset` and is gated only on the
    // operator having configured `[location]`. Without it, the
    // ESS-state override controller bias-to-safety branches kick in
    // (no override, no write).
    if cfg.location.is_configured() {
        let location = cfg.location.clone();
        let params = forecast::sunrise_sunset::SunriseSunsetParams {
            latitude: location.latitude,
            longitude: location.longitude,
            cadence: location.cadence,
            tz: forecast_tz,
        };
        let tx_l = tx.clone();
        forecast_tasks.push(tokio::spawn(async move {
            let _ =
                forecast::sunrise_sunset::run_sunrise_sunset_scheduler(params, tx_l).await;
        }));
    } else {
        info!(
            "sunrise/sunset: scheduler disabled (set [location] latitude/longitude to enable)"
        );
    }

    // PR-baseline-forecast: locally-computed last-resort fallback. Spun
    // up only when the operator has supplied both a site location and at
    // least one non-zero per-hour Wh constant.
    if cfg.forecast.baseline.is_configured() {
        let baseline = cfg.forecast.baseline.clone();
        let params = forecast::baseline::BaselineParams {
            latitude: baseline.latitude,
            longitude: baseline.longitude,
            cadence: baseline.cadence,
            tz: forecast_tz,
        };
        let tx_b = tx.clone();
        let world_b = world.clone();
        forecast_tasks.push(tokio::spawn(async move {
            let _ = forecast::baseline::run_baseline_scheduler(params, world_b, tx_b).await;
        }));
    } else {
        info!("forecast: baseline disabled (set [forecast.baseline] enabled = true to enable)");
    }

    // Outdoor temperature from Open-Meteo. Runs whenever Open-Meteo has
    // valid coordinates, independent of plane config — this is the
    // placeholder source for `outdoor_temperature` until the MQTT
    // weather-sensor binding (SPEC §10.2) is wired up.
    if om_lat != 0.0 || om_lon != 0.0 {
        let tx_t = tx.clone();
        let http_t = http;
        forecast_tasks.push(tokio::spawn(async move {
            let _ = forecast::current_weather::run_open_meteo_temperature(
                http_t, om_lat, om_lon, om_cadence, forecast_tz, tx_t,
            )
            .await;
        }));
    } else {
        info!("weather: Open-Meteo temperature poller disabled (no coordinates)");
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
    let dashboard = DashboardServer::new(
        dashboard_bind,
        world.clone(),
        tx.clone(),
        snapshot_stream,
        meta,
    );
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

    // PR-soc-chart: sample `battery_soc` every 15 min into the in-memory
    // ring. On boot we wait briefly then push the first Fresh reading so
    // the chart isn't blank for the first 15 min.
    {
        let world_for_sampler = Arc::clone(&world);
        let store_for_sampler = Arc::clone(&soc_history);
        tokio::spawn(async move {
            // Boot priming: poll every 5s for up to 5 minutes for the
            // first Fresh battery_soc reading so the chart has at least
            // one point before the periodic ticker fires.
            let mut booted = false;
            for _ in 0..60 {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let (soc_opt, fresh) = {
                    let w = world_for_sampler.lock().await;
                    let a = &w.sensors.battery_soc;
                    (
                        a.value,
                        matches!(
                            a.freshness,
                            victron_controller_core::Freshness::Fresh
                        ),
                    )
                };
                if fresh {
                    if let Some(soc) = soc_opt {
                        let now_ms = system_epoch_ms_now();
                        store_for_sampler.record(soc, now_ms);
                        booted = true;
                        break;
                    }
                }
            }
            if !booted {
                warn!("soc-chart: no Fresh battery_soc within 5 min of boot; ring stays empty until first interval tick");
            }
            // Steady-state ticker. `Skip` so a stalled tokio runtime
            // doesn't generate a burst of catch-up samples.
            let mut interval = tokio::time::interval(SOC_SAMPLE_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval.tick().await; // discard the immediate first tick
            loop {
                interval.tick().await;
                let (soc_opt, fresh) = {
                    let w = world_for_sampler.lock().await;
                    let a = &w.sensors.battery_soc;
                    (
                        a.value,
                        matches!(
                            a.freshness,
                            victron_controller_core::Freshness::Fresh
                        ),
                    )
                };
                if fresh {
                    if let Some(soc) = soc_opt {
                        let now_ms = system_epoch_ms_now();
                        store_for_sampler.record(soc, now_ms);
                    }
                }
            }
        });
    }

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
            // The subscriber now reconnects internally with backoff, so
            // this fires only on clean shutdown (event channel closed).
            info!("subscriber task ended");
        }
        _ = myenergi_task => {
            info!("myenergi task ended");
        }
        _ = pinned_task => {
            // PR-pinned-registers: `run` only returns on a closed
            // event channel, so this branch ~= clean shutdown.
            info!("pinned-register reader ended");
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

    // Reference the type to silence "unused import" — the runtime
    // already constructs its own clock from `topology.tz_handle`.
    let _ = std::marker::PhantomData::<RealClock>;

    Ok(())
}

fn init_tracing(log_tx: tokio::sync::mpsc::Sender<mqtt::LogRecord>) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{prelude::*, EnvFilter};
    // Route stdout writes through a dedicated blocking thread so the
    // tokio workers never touch the pipe. Under daemontools the pipe
    // buffer is ~64 KB; when multilog briefly slows, a synchronous
    // writer would block the worker inside `write_all` and (with
    // worker_threads=2) can wedge the whole async runtime — which
    // is exactly what PR-URGENT-15/16/17 each tried to address a
    // symptom of. The root cause is this synchronous writer.
    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_writer(non_blocking);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(MqttLogLayer::new(log_tx))
        .init();
    guard
}

fn system_epoch_ms_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
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
