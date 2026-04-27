//! WebSocket endpoint. Implements the protocol defined in
//! `models/dashboard.baboon` under `WsClientMessage` / `WsServerMessage`:
//!
//! - server → client: `Hello` on connect; `Snapshot` pushed every
//!   `snapshot_interval_ms`; `Pong` in response to every client `Ping`;
//!   `Log` streamed from the tracing layer; `Ack` in response to
//!   commands.
//! - client → server: `Ping` every few seconds (the reconnect
//!   manager); `SendCommand` for each user-driven knob edit.
//!
//! Each connected browser gets a dedicated task that pulls a
//! `watch::Receiver<WorldSnapshot>` for push updates and owns a
//! command `mpsc::Sender` back to the runtime.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tracing::{debug, warn};

use victron_controller_dashboard_model::baboon_runtime::{BaboonBinEncode, BaboonCodecContext};
use victron_controller_dashboard_model::victron_controller::dashboard::command_ack::CommandAck;
use victron_controller_dashboard_model::victron_controller::dashboard::world_snapshot::WorldSnapshot;
use victron_controller_dashboard_model::victron_controller::dashboard::ws_client_message::WsClientMessage;
use victron_controller_dashboard_model::victron_controller::dashboard::ws_pong::WsPong;
use victron_controller_dashboard_model::victron_controller::dashboard::ws_server_message as srv;
use victron_controller_dashboard_model::victron_controller::dashboard::ws_server_message::WsServerMessage;

use super::convert::{command_to_event, world_to_snapshot};
use super::server::DashboardState;

/// Upgrade handler; spawned per connecting client.
pub async fn ws_handler(
    State(s): State<DashboardState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| client_task(socket, s))
}

async fn client_task(socket: WebSocket, state: DashboardState) {
    let (mut tx_ws, mut rx_ws) = socket.split();

    // Send an initial Hello so the client can display server version.
    let hello = WsServerMessage::Hello(srv::Hello {
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        server_ts_ms: epoch_ms(),
    });
    if send_json(&mut tx_ws, &hello).await.is_err() {
        return;
    }

    // Send an initial snapshot right away so the dashboard populates
    // before the runtime's first tick. Build the snapshot inside a
    // tight scope that drops the MutexGuard BEFORE the network send —
    // otherwise a stalled WS client wedges the whole runtime (PR-URGENT-16).
    //
    // PR-tier-3-ueba: snapshots ride a binary frame carrying the
    // baboon UEBA-encoded `WorldSnapshot` bytes. JSON envelope stays
    // for the small / infrequent messages (Hello/Pong/Ack/Log) so the
    // discriminator on the client is "frame type" rather than a JSON
    // tag — saves the JSON.parse cost on the hot path.
    let snap = {
        let w = state.world.lock().await;
        world_to_snapshot(&w, &state.meta)
    };
    if send_snapshot_binary(&mut tx_ws, &snap).await.is_err() {
        return;
    }

    // Subscribe to the broadcast AFTER sending the priming snapshot —
    // anything published before this point is dropped for this client,
    // which is fine because we just sent a fresh one above.
    let mut snapshot_rx = state.snapshot_stream.subscribe();

    loop {
        tokio::select! {
            msg = rx_ws.next() => {
                let Some(Ok(msg)) = msg else { break; };
                if handle_client_msg(msg, &state, &mut tx_ws).await.is_err() {
                    break;
                }
            }
            recv = snapshot_rx.recv() => {
                match recv {
                    Ok(snap) => {
                        if send_snapshot_binary(&mut tx_ws, &snap).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!(skipped = n, "ws client lagged; skipping snapshots");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    debug!("ws client disconnected");
}

async fn handle_client_msg(
    msg: Message,
    state: &DashboardState,
    tx_ws: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) -> Result<(), ()> {
    let Message::Text(text) = msg else {
        return Ok(()); // ignore Binary/Ping/Pong/Close
    };
    let incoming: WsClientMessage = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, body = %text, "invalid ws client message");
            return Ok(());
        }
    };
    match incoming {
        WsClientMessage::Ping(p) => {
            let body = p.body;
            let pong = WsServerMessage::Pong(srv::Pong {
                body: WsPong {
                    nonce: body.nonce,
                    client_ts_ms: body.client_ts_ms,
                    server_ts_ms: epoch_ms(),
                },
            });
            send_json(tx_ws, &pong).await?;
        }
        WsClientMessage::SendCommand(c) => {
            let event = command_to_event(&c.body, Instant::now());
            // A-58: try_send instead of .send().await. A WS client that
            // keeps the connection open through a runtime slowdown
            // shouldn't tie up the per-client task indefinitely; if the
            // event channel is full we ack `accepted: false` and let
            // the client retry.
            let ack = match event {
                Some(ev) => match state.events.try_send(ev) {
                    Ok(()) => CommandAck {
                        accepted: true,
                        error_message: None,
                    },
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => CommandAck {
                        accepted: false,
                        error_message: Some(
                            "runtime event channel full; retry in a moment".to_string(),
                        ),
                    },
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => CommandAck {
                        accepted: false,
                        error_message: Some("runtime channel closed".to_string()),
                    },
                },
                None => CommandAck {
                    accepted: false,
                    error_message: Some("unknown knob or invalid value".to_string()),
                },
            };
            let out = WsServerMessage::Ack(srv::Ack { body: ack });
            send_json(tx_ws, &out).await?;
        }
    }
    Ok(())
}

async fn send_json(
    tx: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    msg: &WsServerMessage,
) -> Result<(), ()> {
    let body = match serde_json::to_string(msg) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "ws serialize failed");
            return Err(());
        }
    };
    tx.send(Message::Text(body)).await.map_err(|_| ())
}

/// PR-tier-3-ueba: serialize the snapshot via baboon's UEBA codec and
/// ship it as a WebSocket Binary frame. Avoids the multi-KB JSON
/// allocation+serialize on the server *and* the parallel `JSON.parse`
/// allocation on the browser — the dashboard's hottest path.
///
/// `BaboonCodecContext::Default` is the plain-bytes mode (no per-field
/// indices / lengths). The TS-side decoder also defaults to that mode,
/// so the two ends agree without extra negotiation.
async fn send_snapshot_binary(
    tx: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    snap: &WorldSnapshot,
) -> Result<(), ()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let ctx = BaboonCodecContext::Default;
    if let Err(e) = snap.encode_ueba(&ctx, &mut buf) {
        warn!(error = %e, "ws ueba encode failed");
        return Err(());
    }
    tx.send(Message::Binary(buf)).await.map_err(|_| ())
}

fn epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}
