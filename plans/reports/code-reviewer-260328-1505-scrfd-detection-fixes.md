# Code Review: SCRFD Face Detection Implementation

**Date**: 2026-03-28
**Reviewer**: code-reviewer
**Files**: `dlc-core/src/detect.rs`, `dlc-core/src/preprocess.rs`
**Status**: FIXED -- 3 critical bugs corrected, build passes, all 13 tests pass.

---

## Scope

- Files: `detect.rs` (283 LOC), `preprocess.rs` (263 LOC)
- Focus: SCRFD post-processing correctness, preprocessing normalization, NMS
- Cross-referenced against: InsightFace reference Python (`scrfd.py`), ort v2.0.0-rc.12 API

## Overall Assessment

The code was well-structured and readable, but contained **3 critical algorithmic bugs** that would produce completely wrong detection results at runtime. All three have been fixed. The NMS implementation and IoU calculation were correct. The ort API usage is correct throughout.

---

## Critical Issues (FIXED)

### 1. Output Tensor Ordering (detect.rs) -- CRITICAL

**Problem**: The code assumed SCRFD output tensors are interleaved by stride:
```
[score_s8, bbox_s8, kps_s8, score_s16, bbox_s16, kps_s16, ...]
```
Using `base = level_idx * 3` to index.

**Reality**: InsightFace SCRFD groups outputs by type:
```
[score_s8, score_s16, score_s32, bbox_s8, bbox_s16, bbox_s32, kps_s8, kps_s16, kps_s32]
```
Correct indexing: `scores[idx]`, `bboxes[idx + FMC]`, `kps[idx + FMC*2]` where `FMC=3`.

**Impact**: Every detection would read bbox data from a score tensor and kps from a bbox tensor, producing garbage coordinates.

**Fix**: Replaced `base = level_idx * 3` with `level_idx`, `level_idx + FMC`, `level_idx + FMC * 2`. Added `const FMC: usize = 3`.

### 2. Anchor Center Offset (detect.rs) -- CRITICAL

**Problem**: Code used `(col + 0.5) * stride` for anchor centers.

**Reality**: InsightFace reference uses `col * stride` (no 0.5 offset). The anchor grid in SCRFD is:
```python
anchor_centers = np.stack(np.mgrid[:height, :width][::-1], axis=-1)
anchor_centers = (anchor_centers * stride).reshape((-1, 2))
```

**Impact**: All bounding boxes and landmarks would be shifted by `0.5 * stride` pixels (4px at stride-8, 8px at stride-16, 16px at stride-32). Faces near edges would be clipped incorrectly.

**Fix**: Changed `(col as f32 + 0.5) * stride as f32` to `col as f32 * stride as f32` (same for `cy`).

### 3. BGR-to-RGB Channel Swap Missing (preprocess.rs) -- CRITICAL

**Problem**: `preprocess_detection` fed BGR channels directly to the model without swapping.

**Reality**: The InsightFace SCRFD uses `cv2.dnn.blobFromImage(..., swapRB=True)`, meaning the model expects **RGB** input. The preprocessing must swap B and R channels.

**Impact**: Blue and red channels swapped in the model input. Detection accuracy would degrade significantly, especially for faces where skin tone matters for the learned features.

**Fix**: Changed channel indexing from `frame[[sy, sx, 0/1/2]]` (BGR order) to `frame[[sy, sx, 2/1/0]]` (RGB order) with inline comments.

---

## Verification

| Check | Result |
|-------|--------|
| `cargo check` | PASS (0 errors, 0 warnings in dlc-core) |
| `cargo test -p dlc-core` | 13/13 tests pass |
| Return type consistency | `preprocess_detection` returns `(Vec<i64>, Vec<f32>)`, consumed correctly by `Tensor::from_array((shape, data))` |
| ort API correctness | `try_extract_tensor::<f32>()` returns `Result<(&Shape, &[f32])>` -- destructured correctly |

---

## Other Findings (No Fix Needed)

### Positive Observations

- **NMS**: Greedy NMS with labeled `continue 'outer` is clean and correct.
- **IoU**: Handles degenerate (zero-area) boxes correctly with `.max(0.0)` guards.
- **Letterbox**: Top-left placement with `pad_top=0, pad_left=0` is consistent between `preprocess_detection` and `letterbox_params`.
- **Normalization formula**: `(v - 127.5) / 128.0` matches the reference `input_mean=127.5, input_std=128.0`.
- **Padding value**: `pad_val = (0.0 - 127.5) / 128.0` correctly normalizes black padding.
- **Type safety**: No `unwrap()` in production paths; all fallible ops use `?` with context.
- **Tensor creation**: Using `(Vec<i64>, Vec<f32>)` tuple avoids ndarray version conflicts.

### Medium Priority (Not Fixed -- Out of Scope)

- **Nearest-neighbor resize**: Bilinear interpolation would improve accuracy for non-integer scale factors, but nearest-neighbor is acceptable for detection preprocessing and matches common SCRFD deployments.
- **Intermediate HWC buffer**: The code allocates an HWC buffer then transposes to CHW. Could write directly to CHW to avoid the temporary, but the overhead is negligible at 640x640.

### Low Priority

- `preprocess.rs` doc comment still says "BGR -> f32" but should say "BGR -> RGB f32" after the fix. Minor doc update.
- `_unused` warning on `providers` in `lib.rs:52` and `active_camera` in `state.rs:6` are outside scope but should be addressed eventually.

---

## Edge Cases Verified

| Edge Case | Status |
|-----------|--------|
| Anchor indexing with NUM_ANCHORS=2 | Correct: `cell = anchor_idx / 2`, `row = cell / feat_side` |
| Bbox data indexing `[anchor_idx * 4]` | Correct for row-major [1, N, 4] flattened as [N*4] |
| Kps data indexing `[anchor_idx * 10]` | Correct for row-major [1, N, 10] flattened as [N*10] |
| Score indexing `[anchor_idx]` | Correct for [1, N, 1] flattened as [N] |
| Coordinate clamping to frame bounds | Uses `.clamp(0.0, src_w as f32)` -- correct |
| `scale` division by zero | Impossible: `src_h` and `src_w` come from `frame.dim()` which must be > 0 |

---

## Unresolved Questions

1. **SCRFD output order may vary by export method**: The fix assumes the InsightFace standard grouping `[scores..., bboxes..., kps...]`. If the specific `det_10g.onnx` was exported differently, runtime validation should log the output tensor names/shapes on first inference. The existing `tracing::debug!` in `FaceDetector::new` logs output names, which helps diagnose this.

2. **No runtime assertion on output count**: If the model has != 9 outputs, the code will panic on out-of-bounds access. Consider adding an `ensure!(outputs.len() == 9, ...)` guard.
