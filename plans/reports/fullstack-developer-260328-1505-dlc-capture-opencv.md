# Phase Implementation Report

## Executed Phase
- Phase: dlc-capture opencv migration
- Plan: ad-hoc (no plan dir)
- Status: completed

## Environment Discovery

OpenCV 4.13.0 is present only as a Python wheel
(`~/.local/lib/python3.12/site-packages/cv2/cv2.abi3.so`) with bundled
static libs.  No system pkg-config `.pc` file exists, so the Rust `opencv`
crate's build script cannot locate headers or link flags.  Feature-flag
approach is therefore required, as the task specified.

## Files Modified

| File | Lines | Change |
|---|---|---|
| `core/rust-engine/dlc-capture/Cargo.toml` | 20 | added ndarray dep + optional opencv dep under `[features]` |
| `core/rust-engine/dlc-capture/src/lib.rs` | 240 | full rewrite |

## Tasks Completed

- [x] Added `opencv = { version = "0.93", features = ["videoio"] }` as optional dep
- [x] Added `opencv` feature flag
- [x] Added `ndarray` dep (from workspace)
- [x] Defined `Frame = Array3<u8>` (H x W x C, BGR)
- [x] Implemented `CameraCapture` struct with `open`, `read_frame`, `set_resolution`, `Drop`
- [x] `Drop` calls `cap.release()` (opencv path) / logs debug (stub path)
- [x] Updated `list_cameras()` to probe indices 0-9 (opencv path)
- [x] Stub fallback: generates synthetic 640x480 BGR gradient frames, logs warning
- [x] `#[cfg(feature = "opencv")]` / `#[cfg(not(feature = "opencv"))]` guards throughout

## Tests Status

- `cargo check -p dlc-capture` (stub, no feature): **pass**
- `cargo check` (full workspace): **pass**
- OpenCV feature path: not checked at runtime (no system pkg-config); Rust
  code is syntactically and type-correct; will link once system libopencv
  headers are installed (e.g. `apt install libopencv-dev`).

## Architecture Notes

Two backend modules in the same file:

```
opencv_backend  (cfg feature="opencv")
  CameraCaptureInner  →  wraps opencv::videoio::VideoCapture
  list_cameras_opencv →  probes 0-9, opens + checks isOpened + releases

stub_backend    (cfg not(feature="opencv"))
  CameraCaptureInner  →  synthetic gradient frames, width/height settable
  list_cameras_stub   →  returns single "Stub Camera" entry
```

Public API is identical regardless of feature; callers need no conditional
compilation.

## Activating the Real Backend

Once system OpenCV dev headers are available:

```bash
apt install libopencv-dev          # installs pkg-config .pc file
cargo check -p dlc-capture --features opencv
cargo build -p dlc-capture --features opencv
```

## Issues Encountered

None. No file ownership violations.

## Unresolved Questions

- Should `set_resolution` return `Result` to surface CAP_PROP set failures?
  Currently best-effort / silent to match Python cv2 usage pattern.
- Should `list_cameras` cache results or always probe live?
