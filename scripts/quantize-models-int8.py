"""Quantize ONNX models to INT8 for AMD NPU (XDNA2) inference.

Uses AMD Quark quantizer with calibration data.
Produces *_int8.onnx files alongside the originals.

Usage:
    python scripts/quantize-models-int8.py --models-dir <path>
    python scripts/quantize-models-int8.py  # uses default models dir
"""

import argparse
import os
import sys
import numpy as np

class RandomCalibrationReader:
    """Calibration data reader that generates random samples."""
    def __init__(self, input_name, input_shape, num_samples=10):
        self.data = []
        for _ in range(num_samples):
            sample = np.random.randn(*input_shape).astype(np.float32)
            sample = (sample - sample.mean()) / (sample.std() + 1e-6)
            self.data.append({input_name: sample})
        self.index = 0

    def get_next(self):
        if self.index >= len(self.data):
            return None
        result = self.data[self.index]
        self.index += 1
        return result

    def rewind(self):
        self.index = 0


def quantize_model(model_path, output_path, input_name="input.1", input_shape=None):
    """Quantize a single ONNX model to INT8 using AMD Quark."""
    try:
        from quark.onnx import ModelQuantizer, PowerOfTwoMethod, QuantType
        from quark.onnx.quantization.config.config import Config, QuantizationConfig
    except ImportError:
        print(f"  ERROR: AMD Quark not installed.")
        return False

    print(f"  Quantizing {os.path.basename(model_path)}...")
    print(f"  Input: {input_name} shape={input_shape}")

    try:
        quant_config = QuantizationConfig(
            quant_format=QuantType.QInt8,
            calibrate_method=PowerOfTwoMethod.MinMSE,
        )
        config = Config(global_quant_config=quant_config)

        quantizer = ModelQuantizer(config)

        reader = RandomCalibrationReader(input_name, input_shape) if input_shape else None

        quantizer.quantize_model(
            model_path,
            output_path,
            calibration_data_reader=reader,
        )

        orig_size = os.path.getsize(model_path) / 1e6
        quant_size = os.path.getsize(output_path) / 1e6
        print(f"  OK: {orig_size:.0f}MB -> {quant_size:.0f}MB ({quant_size/orig_size*100:.0f}%)")
        return True

    except Exception as e:
        print(f"  FAILED: {e}")
        return False


def quantize_static_fallback(model_path, output_path, input_name, input_shape):
    """Fallback using onnxruntime quantization if quark API differs."""
    try:
        from onnxruntime.quantization import quantize_static, CalibrationDataReader, QuantType
        import onnx

        class RandomCalibReader(CalibrationDataReader):
            def __init__(self, input_name, input_shape, num_samples=10):
                self.data = iter([
                    {input_name: np.random.randn(*input_shape).astype(np.float32)}
                    for _ in range(num_samples)
                ])

            def get_next(self):
                return next(self.data, None)

        print(f"  Using onnxruntime quantize_static fallback...")
        reader = RandomCalibReader(input_name, input_shape) if input_shape else None

        quantize_static(
            model_path,
            output_path,
            reader,
            quant_format=QuantType.QInt8,
            per_channel=True,
        )

        orig_size = os.path.getsize(model_path) / 1e6
        quant_size = os.path.getsize(output_path) / 1e6
        print(f"  OK: {orig_size:.0f}MB -> {quant_size:.0f}MB")
        return True
    except Exception as e:
        print(f"  Fallback FAILED: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Quantize ONNX models to INT8 for NPU")
    parser.add_argument("--models-dir", default="C:/Users/Kratos/AppData/Local/Deep Live Cam/models")
    args = parser.parse_args()

    models_dir = args.models_dir

    # Models to quantize with their input specs
    models = [
        {
            "name": "SCRFD Face Detector",
            "file": "buffalo_l/buffalo_l/det_10g.onnx",
            "input_name": "input.1",
            "input_shape": [1, 3, 640, 640],
        },
        {
            "name": "ArcFace Embedding",
            "file": "buffalo_l/buffalo_l/w600k_r50.onnx",
            "input_name": "input.1",
            "input_shape": [1, 3, 112, 112],
        },
        {
            "name": "inswapper 128",
            "file": "inswapper_128.onnx",
            "input_name": "target",
            "input_shape": [1, 3, 128, 128],
        },
    ]

    print(f"Models dir: {models_dir}")
    print(f"Quantizing {len(models)} models to INT8...\n")

    results = {}
    for m in models:
        path = os.path.join(models_dir, m["file"])
        if not os.path.exists(path):
            print(f"[SKIP] {m['name']}: file not found")
            results[m["name"]] = "skipped"
            continue

        stem, ext = os.path.splitext(path)
        out_path = f"{stem}_int8{ext}"

        print(f"[{m['name']}]")
        ok = quantize_model(path, out_path, m["input_name"], m["input_shape"])
        results[m["name"]] = "OK" if ok else "FAILED"
        print()

    print("=== Results ===")
    for name, status in results.items():
        print(f"  {name}: {status}")


if __name__ == "__main__":
    main()
