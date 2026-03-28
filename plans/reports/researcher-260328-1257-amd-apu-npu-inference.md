# Research: AMD Ryzen AI 9 HX 370 — Best Inference Approach for Deep-Live-Cam

**Date:** 2026-03-28
**Target:** GPD Sweep — Ryzen AI 9 HX 370, Radeon 890M (16 CUs RDNA 3.5), XDNA2 NPU (50 TOPS), 64GB shared LPDDR5X

---

## Executive Summary

Three viable execution providers exist for this hardware. **DirectML on Radeon 890M** is the recommended primary path — no model modification needed, best FPS for FP16/FP32 models. **VitisAI on XDNA2 NPU** offers best power efficiency but requires INT8 quantization, making it unsuitable for face swap accuracy without careful calibration. **Hybrid approach** (NPU for detection, iGPU for swap) is the ideal end-state but adds implementation complexity.

**Recommended strategy for v1:** DirectML-only. Revisit NPU offloading in v2.

---

## Performance Comparison (Estimated, 720p)

| Provider | Hardware | Precision | Est. FPS | Power | Setup Complexity |
|----------|----------|-----------|----------|-------|-----------------|
| **DirectML** | Radeon 890M iGPU | FP16/FP32 | **12-18** (conservative) / **45-60** (optimized) | Medium | Low (pip install) |
| VitisAI | XDNA2 NPU | INT8 only | 20-35 | Ultra-low | High (SDK + quantization) |
| CPU | Zen 5 AVX-512 | FP32 | 4-7 | High | None |
| Hybrid | NPU + iGPU | Mixed | 50-70 (theoretical) | Low | Very High |

---

## Approach 1: DirectML (RECOMMENDED for v1)

### Why
- No model modification needed — runs existing FP32/FP16 ONNX models as-is
- `pip install onnxruntime-directml` — one package
- 64GB unified memory = no VRAM bottleneck
- Mature, stable on Windows 11

### Setup
```bash
pip install onnxruntime-directml
```

### Provider detection code
```python
import onnxruntime as ort

providers = ort.get_available_providers()
if "DmlExecutionProvider" in providers:
    execution_providers = ["DmlExecutionProvider", "CPUExecutionProvider"]
elif "CUDAExecutionProvider" in providers:
    execution_providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
else:
    execution_providers = ["CPUExecutionProvider"]
```

### BIOS optimization
Set "UMA Frame Buffer Size" to 8GB or 16GB (dedicated iGPU pool before hitting shared).

---

## Approach 2: VitisAI on NPU (FUTURE — requires work)

### Blockers for face swap use case
1. **INT8 quantization required** — XDNA2 NPU is fixed-point, cannot run FP32/FP16 natively
2. **Insightface models use unsupported ops** — `Resize`, `NonMaxSuppression` cause graph partitioning (NPU↔CPU bouncing = high latency)
3. **Accuracy loss** — face recognition embeddings degrade significantly with naive INT8 quantization
4. **Requires AMD Ryzen AI Software SDK** (conda env, vaip_config.json, driver 32.0.203.280+)

### When it makes sense
- Face detection (SCRFD/RetinaFace) quantizes well to INT8
- Power efficiency matters (battery laptop use case)
- Could offload detection to NPU while iGPU handles swap

### Setup (if pursuing)
```bash
# Requires Ryzen AI Software v1.5+ installed
conda activate ryzen-ai-1.5.0
pip install onnxruntime-vitisai amd-quark

# Quantize model
python -m quark.onnx --input_model inswapper_128.onnx --output_model inswapper_128_int8.onnx --calibration_data ./calib_images/
```

---

## Approach 3: AMD FLM (FastFlowLM)

**Not applicable for face swap.** FLM is NPU-native runtime for LLMs/VLMs only (Llama, Qwen, Phi). Uses proprietary `.q4nx` format. Cannot run CNN/face-swap ONNX models.

---

## Approach 4: ROCm on Windows

**Not recommended.** ROCm/HIP on Windows validated primarily for Radeon Pro and RX 7000/8000 discrete GPUs. Not stable on 890M iGPU. DirectML is the correct path for integrated Radeon.

---

## Implementation Plan for Deep-Live-Cam

### Changes needed in `core/server.py`

```python
def _init_providers():
    import onnxruntime as ort
    providers = ort.get_available_providers()
    if "CUDAExecutionProvider" in providers:
        globals.execution_providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
    elif "DmlExecutionProvider" in providers:
        globals.execution_providers = ["DmlExecutionProvider", "CPUExecutionProvider"]
    else:
        globals.execution_providers = ["CPUExecutionProvider"]
```

### Changes needed in `scripts/build-sidecar-win.ps1`

Replace `onnxruntime-gpu` with dual install:
```powershell
# Install DirectML for AMD/Intel iGPUs + CUDA for NVIDIA
pip install onnxruntime-directml
# Note: onnxruntime-directml and onnxruntime-gpu conflict — pick one per build
# For universal Windows build: use onnxruntime-directml (works on all GPUs)
```

**Decision:** Ship `onnxruntime-directml` as the Windows default. DirectML works on NVIDIA, AMD, and Intel GPUs. CUDA users can pip upgrade to `onnxruntime-gpu` if they want.

---

## Unresolved Questions

1. Should Windows build ship `onnxruntime-directml` (universal) or `onnxruntime-gpu` (NVIDIA-only)?
   - Recommendation: DirectML for universal compatibility
2. Is INT8 quantization of inswapper_128 viable without unacceptable accuracy loss?
   - Needs empirical testing with AMD Quark quantizer
3. Should we add a "Performance Mode" selector in the UI (Quality/Balanced/Efficiency)?

---

## References

- [AMD Ryzen AI Software GitHub](https://github.com/amd/Ryzen-AI-SW)
- [AMD Quark Quantizer](https://pypi.org/project/amd-quark/)
- [ONNX Runtime DirectML EP](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)
- [AMD Vitis AI Model Zoo](https://github.com/Xilinx/Vitis-AI-Model-Zoo)
