//! Dashboard HTTP server: binds, exposes /api endpoints, serves the SPA.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, info, warn};

use victron_controller_core::types::Event;
use victron_controller_core::world::World;
use victron_controller_dashboard_model::victron_controller::dashboard::command::Command as ModelCommand;
use victron_controller_dashboard_model::victron_controller::dashboard::world_snapshot::WorldSnapshot;

use super::convert::{command_to_event, world_to_snapshot, MetaContext};
use super::ws::ws_handler;

/// Shared state: a snapshot of `World` (published by the runtime) and
/// a sender the HTTP layer uses to push Command events back to the
/// runtime.
#[derive(Clone)]
pub struct DashboardState {
    pub world: Arc<Mutex<World>>,
    pub events: mpsc::Sender<Event>,
    /// Broadcast channel used to fan out snapshots from the runtime's
    /// tick to every connected WebSocket client.
    pub snapshot_stream: Arc<SnapshotBroadcast>,
    /// Inputs needed to build `sensors_meta` (origin, identifier,
    /// cadence, staleness) for `/api/snapshot` and the ws priming
    /// snapshot.
    pub meta: MetaContext,
}

#[derive(Debug)]
pub struct SnapshotBroadcast {
    tx: broadcast::Sender<WorldSnapshot>,
}

impl SnapshotBroadcast {
    #[must_use]
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            tx: broadcast::channel(capacity).0,
        })
    }

    pub fn send(&self, snap: WorldSnapshot) {
        // broadcast::send returns Err only when there are no receivers;
        // that's fine — we don't buffer snapshots when no one is
        // subscribed.
        let _ = self.tx.send(snap);
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<WorldSnapshot> {
        self.tx.subscribe()
    }
}

impl std::fmt::Debug for DashboardState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashboardState")
            .field("events", &"<mpsc>")
            .field("world", &"<Arc<Mutex<World>>>")
            .finish()
    }
}

#[derive(Debug)]
pub struct DashboardServer {
    bind: SocketAddr,
    state: DashboardState,
}

impl DashboardServer {
    #[must_use]
    pub fn new(
        bind: SocketAddr,
        world: Arc<Mutex<World>>,
        events: mpsc::Sender<Event>,
        snapshot_stream: Arc<SnapshotBroadcast>,
        meta: MetaContext,
    ) -> Self {
        Self {
            bind,
            state: DashboardState {
                world,
                events,
                snapshot_stream,
                meta,
            },
        }
    }

    pub async fn run(self) -> Result<()> {
        let app = router(self.state.clone());
        let listener = tokio::net::TcpListener::bind(self.bind)
            .await
            .with_context(|| format!("bind dashboard on {}", self.bind))?;
        info!(bind = %self.bind, "dashboard server listening");
        axum::serve(listener, app).await.context("axum serve")?;
        Ok(())
    }
}

fn router(state: DashboardState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/snapshot", get(snapshot_handler))
        .route("/api/command", post(command_handler))
        .route("/api/version", get(version_handler))
        .route("/", get(index_handler))
        .route("/bundle.js", get(bundle_js_handler))
        .route("/style.css", get(index_css_handler))
        .with_state(state)
}

// --- handlers -------------------------------------------------------------

async fn snapshot_handler(State(s): State<DashboardState>) -> Json<WorldSnapshot> {
    let world = s.world.lock().await;
    Json(world_to_snapshot(&world, &s.meta))
}

async fn command_handler(
    State(s): State<DashboardState>,
    Json(cmd): Json<ModelCommand>,
) -> impl IntoResponse {
    let Some(event) = command_to_event(&cmd, Instant::now()) else {
        warn!(?cmd, "dashboard command has unknown knob or invalid value");
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"accepted": false, "error_message": "unknown knob or invalid value"})),
        );
    };
    if let Err(e) = s.events.send(event).await {
        error!(error = %e, "failed to forward dashboard command to runtime");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"accepted": false, "error_message": "runtime channel closed"})),
        );
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({"accepted": true, "error_message": null})),
    )
}

async fn version_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "current_version": env!("CARGO_PKG_VERSION"),
        "min_supported_version": env!("CARGO_PKG_VERSION"),
        "git_sha": option_env!("VICTRON_CONTROLLER_GIT_SHA"),
    }))
}

const INDEX_HTML: &str = include_str!("../../static/index.html");
const BUNDLE_JS: &str = include_str!("../../static/bundle.js");
const INDEX_CSS: &str = include_str!("../../static/style.css");

async fn index_handler() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")], INDEX_HTML)
}
async fn bundle_js_handler() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "application/javascript")], BUNDLE_JS)
}
async fn index_css_handler() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "text/css")], INDEX_CSS)
}
