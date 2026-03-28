//! Deep-Live-Cam Rust server — drop-in replacement for Python FastAPI sidecar.
//!
//! Implements the same HTTP API contract so the Tauri frontend works unchanged.

use axum::{
    Router,
    extract::{Path, Json, State, ws::{WebSocket, WebSocketUpgrade}},
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

    // Parse --models-dir CLI arg, fall back to env var / default inside AppState::default().
    let models_dir = parse_models_dir_arg();

    let mut app_state = AppState::default();
    if let Some(dir) = models_dir {
        app_state.models_dir = dir;
    }

    tracing::info!("[SERVER] models_dir = {}", app_state.models_dir.display());

    let state = Arc::new(RwLock::new(app_state));

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
        .route("/swap/image", post(swap_image))
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

/// Parse `--models-dir <path>` from process arguments.
fn parse_models_dir_arg() -> Option<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let pos = args.iter().position(|a| a == "--models-dir")?;
    args.get(pos + 1).map(std::path::PathBuf::from)
}

// --- Handlers ---

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "backend": "rust"}))
}

/// POST /source — multipart upload of a source face image.
/// Decodes the image to validate it, then stores raw bytes in state.
/// Face detection will be wired in Week 6 once dlc-core detection is ready.
async fn upload_source(
    State(state): State<Arc<RwLock<AppState>>>,
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    // Extract the first file field from the multipart body.
    let field = loop {
        match multipart.next_field().await {
            Ok(Some(f)) => break f,
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "no file field in multipart body"})),
                )
                    .into_response();
            }
            Err(e) => {
                tracing::error!("multipart error: {e}");
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("multipart error: {e}")})),
                )
                    .into_response();
            }
        }
    };

    let bytes = match field.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("failed to read field bytes: {e}")})),
            )
                .into_response();
        }
    };

    // Validate that the upload is a readable image.
    if let Err(e) = image::load_from_memory(&bytes) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": format!("invalid image: {e}")})),
        )
            .into_response();
    }

    tracing::info!("source image received: {} bytes", bytes.len());

    // Store raw bytes; face detection wired in Week 6.
    let mut s = state.write().await;
    s.source_image_bytes = Some(bytes.to_vec());
    s.source_face = None; // reset any previous detection result

    Json(serde_json::json!({"status": "ok", "bytes": bytes.len()})).into_response()
}

/// POST /swap/image — multipart with source + target images, returns swapped JPEG.
/// Full implementation deferred to Week 6 (ONNX models not yet wired).
async fn swap_image(
    _state: State<Arc<RwLock<AppState>>>,
    _multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "not implemented",
            "detail": "/swap/image will be fully wired in Week 6 once ONNX face-swap models are integrated"
        })),
    )
}

async fn list_cameras(
    _state: State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let cameras = dlc_capture::list_cameras().unwrap_or_default();
    Json(serde_json::json!({"cameras": cameras}))
}

async fn set_camera(Path(index): Path<u32>) -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "camera_index": index}))
}

async fn get_settings(
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let s = state.read().await;
    Json(serde_json::json!({
        "fp_ui": {
            "face_enhancer": s.face_enhancer_gfpgan,
            "face_enhancer_gpen256": s.face_enhancer_gpen256,
            "face_enhancer_gpen512": s.face_enhancer_gpen512,
        },
        "frame_processors": s.frame_processors,
        "models_dir": s.models_dir,
        "source_loaded": s.source_image_bytes.is_some(),
    }))
}

#[derive(Deserialize)]
struct SettingsUpdate {
    face_enhancer: Option<bool>,
    face_enhancer_gpen256: Option<bool>,
    face_enhancer_gpen512: Option<bool>,
}

async fn update_settings(
    State(state): State<Arc<RwLock<AppState>>>,
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
