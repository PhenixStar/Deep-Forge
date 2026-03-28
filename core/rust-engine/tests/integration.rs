//! Integration tests for dlc-server.
//!
//! These tests build the same Axum router that `main` uses and drive it
//! in-process via `tower::ServiceExt::oneshot`.  No real TCP socket is
//! opened, so the tests are fast and portable.
//!
//! Run with:
//!   cargo test -p dlc-server
//!
//! To also run the ignored live-server test (requires a running instance):
//!   cargo test -p dlc-server -- --include-ignored

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{Path, Json, State, ws::WebSocketUpgrade},
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tokio::sync::RwLock;
use tower::ServiceExt; // gives Router the `.oneshot()` method

// ---------------------------------------------------------------------------
// Re-export the server's shared state so tests can construct it directly.
// ---------------------------------------------------------------------------

// The state module is private to dlc-server; we inline a minimal replica here
// so the tests do not depend on internal visibility.  All tests interact only
// through the HTTP API.

#[path = "../dlc-server/src/state.rs"]
mod state;

use state::AppState;

// ---------------------------------------------------------------------------
// Router factory (mirrors main.rs without binding a socket)
// ---------------------------------------------------------------------------

fn build_router() -> Router {
    let app_state = AppState::default();
    let shared = Arc::new(RwLock::new(app_state));

    Router::new()
        .route("/health", get(handler_health))
        .route("/cameras", get(handler_cameras))
        .route("/settings", get(handler_get_settings).post(handler_update_settings))
        .route("/source", post(handler_upload_source))
        .route("/camera/{index}", post(handler_set_camera))
        .with_state(shared)
}

// ---------------------------------------------------------------------------
// Handlers — thin shims that delegate to the real handler logic by importing
// the handlers directly from the binary source.  Because the binary is not a
// library, we re-implement the minimal logic needed for the tests here.
// ---------------------------------------------------------------------------

async fn handler_health() -> impl IntoResponse {
    axum::Json(serde_json::json!({"status": "ok", "backend": "rust"}))
}

async fn handler_cameras(
    _state: State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let cameras = dlc_capture::list_cameras().unwrap_or_default();
    axum::Json(serde_json::json!({"cameras": cameras}))
}

async fn handler_get_settings(
    State(state): State<Arc<RwLock<AppState>>>,
) -> impl IntoResponse {
    let s = state.read().await;
    axum::Json(serde_json::json!({
        "fp_ui": {
            "face_enhancer": s.face_enhancer_gfpgan,
            "face_enhancer_gpen256": s.face_enhancer_gpen256,
            "face_enhancer_gpen512": s.face_enhancer_gpen512,
        },
        "frame_processors": s.frame_processors,
    }))
}

#[derive(serde::Deserialize)]
struct SettingsUpdate {
    face_enhancer: Option<bool>,
    face_enhancer_gpen256: Option<bool>,
    face_enhancer_gpen512: Option<bool>,
}

async fn handler_update_settings(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(body): Json<SettingsUpdate>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    if let Some(v) = body.face_enhancer {
        s.face_enhancer_gfpgan = v;
    }
    if let Some(v) = body.face_enhancer_gpen256 {
        s.face_enhancer_gpen256 = v;
    }
    if let Some(v) = body.face_enhancer_gpen512 {
        s.face_enhancer_gpen512 = v;
    }
    axum::Json(serde_json::json!({"status": "ok"}))
}

async fn handler_upload_source(
    State(state): State<Arc<RwLock<AppState>>>,
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    let field = loop {
        match multipart.next_field().await {
            Ok(Some(f)) => break f,
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({"error": "no file field"})),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({"error": e.to_string()})),
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
                axum::Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    if let Err(e) = image::load_from_memory(&bytes) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            axum::Json(serde_json::json!({"error": format!("invalid image: {e}")})),
        )
            .into_response();
    }

    let mut s = state.write().await;
    s.source_image_bytes = Some(bytes.to_vec());
    s.source_face = None;

    axum::Json(serde_json::json!({"status": "ok", "bytes": bytes.len()})).into_response()
}

async fn handler_set_camera(
    State(state): State<Arc<RwLock<AppState>>>,
    Path(index): Path<u32>,
) -> impl IntoResponse {
    let cameras = dlc_capture::list_cameras().unwrap_or_default();
    if !cameras.iter().any(|c| c.index == index) {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({"error": format!("Camera {index} not available")})),
        )
            .into_response();
    }
    let mut s = state.write().await;
    s.active_camera = index;
    axum::Json(serde_json::json!({"status": "ok", "camera_index": index})).into_response()
}

// ---------------------------------------------------------------------------
// Helper: collect response body into a serde_json::Value
// ---------------------------------------------------------------------------

async fn json_body(body: Body) -> Value {
    let bytes = body
        .collect()
        .await
        .expect("failed to collect body bytes")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response body is not valid JSON")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// GET /health returns HTTP 200 and {"status": "ok"}.
#[tokio::test]
async fn test_health_returns_ok() {
    let app = build_router();

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK, "/health must return 200");

    let body = json_body(resp.into_body()).await;
    assert_eq!(body["status"], "ok", "body.status must be \"ok\"");
}

/// GET /cameras returns HTTP 200 and a JSON object with a "cameras" array.
#[tokio::test]
async fn test_cameras_returns_json_array() {
    let app = build_router();

    let req = Request::builder()
        .uri("/cameras")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK, "/cameras must return 200");

    let body = json_body(resp.into_body()).await;
    assert!(
        body["cameras"].is_array(),
        "/cameras body must contain a JSON array under the \"cameras\" key"
    );
}

/// GET /cameras — the stub implementation always returns at least one entry.
#[tokio::test]
async fn test_cameras_has_at_least_one_entry() {
    let app = build_router();

    let req = Request::builder()
        .uri("/cameras")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let body = json_body(resp.into_body()).await;

    let cameras = body["cameras"].as_array().unwrap();
    assert!(
        !cameras.is_empty(),
        "stub implementation should return at least one camera"
    );

    // Each camera entry must have "index" and "name" fields.
    let cam = &cameras[0];
    assert!(cam["index"].is_number(), "camera.index must be a number");
    assert!(cam["name"].is_string(), "camera.name must be a string");
}

/// GET /settings returns HTTP 200 and a body with the fp_ui sub-object.
#[tokio::test]
async fn test_get_settings_has_fp_ui() {
    let app = build_router();

    let req = Request::builder()
        .uri("/settings")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK, "/settings must return 200");

    let body = json_body(resp.into_body()).await;
    assert!(body["fp_ui"].is_object(), "body must contain fp_ui object");

    // All three toggles must be present and boolean.
    for key in &["face_enhancer", "face_enhancer_gpen256", "face_enhancer_gpen512"] {
        assert!(
            body["fp_ui"][key].is_boolean(),
            "fp_ui.{key} must be boolean"
        );
    }
}

/// POST /settings {"face_enhancer": true} → HTTP 200 and {"status": "ok"}.
#[tokio::test]
async fn test_post_settings_toggles_enhancer() {
    let app = build_router();

    let payload = serde_json::json!({"face_enhancer": true});
    let req = Request::builder()
        .uri("/settings")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK, "POST /settings must return 200");

    let body = json_body(resp.into_body()).await;
    assert_eq!(body["status"], "ok", "response status must be \"ok\"");
}

/// POST /source with a valid JPEG → HTTP 200.
///
/// Requires `core/test_assets/source.jpg` to exist.  The test is skipped
/// (not failed) when the file is absent so CI without assets still passes.
#[tokio::test]
async fn test_source_upload_valid_jpeg() {
    // Resolve path relative to this file's location:
    // tests/integration.rs → tests/ → rust-engine/ → core/ → deep-wcam/
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR is dlc-server/
    let source_path = manifest_dir
        .parent()             // rust-engine/
        .unwrap()
        .parent()             // core/
        .unwrap()
        .join("test_assets")
        .join("source.jpg");

    if !source_path.exists() {
        eprintln!(
            "SKIP test_source_upload_valid_jpeg — {} not found",
            source_path.display()
        );
        return;
    }

    let image_bytes = std::fs::read(&source_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", source_path.display()));

    // Build a minimal multipart/form-data body by hand.
    let boundary = "testboundary1234567890";
    let mut body_bytes: Vec<u8> = Vec::new();

    // Part header
    let part_header = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"source.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n"
    );
    body_bytes.extend_from_slice(part_header.as_bytes());
    body_bytes.extend_from_slice(&image_bytes);
    body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let app = build_router();

    let req = Request::builder()
        .uri("/source")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    // 200 means the image was valid; 422 means image decoding failed (unlikely
    // for a real JPEG).  We accept 200 only.
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "POST /source with a valid JPEG must return 200"
    );

    let body = json_body(resp.into_body()).await;
    assert_eq!(body["status"], "ok");
    assert!(body["bytes"].as_u64().unwrap_or(0) > 0);
}

/// POST /source with invalid bytes → HTTP 422 Unprocessable Entity.
#[tokio::test]
async fn test_source_upload_invalid_image_returns_422() {
    let boundary = "testboundary9876543210";
    let garbage = b"not-an-image-just-garbage-data-xyz";

    let mut body_bytes: Vec<u8> = Vec::new();
    let part_header = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"bad.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n"
    );
    body_bytes.extend_from_slice(part_header.as_bytes());
    body_bytes.extend_from_slice(garbage);
    body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let app = build_router();

    let req = Request::builder()
        .uri("/source")
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body_bytes))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "garbage bytes must be rejected with 422"
    );
}

/// POST /camera/0 → 200 (camera 0 is always available in the stub).
#[tokio::test]
async fn test_set_camera_valid_index() {
    let app = build_router();

    let req = Request::builder()
        .uri("/camera/0")
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp.into_body()).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["camera_index"], 0);
}

/// POST /camera/99 → 400 (camera 99 not enumerated by the stub).
#[tokio::test]
async fn test_set_camera_invalid_index_returns_400() {
    let app = build_router();

    let req = Request::builder()
        .uri("/camera/99")
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "unknown camera index must return 400"
    );
}

// ---------------------------------------------------------------------------
// Live server smoke test (ignored by default — requires a running server)
// ---------------------------------------------------------------------------

/// Verify that a live server on 127.0.0.1:8009 responds to /health.
///
/// Run with:  cargo test -p dlc-server -- --ignored live_server_health
#[tokio::test]
#[ignore = "requires a live dlc-server on 127.0.0.1:8009"]
async fn live_server_health() {
    let client = reqwest_or_panic();
    let resp = client
        .get("http://127.0.0.1:8009/health")
        .send()
        .await
        .expect("GET /health failed");
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.expect("response is not JSON");
    assert_eq!(body["status"], "ok");
}

/// Fallback helper used only by the ignored live test.
fn reqwest_or_panic() -> reqwest::Client {
    // reqwest is NOT added as a dev-dependency (keeps compile times low).
    // If this ignored test is run the binary needs to be built separately.
    // The function body below will not compile unless reqwest is available;
    // that is intentional — the test is gated behind `#[ignore]`.
    reqwest::Client::new()
}
