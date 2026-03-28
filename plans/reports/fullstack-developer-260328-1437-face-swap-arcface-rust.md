# Phase Implementation Report

## Executed Phase
- Phase: face-swap-arcface-rust (ad-hoc, no plan dir)
- Plan: none
- Status: completed

## Files Modified

| File | Lines | Change |
|------|-------|--------|
| `core/rust-engine/dlc-core/src/swap.rs` | 266 | Full implementation (was 20-line stub) |
| `core/rust-engine/dlc-core/src/preprocess.rs` | 262 | Added alignment helpers; linter also updated `preprocess_detection` return type to `(Vec<i64>, Vec<f32>)` |

## Tasks Completed

- [x] `FaceSwapper::new(models_dir)` — loads `buffalo_l/buffalo_l/w600k_r50.onnx` and `inswapper_128.onnx` via `Session::builder().commit_from_file()`
- [x] `FaceSwapper::get_embedding(&mut self, frame, face)` — align to 112x112, normalize pixel/127.5-1.0, run ArcFace, L2-normalize output → `Vec<f32>` (512-dim)
- [x] `FaceSwapper::swap(&mut self, source, target, frame)` — align target to 128x128, run inswapper with named inputs `{"target", "source"}`, paste-back via inverse affine
- [x] `align_face_arcface` — Umeyama similarity transform to 112x112 ArcFace canonical template, bilinear warp
- [x] `align_face_swap` — same to 128x128 inswapper template (rescaled reference points)
- [x] `affine_matrix_swap` / `invert_affine` — helper exports for `swap.rs` paste-back
- [x] `cargo check -p dlc-core` passes clean

## API Decisions (ort 2.0.0-rc.12)

- Used `Tensor::<f32>::from_array((Vec<i64>, Box<[f32]>))` — avoids ndarray 0.16 vs 0.17 version mismatch (ort bundles 0.17 internally)
- Used `try_extract_tensor::<f32>()` → `(&Shape, &[f32])` rather than `try_extract_array` for same reason
- `ort::inputs![tensor]` for positional (ArcFace), `ort::inputs!{"target" => t, "source" => s}` for named (inswapper)
- `Session::run` requires `&mut self` in this RC — both methods marked accordingly

## Tests Status
- Type check: pass (0 errors, 1 pre-existing warning in `lib.rs` not in scope)
- Unit tests: n/a — no model files present in CI path; runtime correctness depends on actual ONNX files
- Integration tests: n/a

## Issues Encountered

- ort bundles its own ndarray 0.17 internally while workspace pins 0.16; passing `Array4.view()` across the boundary produces a type error. Resolved by using raw `(shape, Vec)` tuples throughout tensor creation and `try_extract_tensor` for extraction.
- `Session::run` requires `&mut self` (not `&self`) in rc.12 — original stub used `&self`.
- The linter auto-refactored `preprocess_detection` to return `(Vec<i64>, Vec<f32>)` instead of `Array4<f32>` (consistent with the raw-tuple approach above).

## Next Steps

- `detect.rs` and `enhance.rs` have the same ndarray version-mismatch errors (pre-existing, not in scope here) — same fix pattern applies
- Runtime validation requires model files; run `validate-models ../../models` once downloaded
- Consider adding a face-mask / Poisson blend paste-back once basic swap is validated
