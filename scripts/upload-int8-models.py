"""Upload INT8 quantized models to HuggingFace phenixstar/deep-forge-models."""
from huggingface_hub import HfApi
import os

api = HfApi()
repo_id = "phenixstar/deep-forge-models"
models_dir = r"C:\Users\Kratos\AppData\Local\Deep Live Cam\models"

files = [
    ("buffalo_l/buffalo_l/det_10g_int8.onnx", "SCRFD INT8"),
    ("buffalo_l/buffalo_l/w600k_r50_int8.onnx", "ArcFace INT8"),
    ("inswapper_128_int8.onnx", "inswapper INT8"),
]

for path, name in files:
    full = os.path.join(models_dir, path)
    if not os.path.exists(full):
        print(f"[SKIP] {name}: not found")
        continue
    size = os.path.getsize(full) / 1e6
    print(f"[{name}] Uploading {path} ({size:.0f}MB)...", end=" ", flush=True)
    try:
        api.upload_file(
            path_or_fileobj=full,
            path_in_repo=path,
            repo_id=repo_id,
            commit_message=f"Add {name} QDQ quantized model",
        )
        print("OK")
    except Exception as e:
        print(f"FAILED: {e}")
