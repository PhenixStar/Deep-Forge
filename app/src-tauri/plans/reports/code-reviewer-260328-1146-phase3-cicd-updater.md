## Code Review Summary -- Phase 3 (CI/CD + Installers + Auto-Updater)

### Scope
- Files: 8 (2 new workflows, 1 new capability, 3 modified Tauri config/code, 1 modified package.json, 1 modified App.tsx)
- Supporting files reviewed: 3 sidecar build scripts, 1 sidecar launcher
- Focus: CI/CD correctness, updater wiring, security

### Overall Assessment

Solid implementation. The release matrix, sidecar build ordering, CI placeholder trick, and updater wiring are all structurally correct. There are a handful of issues ranging from a dead cache (High) to version drift (Medium) that should be addressed before the first real release tag push.

---

### Critical Issues

None.

---

### High Priority

**H1. PBS cache in release.yml is inert (wasted CI time, no actual caching)**

`release.yml:63-67` caches `/tmp/pbs-cache`, but none of the three sidecar build scripts read from or write to that path. Every release build will re-download the ~60-100 MB python-build-standalone tarball.

Fix: Either (a) update the build scripts to check/use `/tmp/pbs-cache` as a download cache, or (b) remove the `actions/cache` step entirely so it does not create a false sense of optimization.

```bash
# Example fix in build-sidecar.sh: add before the curl
CACHE_DIR="/tmp/pbs-cache"
CACHED="$CACHE_DIR/cpython-${PYTHON_VERSION}+${PBS_RELEASE}-${TRIPLE}.tar.zst"
mkdir -p "$CACHE_DIR"
if [ -f "$CACHED" ]; then
  echo "[BUILD] Using cached PBS download"
  zstd -dc "$CACHED" | tar xf - -C "$SIDECAR_DIR"
else
  curl -fSL "$URL" -o "$CACHED"
  zstd -dc "$CACHED" | tar xf - -C "$SIDECAR_DIR"
fi
```

**H2. macOS runner `macos-14` is ARM64 only -- no Intel build**

`macos-14` runs on Apple Silicon. The sidecar script auto-detects `uname -m` and builds for `aarch64-apple-darwin`. There is no matrix entry for Intel macOS. If Intel users are in scope, add a `macos-13` (x86_64) matrix entry.

If ARM64-only is intentional for now, this is fine, but document the decision.

**H3. `tauri-action@v0` -- pinned to major 0**

Using `tauri-apps/tauri-action@v0` means you track the latest `0.x` release. This is the correct tag for Tauri v2 support as of today, but confirm this is intentional. `@v0` auto-advances on minor/patch bumps, which could break CI. Consider pinning to a specific SHA for reproducibility in production.

**H4. updater `pubkey` placeholder will cause runtime crash**

`"pubkey": "UPDATER_PUBKEY_PLACEHOLDER"` -- if someone pushes a `v*` tag before replacing this, the published `latest.json` will reference a signature that cannot verify, and the updater check in App.tsx will throw. The `catch {}` block swallows it silently, so it won't crash the app, but updates will silently never work.

Recommendation: Add a CI guard that fails the release build if `UPDATER_PUBKEY_PLACEHOLDER` is still present in `tauri.conf.json`:

```yaml
- name: Verify updater pubkey is set
  run: |
    if grep -q 'UPDATER_PUBKEY_PLACEHOLDER' app/src-tauri/tauri.conf.json; then
      echo "ERROR: Replace UPDATER_PUBKEY_PLACEHOLDER with real key before releasing"
      exit 1
    fi
```

---

### Medium Priority

**M1. Version mismatch: `package.json` is `1.0.0`, `tauri.conf.json` is `0.1.0`**

Tauri uses `tauri.conf.json` version as the authoritative app version. The `package.json` says `1.0.0`. These should match to avoid confusion, especially since the updater compares versions from `latest.json` against the app version in `tauri.conf.json`.

**M2. `release.yml` missing `GITHUB_TOKEN` for `tauri-action`**

`tauri-apps/tauri-action@v0` needs a `GITHUB_TOKEN` to create the GitHub release and upload assets. The workflow has `permissions: contents: write` but does not pass `GITHUB_TOKEN` as an env var. `tauri-action` should pick it up automatically via `${{ github.token }}`, but verify this works, or explicitly set:

```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

**M3. Missing `--bundles` for macOS -- `nsis` and `msi` will fail on macOS/Linux**

`release.yml:100` passes `args: --bundles deb,appimage,msi,nsis,dmg` to every matrix runner. Windows-only formats (`msi`, `nsis`) will error on Linux/macOS, and Linux-only formats (`deb`, `appimage`) will error on macOS/Windows. Tauri v2 may skip unsupported formats gracefully, but this should be tested. Alternatively, move bundle format specification into the matrix:

```yaml
matrix:
  include:
    - os: ubuntu-22.04
      bundles: deb,appimage
    - os: macos-14
      bundles: dmg
    - os: windows-latest
      bundles: msi,nsis
```

Then: `args: --bundles ${{ matrix.bundles }}`

**M4. No `relaunch()` after `downloadAndInstall()` in App.tsx**

In Tauri v2, `update.downloadAndInstall()` downloads and installs the update but does not restart the app. The user is left in the old version until they manually quit and reopen. The typical pattern is:

```ts
import { relaunch } from "@tauri-apps/plugin-process";
await update.downloadAndInstall();
await relaunch();
```

This requires adding `@tauri-apps/plugin-process` and `tauri-plugin-process` (Rust side), plus the `process:allow-restart` capability.

Alternatively, tell the user "Update installed. Please restart the app." via the confirm dialog.

**M5. `ci.yml` does not run `cargo clippy` or `cargo test`**

The CI only runs `cargo check`, which catches compilation errors but not warnings or logic bugs. Consider adding `cargo clippy -- -D warnings` and any Rust tests if they exist.

**M6. `ci.yml` Python check is minimal**

Only two files are syntax-checked (`core/server.py`, `core/modules/camera_utils.py`). If there are other Python files in `core/`, they are not validated. Consider:

```yaml
- name: Python syntax check
  run: python3 -m compileall core/ -q
```

---

### Low Priority

**L1. `release.yml` Linux deps include `zstd` but `build-sidecar.sh` uses `tar --zstd`**

The `tar --zstd` flag requires `tar >= 1.31` with zstd support. Ubuntu 22.04 includes this, so it works. But the macOS script uses `zstd -dc | tar xf -` (piped), while Linux uses `tar --zstd` (flag). Minor inconsistency, both work.

**L2. `app/src-tauri/capabilities/default.json` -- `shell:allow-execute` may be overly broad**

`shell:allow-execute` grants the ability to execute arbitrary commands. If the app only needs sidecar spawning, `shell:allow-spawn` with the scoped sidecar config should suffice. Verify whether `shell:allow-execute` is actually needed.

**L3. Windows sidecar build does not use stripped Python**

Linux/macOS use `install_only_stripped.tar.zst`, Windows uses `install_only.tar.gz` (not stripped). This inflates the Windows bundle size. Check if a stripped Windows build is available from python-build-standalone.

---

### Edge Cases Found

1. **Sidecar crash on startup** -- If the sidecar fails to spawn (line 29 in `main.rs`), the app panics with `expect()`. This was pre-existing, not Phase 3, but the CI/release pipeline now ships this behavior to users. Consider graceful error handling post-Phase 3.

2. **Concurrent release tag pushes** -- If two `v*` tags are pushed in quick succession, both trigger release builds. The `releaseDraft: true` mitigates this (manual publish required), but duplicate drafts may be confusing.

3. **macOS code signing not configured** -- No `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, or notarization secrets are referenced. macOS builds will be unsigned, triggering Gatekeeper warnings. This is likely intentional for Phase 3 but should be Phase 4.

4. **`TAURI_SIGNING_PRIVATE_KEY` set at workflow env level** -- This means the secret is available to ALL steps, including the sidecar build steps that do not need it. Scope it to only the Tauri build step for least-privilege.

---

### Positive Observations

- Sidecar build ordering is correct: build scripts run BEFORE `tauri-action`, ensuring the binary exists.
- CI placeholder sidecar trick (`touch` + `chmod +x`) is clean and avoids needing a full sidecar build for `cargo check`.
- `fail-fast: false` on the release matrix is correct -- a Linux failure should not cancel the macOS build.
- Dynamic import of the updater in App.tsx avoids bundling updater code in dev builds and handles the web context gracefully.
- Windows sidecar launcher is well-designed -- resolves paths relative to exe, proper error messages.
- `releaseDraft: true` is a safe default, requiring manual review before publishing.
- `includeUpdaterJson: true` automatically generates the `latest.json` manifest for the updater endpoint.

---

### Recommended Actions (Priority Order)

1. **H4** Add CI guard against placeholder pubkey in release builds
2. **H1** Fix PBS cache to actually be used by build scripts, or remove it
3. **M3** Per-platform bundle formats in the matrix to avoid cross-platform bundle errors
4. **M1** Sync version across `package.json` and `tauri.conf.json`
5. **M4** Add `relaunch()` after update install, or notify user to restart
6. **M2** Explicitly pass `GITHUB_TOKEN` to `tauri-action`
7. **H2** Decide on Intel macOS support and document
8. **L2** Remove `shell:allow-execute` if not needed

### Metrics

- Type Coverage: N/A (no new TypeScript types added; existing types adequate)
- Test Coverage: 0% (no tests for updater logic or CI scripts)
- Linting Issues: Not run (no Rust clippy in CI)

### Unresolved Questions

1. Is Intel macOS (x86_64) a target for this release? If so, `macos-13` matrix entry needed.
2. Does `tauri-action@v0` auto-inject `GITHUB_TOKEN`, or must it be explicit?
3. Should the updater do a silent background download, or always prompt the user? Current UX uses `window.confirm()` which blocks the UI thread.
4. Are there additional Python files beyond `server.py` and `camera_utils.py` that should be syntax-checked in CI?
