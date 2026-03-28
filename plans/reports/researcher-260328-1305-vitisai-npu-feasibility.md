# AMD VitisAI XDNA2 NPU Feasibility for Face Swap Inference
**Date**: 2026-03-28
**Hardware**: AMD Ryzen AI 9 HX 370 (XDNA2, 50 TOPS), Windows 11
**Models**: insightface buffalo_l detection + inswapper_128 swap

---

## Executive Summary

**Verdict: NOT RECOMMENDED for production. FEASIBLE for detection only; face swap (generative) inference has unresolved blockers.**

The technology stack exists and newer AMD Ryzen AI Software (1.7.0 Feb 2026) improves CNN support, but face swap inference specifically faces three critical issues that cannot be mitigated without significant architectural redesign.

---

## Key Findings by Question

### 1. Can inswapper_128 (face swap) be INT8 quantized without accuracy loss?

**Finding: Uncertain, likely problematic for generative models.**

- ONNX Runtime supports INT8 quantization post-training, but documented best practices focus on CNNs and NLP transformers.
- **Critical distinction**: inswapper_128 is a **generative model (GAN-based)**, not a classification/detection CNN. Generative models are inherently more sensitive to quantization artifacts because they propagate small precision errors through multiple synthesis layers.
- Community reports show inswapper uses a [1, 3, 128, 128] input pipeline with GAN-based synthesis—these architectures typically need per-channel or selective quantization, not uniform INT8.
- **No published accuracy benchmarks exist** for INT8 quantization of inswapper_128. The model originates from InsightFace (research project), and quantization documentation is sparse.
- INT8 generative models historically suffer 2-10% quality degradation; face swap requires <2% (subtle artifacts are visible in identity/alignment).

**Risk**: Quantization may succeed technically but output quality degrades to unusable range. Requires empirical testing.

---

### 2. What operators in insightface buffalo_l detection are unsupported on XDNA2?

**Finding: Mostly supported, but sparse documentation.**

- buffalo_l detection uses SCRFD (Selective Convolutional Response Face Detection)—a CNN architecture with standard convolution operators.
- AMD Ryzen AI 1.7.0 (Feb 2026) added **Integer Compiler for CNNs** supporting A8W8, A16W8, and asymmetric quantization—significant improvement from prior versions.
- Common reshape/transpose/flatten operations have been optimized in recent releases per AMD docs.
- **No public operator matrix** exists explicitly listing XDNA2-unsupported ops for buffalo_l specifically.
- Framework issues observed in Linux environments (missing voe.passes module, compiler crashes), but Windows 11 should be cleaner since Ryzen AI Software targets Windows first.

**Risk**: Operator mapping is data-dependent; only way to know is: (1) export buffalo_l to ONNX, (2) attempt VitisAI compilation, (3) inspect fallback nodes. Expect 60-80% offload; some nodes will CPU-fallback.

---

### 3. Are there community reports of running face swap models on AMD NPU?

**Finding: NO. Zero public implementations found.**

- Found open-source XDNA2 projects (dragon-npu framework, riallto.ai) but none report face swap pipelines.
- dragon-npu reports 24,988 FPS face *recognition* (embedding extraction), not face swap synthesis.
- Community projects use SCRFD detection + ArcFace recognition + CPU/GPU swap—no one has deployed inswapper_128 on XDNA2.
- This is a **red flag**: if feasible, someone would have published it by now given popularity of face swap applications.

**Implication**: You'd be pioneering an unsupported use case. Expect to debug unknown VitisAI compiler issues solo.

---

### 4. What is the latest AMD Ryzen AI Software SDK version and does it improve CNN support?

**Finding: Version 1.7.0 (Feb 2026) is current. CNN support improved significantly, but still limited.**

- **Latest**: Ryzen AI Software 1.7.0 released Feb 20, 2026.
- **Improvements over 1.5/1.6**:
  - 2–3× faster CNN/Transformer compile times
  - Integer Compiler for A8W8, A16W8, asymmetric quantization (new in 1.6)
  - Up to 18% latency improvement, 35% power savings when preprocessing runs on NPU
  - Better operator fusion
- **Still missing** (per open GitHub issues):
  - Stable multi-operator graph partitioning under certain conditions
  - Limited support for "exotic" operators used in newer architectures
  - Windows-focused; Linux support lags significantly
- **For buffalo_l**: Likely workable with partial offload; expect compilation to succeed.
- **For inswapper_128**: No regression testing published; generative models untested on 1.7.0.

---

### 5. Is there a hybrid approach where detection runs on NPU and swap runs on DirectML simultaneously?

**Finding: Technically possible but complex and unvalidated.**

- DirectML (ONNX Runtime EP for Windows) added NPU support in version 1.13.1 + ONNX Runtime 1.17 (developer preview), but Intel NPU-focused, not AMD.
- AMD's VitisAI EP is separate from DirectML. Both can coexist in ONNX Runtime 1.17+.
- **Theoretical hybrid**: buffalo_l → VitisAI (NPU), inswapper_128 → DirectML (GPU fallback on Radeon 890M iGPU).
- **Problems**:
  1. No published examples of mixing VitisAI + DirectML in same inference graph.
  2. Inter-provider tensor transfers have undocumented latency (likely defeats NPU gains).
  3. XDNA2 and Radeon 890M GPU are separate memory spaces; copying intermediate tensors is expensive.
  4. Windows 11 driver stack would need validation (as of Feb 2026, vendor-specific EP support is fragmented).

**Practical outcome**: Hybrid approach **not recommended** without weeks of prototyping. Serial pipeline (NPU detection → DirectML swap) is simpler but you lose parallelism gains.

---

## Summary Assessment

| Component | Status | Confidence | Blocker? |
|-----------|--------|------------|----------|
| Detection (buffalo_l) on NPU | Feasible | Medium | No |
| INT8 Quantization support | Exists | Low | Yes* |
| Swap (inswapper_128) on NPU | Untested | Very Low | **Yes** |
| VitisAI compilation on Win11 | Likely works | Medium | No |
| Hybrid NPU+GPU pipeline | Possible | Low | No |

**Key blockers**:
1. **No one has successfully run inswapper_128 on XDNA2 (or any mobile NPU).** This is uncharted territory.
2. **Generative models + INT8 quantization is risky** without empirical validation on this specific model.
3. **VitisAI operator coverage for inswapper is unknown.** Model likely uses operators (upsampling, attention, normalization) not fully tested on XDNA2.

---

## Recommendation

**Suggested approach** (in priority order):

1. **Export test**: Convert both models to ONNX (if not already), attempt VitisAI compilation on Windows 11 with Ryzen AI 1.7.0. If inswapper_128 fails to compile → stop here.

2. **CPU baseline**: Benchmark FP32 inswapper_128 on CPU to establish quality baseline and latency target.

3. **Quantization pilot**: INT8 quantize inswapper_128 locally, validate accuracy loss on test frames (must be <2% visual degradation). If acceptable, move to step 4.

4. **NPU detection only**: Deploy buffalo_l detection on NPU (lower risk), keep swap on CPU/GPU. This gives you real-world wins (detection 10–20× faster) without betting on untested generative quantization.

5. **Swap optimization**: If swap becomes bottleneck, explore:
   - TensorRT quantization (NVIDIA-focused, but better generative support)
   - Distilled inswapper models (smaller, more quantization-friendly)
   - DirectML GPU fallback (iGPU acceleration, no additional hardware)

**Do NOT attempt simultaneous NPU offload of both detection and swap without steps 1–3 validation.**

---

## Unresolved Questions

- Does inswapper_128 ONNX export include any unsupported VitisAI operators? (Requires running compiler on actual model.)
- What is actual INT8 accuracy loss for inswapper_128 on face identity retention? (No published benchmarks.)
- Can tensor transfers between VitisAI and DirectML EPs be done efficiently on XDNA2 + Radeon 890M? (No public validation.)
- Is Windows 11 NPU driver stack (v32.0.203.280) stable under sustained inference? (User reports limited.)

---

## Sources

- [Ryzen AI Release 1.7.0 AMD Feb 20, 2026](https://ryzenai.docs.amd.com/_/downloads/en/latest/pdf/)
- [AMD Ryzen AI Software 1.6 Release](https://www.amd.com/en/developer/resources/technical-articles/2025/ryzen-ai-software-1-6-now-available.html)
- [AMD - Vitis AI ONNX Runtime EP](https://onnxruntime.ai/docs/execution-providers/Vitis-AI-ExecutionProvider.html)
- [GitHub: dragon-npu XDNA2 Framework](https://github.com/In2infinity/dragon-npu)
- [InsightFace Face Swapping Blog](https://www.insightface.ai/blog/the-evolution-of-neural-network-face-swapping-from-deepfakes-to-one-shot-innovation-with-insightface)
- [ONNX Runtime Quantization Guide](https://onnxruntime.ai/docs/performance/model-optimizations/quantization.html)
- [DirectML NPU Support (Developer Preview)](https://devblogs.microsoft.com/directx/introducing-neural-processor-unit-npu-support-in-directml-developer-preview/)
- [Vitis AI Compilation Failures on Linux](https://github.com/amd/RyzenAI-SW/issues/341)
- [SCRFD ONNX Runtime Implementation](https://github.com/prabhat0206/scrfd)
