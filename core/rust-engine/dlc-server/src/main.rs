//! Deep-Live-Cam Rust server — drop-in replacement for Python FastAPI sidecar.
//!
//! Implements the same HTTP API contract so the Tauri frontend works unchanged.

use axum::{
    Router,
    extract::{Path, Json, ws::{WebSocket, WebSocketUpgrade}},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{CorsLayer, Any};

mod state;

use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = Arc::new(RwLock::new(AppState::default()));

    let cors = CorsLayer::new()
        .allow_origin([
            "tauri://localhost".parse().unwrap(),
            "http://localhost:1420".parse().unwrap(),
            "http://localhost:8008".parse().unwrap(),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/source", post(upload_source))
        .route("/cameras", get(list_cameras))
        .route("/camera/{index}", post(set_camera))
        .route("/settings", get(get_settings).post(update_settings))
        .route("/ws/video", get(ws_video))
        .layer(cors)
        .with_state(state);

    let addr = "127.0.0.1:8008";
    tracing::info!("[SERVER] Rust backend starting on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// --- Handlers (stubs — wired up in Weeks 6-7) ---

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "backend": "rust"}))
}

async fn upload_source() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "TODO: Week 6")
}

async fn list_cameras(
    state: axum::extract::State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let cameras = dlc_capture::list_cameras().unwrap_or_default();
    Json(serde_json::json!({"cameras": cameras}))
}

async fn set_camera(Path(index): Path<u32>) -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "camera_index": index}))
}

async fn get_settings(
    state: axum::extract::State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let s = state.read().await;
    Json(serde_json::json!({
        "fp_ui": {
            "face_enhancer": s.face_enhancer_gfpgan,
            "face_enhancer_gpen256": s.face_enhancer_gpen256,
            "face_enhancer_gpen512": s.face_enhancer_gpen512,
        },
        "frame_processors": s.frame_processors,
    }))
}

#[derive(Deserialize)]
struct SettingsUpdate {
    face_enhancer: Option<bool>,
    face_enhancer_gpen256: Option<bool>,
    face_enhancer_gpen512: Option<bool>,
}

async fn update_settings(
    state: axum::extract::State<Arc<RwLock<AppState>>>,
    Json(body): Json<SettingsUpdate>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    if let Some(v) = body.face_enhancer { s.face_enhancer_gfpgan = v; }
    if let Some(v) = body.face_enhancer_gpen256 { s.face_enhancer_gpen256 = v; }
    if let Some(v) = body.face_enhancer_gpen512 { s.face_enhancer_gpen512 = v; }
    Json(serde_json::json!({"status": "ok"}))
}

async fn ws_video(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_video_ws)
}

async fn handle_video_ws(mut _socket: WebSocket) {
    // TODO Week 7: camera capture + face processing + JPEG streaming
    tracing::warn!("WebSocket video handler not yet implemented");
}
