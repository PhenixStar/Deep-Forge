# Code Review: Python & Rust Server Parity

**Reviewer**: code-reviewer | **Date**: 2026-03-28 | **Scope**: server.py, main.rs, state.rs

## Scope

- Files: `core/server.py`, `core/rust-engine/dlc-server/src/main.rs`, `core/rust-engine/dlc-server/src/state.rs`
- LOC: ~265 (180 Python + 230 Rust + 36 state)
- Focus: route parity, correctness, compilation

## Overall Assessment

Both servers are well-structured and implement the same API contract. Three functional bugs were found in the Rust server and all three have been fixed. Python server is clean.

## Issues Found & Fixed

### 1. [HIGH] `set_camera` did not validate or persist camera index (Rust)

**Was**: The handler accepted any `u32` index, echoed it back, but never stored it in `AppState.active_camera` and never validated against available cameras.

**Impact**: Frontend would believe the camera was switched but the Rust server would ignore the request entirely. The `active_camera` field in state was dead code.

**Fix**: Added `State` extractor, validation against `dlc_capture::list_cameras()`, and `s.active_camera = index` write -- matching Python's `/camera/{index}` behavior exactly.

### 2. [MEDIUM] WebSocket `/ws/video` silently dropped connection (Rust)

**Was**: `handle_video_ws` logged a warning server-side then returned, dropping the socket with no message to the client.

**Impact**: Frontend WebSocket `onopen` fires, then immediately gets a close with no explanation. Could appear as a connection failure rather than a stub.

**Fix**: Now sends a JSON `{"error": "not_implemented", ...}` text message followed by a proper Close frame, giving the frontend actionable information.

### 3. [LOW] Unused `Serialize` import (Rust)

**Was**: `use serde::{Deserialize, Serialize}` -- `Serialize` unused since all JSON output goes through `serde_json::json!()`.

**Fix**: Changed to `use serde::Deserialize;`. Eliminates compiler warning.

## Review Checklist Answers

| # | Question | Result |
|---|----------|--------|
| 1 | Endpoints consistent Python<->Rust? | Yes. Rust has extra `/swap/image` (forward-looking stub, returns 501). All shared routes match. |
| 2 | WS video loop per-frame processor re-fetch correct? | Yes. `get_frame_processors_modules()` is called each iteration so `fp_ui` toggles apply immediately. Lock scope is minimal (copy ref, release). |
| 3 | `/source` properly validates and stores images? | Yes. Multipart extraction, `image::load_from_memory` validation, stores bytes in state. One note: returns 422 for invalid images vs Python's 400 -- acceptable since 422 is semantically correct for "understood the request but the entity is unprocessable". |
| 4 | `/ws/video` handler properly stubbed? | **Fixed.** Was silently dropping. Now sends error message + Close frame. |
| 5 | CORS origins matching? | Yes. Both use identical origins: `tauri://localhost`, `http://localhost:1420`, `http://localhost:8008`. |
| 6 | `python3 -m py_compile` passes? | Yes. Clean. |
| 7 | `cargo check` passes? | Yes. Clean for dlc-server (1 pre-existing warning in dlc-core for unused `providers` param). |

## Remaining Observations (Not Fixed -- Out of Scope)

### Python: `@app.on_event("startup")` deprecation

FastAPI has deprecated `on_event` in favor of `lifespan` context manager. Not broken but will emit a deprecation warning on newer FastAPI versions.

### Python: blocking `cv2.VideoCapture.read()` on async event loop

`cap.read()` in the WebSocket handler is a blocking call on the asyncio event loop. At scale this would starve other coroutines. Should use `asyncio.to_thread()` or `run_in_executor()`. Acceptable for single-user sidecar use.

### Rust: `dlc-core` unused variable warning

`validate_models` has unused `providers` parameter in `dlc-core/src/lib.rs:52`. Not in the reviewed files but worth a `_providers` prefix when touching that crate.

## Compilation Status

| Target | Status |
|--------|--------|
| `python3 -m py_compile core/server.py` | PASS |
| `cargo check` (dlc-server) | PASS (0 warnings in dlc-server) |

## Files Modified

- `/raid/projects/deep-wcam/core/rust-engine/dlc-server/src/main.rs` -- 3 fixes applied
