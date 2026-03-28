//! Deep-Live-Cam Rust server — drop-in replacement for Python FastAPI sidecar.
//!
//! Implements the same HTTP API contract so the Tauri frontend works unchanged.

use axum::{
    Router,
    body::Bytes,
    extract::{Path, Json, State, ws::{WebSocket, WebSocketUpgrade}},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::{CorsLayer, Any};

mod state;

use state::AppState;
use dlc_core::{detect::FaceDetector, swap::FaceSwapper, Frame};

// ---------------------------------------------------------------------------
// Model container — held in a Mutex because detect/swap require &mut self.
// ---------------------------------------------------------------------------

struct Models {
    detector: Option<FaceDetector>,
    swapper:  Option<FaceSwapper>,
}

// ---------------------------------------------------------------------------
// Combined server state.
// `FromRef` lets existing handlers extract `Arc<RwLock<AppState>>` directly
// without changes to their signatures.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ServerState {
    app:    Arc<RwLock<AppState>>,
    models: Arc<Mutex<Models>>,
}

impl axum::extract::FromRef<ServerState> for Arc<RwLock<AppState>> {
    fn from_ref(s: &ServerState) -> Self {
        s.app.clone()
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

    // Attempt to load ONNX models at startup.  Models are optional: the server
    // starts successfully without them and returns 503 when they are needed.
    let det_path = app_state.models_dir.join("buffalo_l/buffalo_l/det_10g.onnx");
    let detector = match FaceDetector::new(&det_path) {
        Ok(d)  => { tracing::info!("FaceDetector loaded");             Some(d) }
        Err(e) => { tracing::warn!("FaceDetector unavailable: {e:#}"); None   }
    };
    let swapper = match FaceSwapper::new(&app_state.models_dir) {
        Ok(s)  => { tracing::info!("FaceSwapper loaded");              Some(s) }
        Err(e) => { tracing::warn!("FaceSwapper unavailable: {e:#}");  None   }
    };

    let server_state = ServerState {
        app:    Arc::new(RwLock::new(app_state)),
        models: Arc::new(Mutex::new(Models { detector, swapper })),
    };

    let cors = CorsLayer::new()
        .allow_origin([
            "tauri://localhost".parse().unwrap(),
            "http://localhost:1420".parse().unwrap(),
            "http://localhost:8008".parse().unwrap(),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health",         get(health))
        .route("/source",         post(upload_source))
        .route("/swap/image",     post(swap_image))
        .route("/cameras",        get(list_cameras))
        .route("/camera/{index}", post(set_camera))
        .route("/settings",       get(get_settings).post(update_settings))
        .route("/ws/video",       get(ws_video))
        .layer(cors)
        .with_state(server_state);

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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "backend": "rust"}))
}

/// POST /source — multipart upload of a source face image.
/// Decodes the image to validate it, then stores raw bytes in state.
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

    // Store raw bytes; reset any previous detection result.
    let mut s = state.write().await;
    s.source_image_bytes = Some(bytes.to_vec());
    s.source_face = None;

    Json(serde_json::json!({"status": "ok", "bytes": bytes.len()})).into_response()
}

// ---------------------------------------------------------------------------
// Image decode / encode helpers (used by swap_image)
// ---------------------------------------------------------------------------

/// Decode image bytes (any format) → BGR `Frame` (Array3<u8>, HWC).
fn decode_to_bgr_frame(bytes: &[u8]) -> anyhow::Result<Frame> {
    let img = image::load_from_memory(bytes)?.to_rgb8();
    let (w, h) = img.dimensions();
    let mut frame = ndarray::Array3::<u8>::zeros((h as usize, w as usize, 3));
    for (x, y, px) in img.enumerate_pixels() {
        // image crate yields RGB pixels; store as BGR to match dlc-core convention.
        frame[[y as usize, x as usize, 0]] = px[2]; // B
        frame[[y as usize, x as usize, 1]] = px[1]; // G
        frame[[y as usize, x as usize, 2]] = px[0]; // R
    }
    Ok(frame)
}

/// Encode BGR `Frame` (Array3<u8>, HWC) → JPEG bytes at quality 85.
fn encode_bgr_frame_to_jpeg(frame: &Frame) -> anyhow::Result<Vec<u8>> {
    use image::{ImageBuffer, Rgb, ImageEncoder};
    use image::codecs::jpeg::JpegEncoder;

    let (h, w, _) = frame.dim();
    // BGR HWC → RGB for the image encoder.
    let mut rgb_buf: Vec<u8> = Vec::with_capacity(h * w * 3);
    for y in 0..h {
        for x in 0..w {
            rgb_buf.push(frame[[y, x, 2]]); // R
            rgb_buf.push(frame[[y, x, 1]]); // G
            rgb_buf.push(frame[[y, x, 0]]); // B
        }
    }
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(w as u32, h as u32, rgb_buf)
            .ok_or_else(|| anyhow::anyhow!("failed to construct RgbImage from frame"))?;

    let mut out: Vec<u8> = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut out, 85);
    encoder.write_image(
        img.as_raw(),
        w as u32,
        h as u32,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// POST /swap/image
// ---------------------------------------------------------------------------

/// Accepts multipart with "source" and "target" image fields.
/// Runs face detection on both, extracts source embedding, runs inswapper,
/// returns a JPEG of the target with the source face applied.
///
/// 400 — missing/malformed fields.
/// 422 — no face detected in source or target.
/// 503 — ONNX models not loaded (models_dir missing files at startup).
/// 200 — JPEG bytes with Content-Type: image/jpeg.
async fn swap_image(
    State(server_state): State<ServerState>,
    mut multipart: axum::extract::Multipart,
) -> Response {
    // ── 1. Parse multipart: collect "source" and "target" fields ────────────
    let mut source_bytes: Option<Vec<u8>> = None;
    let mut target_bytes: Option<Vec<u8>> = None;

    loop {
        match multipart.next_field().await {
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("multipart read error: {e}")})),
                )
                    .into_response();
            }
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                match field.bytes().await {
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"error": format!("field read error: {e}")})),
                        )
                            .into_response();
                    }
                    Ok(b) => match name.as_str() {
                        "source" => source_bytes = Some(b.to_vec()),
                        "target" => target_bytes = Some(b.to_vec()),
                        _ => {} // ignore unknown fields
                    },
                }
            }
        }
    }

    let source_bytes = match source_bytes {
        Some(b) => b,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing multipart field: source"})),
            )
                .into_response();
        }
    };
    let target_bytes = match target_bytes {
        Some(b) => b,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing multipart field: target"})),
            )
                .into_response();
        }
    };

    // ── 2. Decode images to BGR frames ───────────────────────────────────────
    let source_frame = match decode_to_bgr_frame(&source_bytes) {
        Ok(f)  => f,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": format!("invalid source image: {e}")})),
            )
                .into_response();
        }
    };
    let mut target_frame = match decode_to_bgr_frame(&target_bytes) {
        Ok(f)  => f,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": format!("invalid target image: {e}")})),
            )
                .into_response();
        }
    };

    // ── 3. Lock models; 503 if detector not available ────────────────────────
    let mut models = server_state.models.lock().await;

    let detector = match models.detector.as_mut() {
        Some(d) => d,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "models not loaded",
                    "detail": "FaceDetector ONNX model unavailable — check models_dir"
                })),
            )
                .into_response();
        }
    };

    // ── 4. Detect faces in source ────────────────────────────────────────────
    let source_faces = match detector.detect(&source_frame, 0.5) {
        Ok(f)  => f,
        Err(e) => {
            tracing::error!("source detection failed: {e:#}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("source detection failed: {e}")})),
            )
                .into_response();
        }
    };

    if source_faces.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "no face detected in source image"})),
        )
            .into_response();
    }

    // ── 5. Detect faces in target ────────────────────────────────────────────
    let target_faces = match detector.detect(&target_frame, 0.5) {
        Ok(f)  => f,
        Err(e) => {
            tracing::error!("target detection failed: {e:#}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("target detection failed: {e}")})),
            )
                .into_response();
        }
    };

    if target_faces.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "no face detected in target image"})),
        )
            .into_response();
    }

    // Use the highest-confidence face from each image (detect() returns sorted by score desc).
    let source_face = source_faces.into_iter().next().unwrap();
    let target_face = target_faces.into_iter().next().unwrap();

    // ── 6. Check swapper; 503 if not available ───────────────────────────────
    let swapper = match models.swapper.as_mut() {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "models not loaded",
                    "detail": "FaceSwapper ONNX models unavailable — check models_dir"
                })),
            )
                .into_response();
        }
    };

    // ── 7. Extract source embedding via ArcFace ──────────────────────────────
    let embedding = match swapper.get_embedding(&source_frame, &source_face) {
        Ok(e)  => e,
        Err(e) => {
            tracing::error!("embedding extraction failed: {e:#}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("embedding extraction failed: {e}")})),
            )
                .into_response();
        }
    };

    // Attach embedding to the source face descriptor.
    let mut source_face_with_emb = source_face;
    source_face_with_emb.embedding = Some(embedding);

    // ── 8. Run inswapper ─────────────────────────────────────────────────────
    if let Err(e) = swapper.swap(&source_face_with_emb, &target_face, &mut target_frame) {
        tracing::error!("face swap failed: {e:#}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("face swap failed: {e}")})),
        )
            .into_response();
    }

    // ── 9. Encode result as JPEG and return ───────────────────────────────────
    let jpeg_bytes = match encode_bgr_frame_to_jpeg(&target_frame) {
        Ok(b)  => b,
        Err(e) => {
            tracing::error!("JPEG encoding failed: {e:#}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("JPEG encoding failed: {e}")})),
            )
                .into_response();
        }
    };

    tracing::info!("swap_image: returning {} byte JPEG", jpeg_bytes.len());

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/jpeg")],
        Bytes::from(jpeg_bytes),
    )
        .into_response()
}

async fn list_cameras(
    _state: State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let cameras = dlc_capture::list_cameras().unwrap_or_default();
    Json(serde_json::json!({"cameras": cameras}))
}

async fn set_camera(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(index): Path<u32>,
) -> impl IntoResponse {
    let cameras = dlc_capture::list_cameras().unwrap_or_default();
    if !cameras.iter().any(|c| c.index == index) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("Camera {} not available", index)})),
        )
            .into_response();
    }

    let mut s = state.write().await;
    s.active_camera = index;
    Json(serde_json::json!({"status": "ok", "camera_index": index})).into_response()
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
    if let Some(v) = body.face_enhancer         { s.face_enhancer_gfpgan    = v; }
    if let Some(v) = body.face_enhancer_gpen256  { s.face_enhancer_gpen256   = v; }
    if let Some(v) = body.face_enhancer_gpen512  { s.face_enhancer_gpen512   = v; }
    Json(serde_json::json!({"status": "ok"}))
}

async fn ws_video(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_video_ws(socket, state))
}

/// Generate a solid-blue (R=0, G=0, B=200) RGB test frame, 640x480.
/// Replaces real camera capture until nokhwa is wired in Week 7.
fn generate_test_frame() -> Vec<u8> {
    const W: usize = 640;
    const H: usize = 480;
    let mut pixels = vec![0u8; W * H * 3];
    for chunk in pixels.chunks_exact_mut(3) {
        chunk[0] = 0;   // R
        chunk[1] = 0;   // G
        chunk[2] = 200; // B
    }
    pixels
}

/// Encode raw RGB pixels (width x height x 3) as JPEG bytes at quality 80.
fn encode_jpeg(width: u32, height: u32, rgb_pixels: &[u8]) -> anyhow::Result<Vec<u8>> {
    use image::{ImageBuffer, Rgb, ImageEncoder};
    use image::codecs::jpeg::JpegEncoder;

    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb_pixels.to_vec())
            .ok_or_else(|| anyhow::anyhow!("invalid pixel buffer dimensions"))?;

    let mut buf: Vec<u8> = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buf, 80);
    encoder.write_image(
        img.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(buf)
}

/// WebSocket handler: streams JPEG frames at ~30 fps.
///
/// Frame source is a stub (solid-blue 640x480) until nokhwa is integrated in Week 7.
async fn handle_video_ws(mut socket: WebSocket, state: Arc<RwLock<AppState>>) {
    use axum::extract::ws::Message;
    use tokio::time::{interval, Duration};

    tracing::info!("[WS] video client connected");

    let mut ticker = interval(Duration::from_millis(33));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let rgb = generate_test_frame();

                {
                    let _s = state.read().await;
                    // When _s.source_face is Some, invoke processors here (Week 7).
                }

                let jpeg = match encode_jpeg(640, 480, &rgb) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!("[WS] JPEG encode error: {e}");
                        continue;
                    }
                };

                if let Err(e) = socket.send(Message::Binary(jpeg.into())).await {
                    tracing::info!("[WS] send failed (client disconnected): {e}");
                    break;
                }
            }

            msg = socket.recv() => {
                match msg {
                    None | Some(Ok(Message::Close(_))) => {
                        tracing::info!("[WS] client closed connection");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::warn!("[WS] receive error: {e}");
                        break;
                    }
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    tracing::info!("[WS] video handler exiting");
}
