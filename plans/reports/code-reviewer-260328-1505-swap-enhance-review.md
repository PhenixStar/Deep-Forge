# Code Review: swap.rs + enhance.rs

**Reviewer**: code-reviewer
**Date**: 2026-03-28
**Scope**: `dlc-core/src/swap.rs`, `dlc-core/src/enhance.rs`
**LOC**: 267 (swap) + 335 (enhance) = 602 total

## Overall Assessment

Both files are well-structured, correct in their core logic, and properly use the `ort 2.0.0-rc.12` API. One failing test was found and fixed. No compilation errors. No security issues. The code is production-ready with minor observations noted below.

## Checklist Results

### 1. ArcFace normalization: PASS
`bgr_hwc_to_rgb_nchw_normalized()` (swap.rs:150-170) correctly applies `pixel / 127.5 - 1.0`, producing the standard [-1, 1] range expected by ArcFace w600k_r50.

### 2. Inswapper named inputs: PASS
swap.rs:120-123 uses `ort::inputs! { "target" => target_tensor, "source" => source_tensor }` which is the correct named-inputs form. The macro returns `Vec<(Cow<str>, SessionInputValue)>` (no `?` needed), and `Session::run` accepts it via `From<Vec<(K, V)>> for SessionInputs`.

### 3. GFPGAN preprocess [-1,1] range: PASS
enhance.rs:70-74 uses `(pixel / 255.0 - 0.5) / 0.5` which is algebraically equivalent to `pixel / 127.5 - 1.0`, correctly mapping [0, 255] to [-1, 1]. The postprocess (line 90-92) correctly inverts: `(v + 1.0) / 2.0 * 255.0`.

### 4. Paste-back feathered alpha blend: PASS (with test fix)
The feathering logic in enhance.rs:124-153 is correct. Alpha ramps linearly from 0.0 at the crop edge to 1.0 inside the border zone (5% of crop dimension). The bilinear interpolation in swap.rs:245-254 is also correct.

### 5. ort v2 RC API patterns: ALL PASS

| Pattern | Location | Status |
|---------|----------|--------|
| `Session::builder()?.commit_from_file(path)?` | swap.rs:37-46, enhance.rs:175-178 | Correct |
| `Tensor::from_array((shape_vec, data_vec))?` | swap.rs:62,109,115; enhance.rs:220 | Correct |
| `session.run(ort::inputs![tensor])` (no `?` on macro) | swap.rs:67,120; enhance.rs:226 | Correct |
| `try_extract_tensor::<f32>()` returns `(&Shape, &[f32])` | swap.rs:72; enhance.rs:231 | Correct |

The `Shape` type derefs to `SmallVec<[i64; 4]>`, so `.len()` and `[2]` indexing (enhance.rs:235-236) work correctly.

### 6. Unused imports / dead code: PASS
No unused imports in either reviewed file. The `_providers` warning in `lib.rs` was fixed (not in scope but adjacent).

## Issues Found and Fixed

### BUG FIX: Failing test `paste_back_border_blends` (Medium)

**File**: `enhance.rs:325-334`
**Problem**: The test checked pixel `[25, 25]` (the very first pixel of the crop at `dy=0, dx=0`) and asserted `corner > 50`. But the feathering alpha at `(0, 0)` is `(0/border_h) * (0/border_w) = 0.0`, so the pixel correctly stays at its original value of 50. The assertion was wrong, not the algorithm.
**Fix**: Changed the test to:
- Assert edge pixel `[25, 25]` equals 50 (alpha=0, unchanged)
- Assert pixel `[26, 26]` (one step inside the feather zone) is between 50 and 200 (partial blend)

### WARNING FIX: Unused `providers` parameter (Low)

**File**: `lib.rs:52`
**Fix**: Renamed to `_providers` to suppress warning without changing the public API.

## Edge Cases Reviewed

| Edge Case | Verdict |
|-----------|---------|
| Zero-length embedding | Guarded by `ensure!(raw.len() == 512)` in swap.rs:75 |
| Missing embedding on source face | Guarded by `.context()` on `Option::as_ref()` in swap.rs:95-98 |
| Degenerate crop bounds | Guarded by `x1 <= x0 || y1 <= y0` bail in enhance.rs:207-209 |
| Sub-pixel boundary in bilinear interp | `.min(pw-1)` / `.min(ph-1)` clamps prevent OOB in swap.rs:240-241 |
| L2 norm near zero | `max(1e-10)` prevents division by zero in swap.rs:265 |
| Singular affine matrix | `det.abs() > 1e-10` guard in preprocess.rs:174 |
| Frame shape mismatch | `ensure!(img.shape() == [h,w,3])` in both normalization helpers |

## Potential Improvements (not bugs)

1. **Performance**: The paste-back in swap.rs iterates over the entire frame (lines 226-257) even though the swapped patch only covers a small region. A bounding-box pre-computation on the forward affine matrix could skip ~95% of pixels. This is O(frame_area) vs O(patch_area).

2. **Preprocess normalization consistency**: `swap.rs` uses two different normalizations -- `pixel/127.5 - 1.0` for ArcFace and `pixel/255.0` for inswapper target. Both are correct for their respective models, but a brief doc-comment on `bgr_hwc_to_rgb_nchw_01` noting "inswapper expects [0,1] not [-1,1]" would help future readers.

3. **`enhance.rs` `preprocess` algebraic simplification**: `(r / 255.0 - 0.5) / 0.5` is equivalent to `r / 127.5 - 1.0`. Using the simpler form would match swap.rs and save one division. Purely cosmetic.

## Metrics

- **Compilation**: Clean (0 errors, 0 warnings in reviewed files)
- **Tests**: 13/13 pass (was 12/13 before fix)
- **ort API compliance**: 6/6 patterns verified against 2.0.0-rc.12 source
- **Unsafe code**: None
- **Linting issues in reviewed files**: 0

## Positive Observations

- Clean separation of concerns: alignment in `preprocess.rs`, inference in `swap.rs`/`enhance.rs`
- Consistent use of `anyhow::Context` for error chain enrichment
- No `unsafe` blocks; pure safe Rust throughout
- Avoids ndarray version conflicts by using `(Vec<i64>, Vec<f32>)` tuple form for tensor creation
- Good test coverage for the enhancement pipeline helpers

## Unresolved Questions

1. The `FMC` constant in `detect.rs:29` is dead code -- should it be removed or is it planned for use?
2. `dlc-server` has unused `Serialize` import and dead `active_camera` field -- separate cleanup?
