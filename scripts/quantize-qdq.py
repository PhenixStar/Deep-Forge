"""Static QDQ INT8 quantization for VitisAI NPU with real face calibration data."""
import os
import numpy as np
from PIL import Image
from onnxruntime.quantization import quantize_static, CalibrationDataReader, QuantFormat, QuantType

def load_face_image(path, size):
    """Load and preprocess a face image for calibration."""
    img = Image.open(path).convert("RGB").resize((size[3], size[2]))
    arr = np.array(img, dtype=np.float32)
    # HWC RGB -> CHW BGR, normalize to [-1, 1]
    arr = arr[:, :, ::-1]  # RGB to BGR
    arr = np.transpose(arr, (2, 0, 1))  # HWC to CHW
    arr = (arr - 127.5) / 128.0
    return arr[np.newaxis]  # Add batch dim

class FaceCalibReader(CalibrationDataReader):
    def __init__(self, input_name, shape, image_path, n=20):
        base = load_face_image(image_path, shape)
        # Create variations with slight augmentation
        self.data = []
        for i in range(n):
            aug = base + np.random.randn(*base.shape).astype(np.float32) * 0.02
            self.data.append({input_name: aug.astype(np.float32)})
        self.iter = iter(self.data)

    def get_next(self):
        return next(self.iter, None)

# Use the test face for calibration
calib_image = r"C:\Users\Kratos\AppData\Local\Temp\test_face.jpg"
models_dir = r"C:\Users\Kratos\AppData\Local\Deep Live Cam\models"

models = [
    ("buffalo_l/buffalo_l/det_10g.onnx", "SCRFD", "input.1", [1,3,640,640]),
    ("buffalo_l/buffalo_l/w600k_r50.onnx", "ArcFace", "input.1", [1,3,112,112]),
]

for path, name, inp, shape in models:
    full = os.path.join(models_dir, path)
    stem, ext = os.path.splitext(full)
    out = f"{stem}_int8{ext}"
    if not os.path.exists(full):
        print(f"[SKIP] {name}")
        continue
    # Remove old quantized model
    if os.path.exists(out):
        os.remove(out)
    print(f"[{name}] QDQ quantization with face calibration...", flush=True)
    try:
        reader = FaceCalibReader(inp, shape, calib_image)
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
