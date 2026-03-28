## Code Review Summary: CI/CD Workflows and Build Scripts

### Scope
- Files: `release.yml`, `ci.yml`, `build-sidecar.sh`, `build-sidecar-macos.sh`, `build-sidecar-win.ps1`, `tauri.conf.json`
- LOC: ~230
- Focus: CI/CD correctness, PBS URL validity, cross-platform compatibility

### Overall Assessment

Three build-breaking bugs found and fixed. The PBS download URLs used `.tar.zst` extensions but the actual python-build-standalone assets are `.tar.gz`. This would have caused every Linux and macOS release build to fail at the sidecar download step. A cross-platform shell issue in the pubkey guard step was also fixed.

---

### Critical Issues (Fixed)

**1. PBS URL file extension mismatch -- Linux (`build-sidecar.sh`)**
- URL ended with `install_only_stripped.tar.zst`; actual asset is `install_only_stripped.tar.gz`
- Extraction used `tar --zstd`; changed to `tar -xzf`
- Impact: 100% build failure on Linux release

**2. PBS URL file extension mismatch -- macOS (`build-sidecar-macos.sh`)**
- Same `.tar.zst` vs `.tar.gz` mismatch
- Extraction used `zstd -dc | tar xf -`; changed to `tar -xzf`
- Impact: 100% build failure on macOS release

**3. Pubkey guard step fails on Windows (`release.yml`)**
- Used `grep -q` in a `run:` block with no `shell:` directive
- Windows runners default to `pwsh`, where `grep` is not available
- Fix: Added `shell: bash` (Git Bash is present on all GitHub Actions Windows runners)

### High Priority

**4. Unnecessary `zstd` dependency in `release.yml` (Fixed)**
- Installed `zstd` on Linux (`apt-get`) and macOS (`brew install zstd`) solely for PBS extraction
- Since PBS assets are `.tar.gz`, `zstd` is not needed
- Removed the `zstd` package from Linux deps and removed the entire macOS dependency step
- Saves ~15s of CI time per macOS build

### Medium Priority

**5. PBS cache step is a no-op**
- `release.yml` caches `/tmp/pbs-cache` but no build script reads from or writes to that path
- The scripts download directly via `curl` every time
- Recommendation: Either remove the cache step, or modify scripts to check `$PBS_CACHE_DIR` before downloading

**6. Windows build script uses `install_only` instead of `install_only_stripped`**
- Linux/macOS use `install_only_stripped.tar.gz` (smaller, debug symbols removed)
- Windows uses `install_only.tar.gz` (larger, includes debug symbols)
- Both variants exist on PBS `20250317` for `x86_64-pc-windows-msvc`
- Recommendation: Consider switching Windows to `install_only_stripped.tar.gz` for smaller bundle size (~20-30% reduction)

### Checklist Results

| # | Check | Result |
|---|-------|--------|
| 1 | Matrix builds per-platform bundles correctly | PASS -- `deb,appimage` / `dmg` / `msi,nsis` |
| 2 | Pubkey guard step correct | FIXED -- added `shell: bash` for Windows |
| 3 | CI creates sidecar placeholder for cargo check | PASS -- correct triple `x86_64-unknown-linux-gnu`, `chmod +x` |
| 4 | Build scripts reference `core/` not `Deep-Live-Cam/` | PASS -- all use `$REPO_ROOT/core/server.py` and `core/modules` |
| 5 | PBS URLs realistic (release tag format) | FIXED -- tag `20250317` exists, but file extension was wrong |
| 6 | Windows script notes Rust launcher build step | PASS -- lines 50-54 document the cargo build requirement |
| 7 | Updater endpoint points to Deep-Forge | PASS -- `PhenixStar/Deep-Forge` confirmed to exist on GitHub |

### Positive Observations

- `fail-fast: false` in the release matrix prevents one platform failure from canceling others
- `set -euo pipefail` in both shell scripts -- strict error handling
- macOS script correctly detects `arm64`/`x86_64` and maps to PBS triples
- macOS wrapper correctly notes that `DYLD_LIBRARY_PATH` is stripped by SIP
- Windows correctly delegates to a compiled Rust launcher instead of a batch wrapper
- Updater pubkey is a real minisign key (decoded successfully), not a placeholder
- Secrets (`TAURI_SIGNING_PRIVATE_KEY`) are properly referenced via `${{ secrets.* }}`
- `timeout-minutes: 60` prevents runaway builds

### Files Modified

1. `/raid/projects/deep-wcam/scripts/build-sidecar.sh` -- URL extension `.tar.zst` -> `.tar.gz`, extraction `tar --zstd` -> `tar -xzf`
2. `/raid/projects/deep-wcam/scripts/build-sidecar-macos.sh` -- URL extension `.tar.zst` -> `.tar.gz`, extraction `zstd -dc | tar` -> `tar -xzf`
3. `/raid/projects/deep-wcam/.github/workflows/release.yml` -- added `shell: bash` to pubkey guard, removed `zstd` from Linux deps, removed macOS deps step

### No Changes Needed

4. `/raid/projects/deep-wcam/.github/workflows/ci.yml` -- correct as-is
5. `/raid/projects/deep-wcam/scripts/build-sidecar-win.ps1` -- correct as-is (minor optimization opportunity noted)
6. `/raid/projects/deep-wcam/app/src-tauri/tauri.conf.json` -- correct as-is, real pubkey, correct endpoint

### Unresolved Questions

1. The PBS cache step in `release.yml` (lines 68-73) caches a path that no script uses. Should it be wired up or removed?
2. Should the Windows build script switch to `install_only_stripped.tar.gz` for smaller bundles?
