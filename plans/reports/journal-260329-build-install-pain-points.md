# Deep Forge Build & Install Journal — 2026-03-28/29

## Overview

Full code review, fix implementation via 3-agent team, and Windows build/install of Deep Forge (real-time face swap desktop app). Documented every pain point and resolution for future reference.

---

## Pain Point 1: Tauri v2 Shell Plugin Config Migration

**Symptom:** App crashes on startup with:
```
PluginInitialization("shell", "Error deserializing 'plugins.shell': unknown field `scope`, expected `open`")
```

**Root Cause:** `tauri.conf.json` had Tauri v1 `plugins.shell.scope` config. In Tauri v2, the shell plugin only accepts `open` (boolean). Sidecar permissions moved to the capabilities system.

**Fix:**
1. Replace `plugins.shell.scope` with `"shell": { "open": true }` in `tauri.conf.json`
2. Add scoped `shell:allow-spawn` permission in `capabilities/default.json`:
```json
{
  "identifier": "shell:allow-spawn",
  "allow": [{ "name": "binaries/deep-live-cam-server", "sidecar": true, "args": true }]
}
```

**Lesson for v1.0 installer:** Always validate Tauri plugin config against the exact Tauri version in `Cargo.toml`. The `shell:allow-sidecar` permission does NOT exist — use scoped `shell:allow-spawn`.

---

## Pain Point 2: Sidecar Binary Resolution on Windows

**Symptom:** App starts but panics with:
```
failed to spawn sidecar: Io(Os { code: 3, kind: NotFound, message: "The system cannot find the path specified." })
```

**Root Cause:** Tauri's `app.shell().sidecar("binaries/deep-live-cam-server")` resolves to `{resource_dir}/binaries/deep-live-cam-server-x86_64-pc-windows-msvc.exe`. But the NSIS installer flattens all binaries into a single directory without the `binaries/` subdirectory or the target-triple suffix.

**Fix:** Replaced Tauri's `sidecar()` API with `std::process::Command` and a custom `resolve_server_exe()` function that checks multiple candidate paths:
1. `{resource_dir}/deep-live-cam-server.exe` (NSIS flat install)
2. `{resource_dir}/binaries/deep-live-cam-server-{triple}.exe` (Tauri convention)
3. Same directory as the app exe (fallback)

**Lesson for v1.0 installer:** Either:
- Configure NSIS to preserve the `binaries/` subdirectory structure, OR
- Keep the robust multi-path resolver (recommended — handles all install methods)

---

## Pain Point 3: CORS Origin Mismatch on Windows

**Symptom:** App window loads but shows "Backend not reachable". Backend health check works from curl.

**Root Cause:** Tauri v2 on Windows uses `http://tauri.localhost` as the webview origin. The Rust server's CORS config only allowed `tauri://localhost` (the Tauri v1/macOS origin).

**Fix:** Added all Tauri v2 origins to CORS:
```rust
"tauri://localhost"         // macOS/Linux
"http://tauri.localhost"    // Windows
"https://tauri.localhost"   // Windows (HTTPS variant)
"http://localhost:1420"     // dev server
"http://localhost:8008"     // direct API access
```

**Lesson for v1.0:** Always include both `tauri://localhost` and `http://tauri.localhost` in CORS. Test on Windows specifically — the origin differs from macOS/Linux.

---

## Pain Point 4: OpenCV Build Toolchain on Windows

**Symptom:** `cargo build --features dlc-capture/opencv` fails with multiple errors.

**Sub-issues encountered in order:**

### 4a: libclang.dll not found
```
STATUS_DLL_NOT_FOUND — clang-sys build script
```
**Fix:** Install LLVM via `scoop install llvm`. Set `LIBCLANG_PATH` env var.

### 4b: Git Bash doesn't propagate PATH to Windows executables
Even with `export PATH="...llvm/bin:$PATH"`, the build-script .exe couldn't find `libclang.dll`. Git Bash's PATH doesn't translate to Windows DLL search paths for spawned processes.

**Fix:** Run `cargo build` via PowerShell instead of Git Bash, which properly sets Windows PATH.

### 4c: opencv crate 0.93 incompatible with OpenCV 4.12
```
error[E0599]: no method named `as_raw_VectorOfVideoCapture` found
```
115 compile errors from mismatched bindings.

**Fix:** Update `opencv` crate from `0.93` to `0.98` in `dlc-capture/Cargo.toml`.

### 4d: API change in opencv 0.98
```
error[E0599]: no function or associated item named `is_opened` found for struct `VideoCapture`
```
**Fix:** Import `VideoCaptureTraitConst` and change `VideoCapture::is_opened(&cap)` to `cap.is_opened()`.

### 4e: vcpkg triplet mismatch
The crate's vcpkg probe looked for `x64-windows-static-md` but vcpkg installed `x64-windows`.

**Fix:** Bypass probing entirely with explicit env vars:
```
OPENCV_INCLUDE_PATHS=.../installed/x64-windows/include/opencv4
OPENCV_LINK_PATHS=.../installed/x64-windows/lib
OPENCV_LINK_LIBS=opencv_core4,opencv_imgproc4,opencv_imgcodecs4,opencv_videoio4,opencv_highgui4
```

**Lesson for v1.0 build scripts:** Create a `scripts/build-rust-sidecar-win-opencv.ps1` that:
1. Checks for LLVM, vcpkg, OpenCV prerequisites
2. Sets all env vars (LIBCLANG_PATH, OPENCV_*, VCPKG_ROOT)
3. Runs cargo build via PowerShell (not bash)
4. Copies OpenCV DLLs alongside the sidecar binary

---

## Pain Point 5: Camera Probe Hangs Server

**Symptom:** After enabling OpenCV, the `/cameras` endpoint hangs for 30-60 seconds. During this time, ALL other endpoints (including `/health`) are unreachable.

**Root Cause:** `list_cameras_opencv()` probes indices 0-9 synchronously. On Windows with MSMF backend, each failed camera index blocks for 5-10 seconds. This blocks the tokio async runtime since it ran on the main thread.

**Fix:**
1. Reduce probe range from 0-9 to 0-3 (most users have 0-2 cameras)
2. Wrap the probe in `tokio::task::spawn_blocking()` so the server stays responsive

**Lesson for v1.0:** Camera probing should be:
- Async (spawn_blocking or dedicated thread)
- Cached (probe once on startup, re-probe only on user request)
- Timeout-bounded (skip indices that don't respond within 2s)

---

## Pain Point 6: OpenCV Runtime DLLs

**Symptom:** Sidecar works in dev (DLLs on PATH) but crashes in installed app (DLLs not found).

**Fix:** Copy all OpenCV DLLs from `vcpkg/installed/x64-windows/bin/` to the install directory alongside the sidecar exe.

**Lesson for v1.0 installer:** The NSIS/MSI bundle must include:
- `deep-live-cam-server.exe`
- `opencv_core4.dll`, `opencv_imgproc4.dll`, `opencv_imgcodecs4.dll`, `opencv_videoio4.dll`, `opencv_highgui4.dll`
- Transitive deps: `tiff.dll`, `jpeg62.dll`, `libpng16.dll`, `zlib1.dll`, `libwebp.dll`, etc.

Use `dumpbin /dependents dlc-server.exe` or `ldd` to identify the full DLL dependency tree.

---

## Pain Point 7: GitHub PAT Workflow Scope

**Symptom:** `git push` rejected for `.github/workflows/ci.yml`:
```
refusing to allow a Personal Access Token to create or update workflow without `workflow` scope
```

**Fix:** Cannot be fixed via API either. Options:
1. Update PAT to include `workflow` scope
2. Push workflow changes from a different auth method (SSH key, GitHub App)
3. Create PR from a branch (still needs workflow scope for the push)

**Lesson:** Always ensure PATs have `workflow` scope if CI files will be modified.

---

## Summary of All Changes Made

| Commit | Changes |
|--------|---------|
| `b03dbca` | 7 code review fixes: WS pipeline, per-model locks, upload limit, ErrorBoundary, createImageBitmap, Tauri config, sidecar resolver |
| `6f8334d` | OpenCV 0.98 upgrade, camera probe fixes, spawn_blocking, CORS origins |
| (pending) | CI workflow with clippy + cargo test (needs `workflow` PAT scope) |

## Build Requirements for v1.0 Release

### Windows Build Prerequisites
- Rust stable toolchain
- Node.js 22+ / pnpm 10+
- LLVM (for libclang.dll — `scoop install llvm`)
- vcpkg with `opencv4:x64-windows`
- cmake (`scoop install cmake`)

### Build Command (PowerShell)
```powershell
$env:LIBCLANG_PATH = "C:\path\to\llvm\bin"
$env:OPENCV_INCLUDE_PATHS = "C:\path\to\vcpkg\installed\x64-windows\include\opencv4"
$env:OPENCV_LINK_PATHS = "C:\path\to\vcpkg\installed\x64-windows\lib"
$env:OPENCV_LINK_LIBS = "opencv_core4,opencv_imgproc4,opencv_imgcodecs4,opencv_videoio4,opencv_highgui4"
cd core\rust-engine
cargo build --release -p dlc-server --features dlc-capture/opencv
```

### Installer Must Bundle
- `deep-live-cam-app.exe` (Tauri shell)
- `deep-live-cam-server.exe` (Rust sidecar)
- All OpenCV + transitive DLLs
- ONNX models (or first-run download mechanism)

### No Admin Required
App installs to `%LOCALAPPDATA%\Deep Live Cam\` — fully user-scoped, no UAC prompt needed.
