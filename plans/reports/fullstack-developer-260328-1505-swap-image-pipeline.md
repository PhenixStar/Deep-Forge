# Phase Implementation Report

## Executed Phase
- Phase: wire-swap-image-endpoint
- Plan: ad-hoc (no plan dir)
- Status: completed

## Files Modified

| File | Change |
|------|--------|
| `core/rust-engine/dlc-server/src/main.rs` | Full rewrite of `swap_image` + state init; added `ServerState`, `Models`, helpers |
| `core/rust-engine/dlc-server/Cargo.toml` | Added `ndarray.workspace = true` (needed to construct `Frame` directly) |

## Tasks Completed

- [x] Introduced `Models { detector: Option<FaceDetector>, swapper: Option<FaceSwapper> }` struct in `main.rs`
- [x] Introduced `ServerState { app, models }` with `FromRef<ServerState> for Arc<RwLock<AppState>>` so all existing handlers keep their `State<Arc<RwLock<AppState>>>` signature unchanged
- [x] State initialization: load `FaceDetector` + `FaceSwapper` at startup; failures are logged as warnings, not panics (graceful degradation)
- [x] `swap_image` handler:
  - Parses multipart for `"source"` and `"target"` fields
  - Decodes both to BGR `Frame` (Array3<u8>) via `decode_to_bgr_frame`
  - Acquires `Mutex<Models>` lock
  - Returns 503 if detector or swapper absent
  - Runs `FaceDetector::detect` on both frames (threshold 0.5)
  - Returns 422 if no face found in either image
  - Calls `FaceSwapper::get_embedding` on highest-confidence source face
  - Calls `FaceSwapper::swap` mutating `target_frame` in-place
  - Encodes result as JPEG via `encode_bgr_frame_to_jpeg`
  - Returns `200 image/jpeg` bytes
- [x] All error paths use structured JSON bodies with appropriate HTTP status codes

## Tests Status

- `cargo check -p dlc-server`: **pass** (`Finished dev profile`)
- Unit tests: N/A (no unit tests added; existing tests unaffected)
- Integration tests: not run (require ONNX model files at runtime)

## Design Decisions

- **`ServerState` + `FromRef`**: axum's sub-state extraction lets all pre-existing handlers compile unchanged while `swap_image` gets the full `ServerState` (including `Arc<Mutex<Models>>`).
- **`Mutex` not `RwLock` for models**: `FaceDetector::detect` and `FaceSwapper` methods take `&mut self` (ONNX session internals are mutable); exclusive lock is required.
- **Models loaded at startup not per-request**: avoids repeated ONNX session creation overhead; missing model files produce a warning at boot, 503 at call time.
- **`ndarray` added to `dlc-server` deps**: `Frame = Array3<u8>` is constructed in `decode_to_bgr_frame`; the workspace already pins the version.

## Issues Encountered

None. One compile error (`ndarray` not in `dlc-server` deps) fixed by adding it to `Cargo.toml`.

## Next Steps

- Week 7: wire real camera frames into `handle_video_ws` using the same pipeline
- `/source` endpoint could pre-run detection + embedding when `FaceDetector`/`FaceSwapper` are available, caching the result in `AppState::source_face` to avoid re-detecting on every swap call
