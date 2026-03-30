"""Dynamic INT8 quantization using onnxruntime (no calibration data needed)."""
import os
from onnxruntime.quantization import quantize_dynamic, QuantType

models_dir = r"C:\Users\Kratos\AppData\Local\Deep Live Cam\models"
models = [
    ("buffalo_l/buffalo_l/det_10g.onnx", "SCRFD"),
    ("buffalo_l/buffalo_l/w600k_r50.onnx", "ArcFace"),
    ("inswapper_128.onnx", "inswapper"),
]

for path, name in models:
    full = os.path.join(models_dir, path)
    stem, ext = os.path.splitext(full)
    out = f"{stem}_int8{ext}"
    if not os.path.exists(full):
        print(f"[SKIP] {name}: not found")
        continue
    print(f"[{name}] Quantizing...", end=" ", flush=True)
    try:
        quantize_dynamic(full, out, weight_type=QuantType.QInt8)
        orig = os.path.getsize(full) / 1e6
        qsz = os.path.getsize(out) / 1e6
        print(f"OK: {orig:.0f}MB -> {qsz:.0f}MB ({qsz/orig*100:.0f}%)")
    except Exception as e:
        print(f"FAILED: {e}")
