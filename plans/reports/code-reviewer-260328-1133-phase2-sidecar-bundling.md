# Code Review: Phase 2 -- Python-Build-Standalone Sidecar Bundling

**Reviewer**: code-reviewer
**Date**: 2026-03-28
**Scope**: Phase 2 sidecar build scripts, Windows Rust launcher, tauri.conf.json, server.py changes

---

## Scope

- **Files reviewed**: 7 (3 build scripts, Cargo.toml, main.rs launcher, tauri.conf.json, server.py)
- **LOC**: ~300 new/modified
- **Focus**: Build correctness, path resolution, env vars, Tauri integration, error handling
- **Scout findings**: 5 edge cases documented below

---

## Overall Assessment

Solid implementation. The three-platform build strategy is clean and consistent. The env-var-based `MODELS_DIR` in `modules/paths.py` is the right abstraction -- all four frame processors already import from it, so the sidecar wrappers setting `DEEP_LIVE_CAM_MODELS_DIR` correctly propagates everywhere. The Windows Rust launcher mirrors the bash wrappers well. A handful of issues ranging from Critical to Low are documented below.

---

## Critical Issues

### C1. Sidecar child process is never killed on app exit

**File**: `app/src-tauri/src/main.rs:21`
**Problem**: The sidecar `CommandChild` is stored as `_child` (unused binding) and never joined or killed. When the Tauri app window closes, the Python process becomes orphaned and keeps listening on port 8008. On next launch, the new sidecar will fail to bind because the port is occupied.

**Impact**: Users will see "address already in use" errors after closing and reopening the app, requiring manual process termination.

**Suggested fix**: Store the `_child` handle in Tauri managed state and kill it on the `exit_requested` or `before_quit` event, or use `app.manage()` plus a drop guard.

```rust
// Example approach:
let (mut _rx, child) = sidecar.spawn().expect("...");
app.manage(std::sync::Mutex::new(Some(child)));
// Then on exit: child.kill()
```

### C2. Linux wrapper missing LD_LIBRARY_PATH for GPU libraries

**File**: `scripts/build-sidecar.sh` (wrapper at line 50-58)
**Problem**: The Linux build installs `onnxruntime-gpu==1.24.2` which requires CUDA shared libraries. The macOS wrapper correctly sets `DYLD_LIBRARY_PATH`, but the Linux wrapper does not set `LD_LIBRARY_PATH`. If the bundled venv includes GPU `.so` files or the user has CUDA installed in a non-standard path, the runtime may fail to find them.

**Impact**: GPU inference may silently fall back to CPU on Linux, or fail outright with a dlopen error.

**Suggested fix**: Add to the Linux wrapper:
```bash
export LD_LIBRARY_PATH="$SIDECAR/venv/lib:${LD_LIBRARY_PATH:-}"
```

---

## High Priority

### H1. PBS release URL: `install_only_stripped` variant may not exist for all triples

**File**: `scripts/build-sidecar.sh:14`, `scripts/build-sidecar-macos.sh:20`
**Problem**: The URL uses `install_only_stripped.tar.zst`. The PBS project has published both `install_only` and `install_only_stripped` variants, but availability varies by release. The `20250317` release is plausible (astral-sh/python-build-standalone does monthly-ish releases), but if the stripped variant is missing for a particular triple, the download will fail with a 404.

**Recommendation**: Add a fallback to `install_only` if the stripped URL returns a non-200, or at minimum validate the HTTP status before piping to tar:

```bash
curl -fSL "$URL" -o "$SIDECAR_DIR/python.tar.zst" || {
    echo "[BUILD] Stripped variant not found, trying install_only..."
    URL="${URL/install_only_stripped/install_only}"
    curl -fSL "$URL" -o "$SIDECAR_DIR/python.tar.zst"
}
tar --zstd -xf "$SIDECAR_DIR/python.tar.zst" -C "$SIDECAR_DIR"
```

### H2. Windows script uses `.tar.gz` while Linux/macOS use `.tar.zst`

**File**: `scripts/build-sidecar-win.ps1:12`
**Problem**: The Windows URL uses `install_only.tar.gz` (no `_stripped` suffix either), while the Unix scripts use `install_only_stripped.tar.zst`. This is intentionally different (Windows PBS releases historically use tar.gz), which is correct. However, the Windows variant is `install_only` (not `install_only_stripped`), meaning the Windows sidecar will be significantly larger (~50-100MB bigger) because it includes debug symbols and extra files.

**Impact**: Larger installer size on Windows. Not a correctness issue, but worth noting for size-conscious distribution. If a stripped Windows variant exists for the chosen release, prefer it.

### H3. `tar --zstd` requires zstd support in tar

**File**: `scripts/build-sidecar.sh:25`, `scripts/build-sidecar-macos.sh:31`
**Problem**: `tar --zstd` requires GNU tar >= 1.31 compiled with zstd support, or `zstd` in PATH. On macOS, the default `tar` is BSD tar which does NOT support `--zstd`. The macOS build script will fail on a stock macOS unless the user has GNU tar (e.g., via `brew install gnu-tar`) or uses a different extraction approach.

**Impact**: macOS builds will fail on most developer machines out of the box.

**Suggested fix**: Use `zstd -dc "$file" | tar xf -` or check for GNU tar:
```bash
# Portable alternative:
curl -fSL "$URL" -o /tmp/python.tar.zst
zstd -dc /tmp/python.tar.zst | tar xf - -C "$SIDECAR_DIR"
```

---

## Medium Priority

### M1. Build scripts copy only `server.py` and `modules/`, missing `__init__.py` at root level

**File**: `scripts/build-sidecar.sh:42-43` (all three scripts)
**Problem**: The scripts copy `core/server.py` and `core/modules/` to `$SIDECAR_DIR/app/`. The `server.py` does `sys.path.insert(0, os.path.dirname(__file__))` which makes `$SIDECAR_DIR/app/` the import root. This works for `import modules.globals` etc. However, if any file in `core/` besides `server.py` and `modules/` is needed (e.g., a top-level `__init__.py` or other Python files in `core/`), they would be missing.

**Status**: Reviewed the import tree -- `server.py` only imports from `modules.*` and standard library. All `modules/` submodules also import from `modules.*`. **No issue currently**, but this is fragile. Adding any file to `core/` that `modules/` imports from would silently break the sidecar.

**Recommendation**: Add a comment in the build scripts noting which files are required, or copy all of `core/` to future-proof.

### M2. `@app.on_event("startup")` is deprecated in modern FastAPI

**File**: `core/server.py:54`
**Problem**: FastAPI deprecated `@app.on_event("startup")` in favor of lifespan context managers (since FastAPI 0.93+). While it still works, newer FastAPI versions emit deprecation warnings.

**Impact**: Deprecation warnings in logs. Will eventually break when FastAPI removes legacy support.

**Suggested fix**: Use lifespan:
```python
from contextlib import asynccontextmanager

@asynccontextmanager
async def lifespan(app: FastAPI):
    _init_providers()
    os.makedirs(MODELS_DIR, exist_ok=True)
    yield

app = FastAPI(title="Deep-Live-Cam Server", version="0.1.0", lifespan=lifespan)
```

### M3. No error output if `curl` fails in build scripts

**File**: All three build scripts
**Problem**: `set -euo pipefail` (bash) and `$ErrorActionPreference = 'Stop'` (PS) will abort on failure, but the error message will be the raw curl/tar output. No user-friendly message explains what went wrong.

**Recommendation**: Add a trap or explicit error message after the download step:
```bash
curl -fSL "$URL" -o /tmp/python.tar.zst || { echo "[BUILD] ERROR: Download failed. Check URL: $URL"; exit 1; }
```

### M4. Windows PowerShell script does not build the Rust launcher

**File**: `scripts/build-sidecar-win.ps1:50-54`
**Problem**: The script prints a NOTE telling the user to run `cargo build` separately. This is a manual step that's easy to forget. The Linux/macOS scripts create their wrapper inline. A CI pipeline running only `build-sidecar-win.ps1` would produce an incomplete sidecar with no entry point.

**Recommendation**: Either add `cargo build --release` to the script, or add a check that the `.exe` exists after the script runs:
```powershell
$LauncherExe = "$RepoRoot\app\src-tauri\sidecar-launcher\target\release\deep-live-cam-server.exe"
if (-not (Test-Path $LauncherExe)) {
    Write-Host "[BUILD] Building Rust launcher..."
    cargo build --release --manifest-path "$RepoRoot\app\src-tauri\sidecar-launcher\Cargo.toml"
}
Copy-Item $LauncherExe "$BinariesDir\deep-live-cam-server-x86_64-pc-windows-msvc.exe"
```

---

## Low Priority

### L1. Sidecar Cargo.toml missing `description` and `license` fields

**File**: `app/src-tauri/sidecar-launcher/Cargo.toml`
**Problem**: Missing `description` and `license` metadata. Cargo will warn during `publish` (not applicable here but good hygiene). No functional impact.

### L2. Hardcoded dependency versions may drift

**File**: All three build scripts
**Problem**: `opencv-python==4.10.0.84`, `insightface==0.7.3`, `onnxruntime-gpu==1.24.2` etc. are pinned inline in three separate scripts. If one script is updated and the others are not, the platforms will diverge silently.

**Recommendation**: Extract a shared `requirements-sidecar.txt` and have all scripts reference it:
```bash
"$SIDECAR_DIR/venv/bin/pip" install --no-cache-dir -r "$REPO_ROOT/core/requirements-sidecar.txt"
```

### L3. `du -sh` in build scripts may not exist on minimal containers

**File**: `scripts/build-sidecar.sh:62`, `scripts/build-sidecar-macos.sh:69`
**Impact**: Trivial -- only affects the final informational print. Will error in extremely minimal Docker images but `set -e` would cause an unnecessary failure.

**Fix**: Guard it: `du -sh "$SIDECAR_DIR" 2>/dev/null | cut -f1 || echo "unknown"`

---

## Edge Cases Found by Scout

1. **Orphaned Python process on app close** -- Critical. See C1 above. The `_child` handle is dropped without kill.

2. **Port 8008 conflict** -- If any other process binds 8008, the sidecar will crash at startup with no recovery mechanism. The Tauri `main.rs` does `.expect("failed to spawn sidecar")` which panics the entire app rather than showing a user-friendly error.

3. **PYTHONHOME + venv interaction** -- Setting `PYTHONHOME` to the standalone python while using a venv created from it is unusual. In most Python distributions, `PYTHONHOME` overrides sys.prefix which can conflict with venv activation. With python-build-standalone this generally works because the distribution is self-contained, but it should be tested. If the venv `pyvenv.cfg` has `home = ...python/bin`, and PYTHONHOME points to the same base, it should resolve correctly. However, if the standalone Python's `lib/python3.11` path structure differs from what PYTHONHOME expects, imports will fail silently.

4. **Model auto-download in sandboxed/offline environment** -- The `pre_check()` functions in face_swapper.py, face_enhancer*.py download models on first use via `urllib.request`. If the sidecar runs in a sandboxed or offline environment (common for desktop apps), these downloads will hang or fail without a timeout, potentially freezing the face-swap pipeline indefinitely.

5. **macOS SIP and DYLD_LIBRARY_PATH** -- On macOS, System Integrity Protection strips `DYLD_LIBRARY_PATH` from processes launched by protected binaries. If Tauri's app bundle is signed and launched from /Applications, SIP may strip the env var before it reaches the Python process. The wrapper sets it, but the parent Tauri process may not preserve it.

---

## Positive Observations

- Clean separation of concerns: build scripts, wrapper, launcher, and server are all independent
- `DEEP_LIVE_CAM_MODELS_DIR` env var strategy is well-designed -- single source of truth in `modules/paths.py` with env override
- The Rust launcher is minimal and correct -- no unnecessary dependencies, proper error reporting
- `set -euo pipefail` in bash scripts catches errors early
- PBS version and Python version choices are reasonable (3.11 is the sweet spot for ML compatibility)
- The wrapper scripts use `exec` to replace the shell process, avoiding zombie shell parents

---

## Recommended Actions (Prioritized)

1. **[Critical]** Fix orphaned sidecar process on app exit (C1) -- store child handle, kill on exit
2. **[Critical]** Add `LD_LIBRARY_PATH` to Linux wrapper (C2) -- one-line fix
3. **[High]** Fix macOS `tar --zstd` compatibility (H3) -- use `zstd -dc | tar xf -`
4. **[High]** Add PBS URL fallback or validation (H1)
5. **[Medium]** Build the Rust launcher inside the Windows script (M4)
6. **[Medium]** Extract shared `requirements-sidecar.txt` (L2) to prevent version drift
7. **[Medium]** Migrate `on_event("startup")` to lifespan (M2)
8. **[Low]** Add better error messages on download failure (M3)

---

## Metrics

| Metric | Value |
|--------|-------|
| Type Coverage (Python) | N/A (untyped, consistent with codebase style) |
| Type Coverage (Rust) | 100% (Rust enforces) |
| Test Coverage | 0% (no tests for build scripts or launcher) |
| Linting Issues | 1 (FastAPI deprecation warning) |
| Security Issues | 0 (no secrets, localhost-only binding, CORS is wide but expected for local sidecar) |

---

## Unresolved Questions

1. Has the exact PBS URL (`cpython-3.11.11+20250317-{triple}-install_only_stripped.tar.zst`) been validated to return 200 for all three target triples?
2. Is the `PYTHONHOME` + venv combination tested with python-build-standalone specifically? The interaction is non-standard.
3. Should there be a health-check retry loop in `main.rs` after spawning the sidecar, to confirm the server is ready before the frontend tries to connect?
4. What is the plan for model download on first run in offline/air-gapped environments?
