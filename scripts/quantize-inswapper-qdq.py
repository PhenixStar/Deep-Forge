"""Static QDQ quantization for inswapper_128 (multi-input model).
The inswapper has 2 inputs: target[1,3,128,128] and source[1,512].
We provide calibration data for both."""
import os
import numpy as np
from PIL import Image
from onnxruntime.quantization import quantize_static, CalibrationDataReader, QuantFormat, QuantType

class InswapperCalibReader(CalibrationDataReader):
    def __init__(self, n=20):
        self.data = []
        for _ in range(n):
            # target: face image [1,3,128,128] normalized [0,1]
            target = np.random.rand(1, 3, 128, 128).astype(np.float32)
            # source: embedding [1,512] L2-normalized
            source = np.random.randn(1, 512).astype(np.float32)
            source = source / (np.linalg.norm(source) + 1e-10)
            self.data.append({"target": target, "source": source})
        self.iter = iter(self.data)

    def get_next(self):
        return next(self.iter, None)

models_dir = r"C:\Users\Kratos\AppData\Local\Deep Live Cam\models"
full = os.path.join(models_dir, "inswapper_128.onnx")
stem, ext = os.path.splitext(full)
out = f"{stem}_int8{ext}"

if os.path.exists(out):
    os.remove(out)

print("[inswapper] QDQ quantization with dual-input calibration...", flush=True)
try:
    reader = InswapperCalibReader(n=20)
    quantize_static(
        full, out, reader,
        quant_format=QuantFormat.QDQ,
        per_channel=False,
        weight_type=QuantType.QInt8,
        activation_type=QuantType.QUInt8,
    )
    orig = os.path.getsize(full) / 1e6
    qsz = os.path.getsize(out) / 1e6
    print(f"  OK: {orig:.0f}MB -> {qsz:.0f}MB ({qsz/orig*100:.0f}%)")
except Exception as e:
    print(f"  FAILED: {e}")
