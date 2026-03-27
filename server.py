"""FastAPI backend for Deep-Live-Cam Tauri app.

Exposes face-swap pipeline via HTTP + WebSocket for real-time video streaming.
Designed to run as a Tauri sidecar process on localhost:8008.
"""

import asyncio
import sys
import os
import io
import threading

import cv2
import numpy as np
from fastapi import FastAPI, WebSocket, UploadFile, File, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
import uvicorn

# Add project root to path so modules/ is importable
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import modules.globals as globals
from modules.face_analyser import get_one_face
from modules.processors.frame.core import get_frame_processors_modules

app = FastAPI(title="Deep-Live-Cam Server", version="0.1.0")
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

# Shared state
_source_face = None
_source_face_lock = threading.Lock()


def _init_providers():
    """Initialize ONNX execution providers."""
    import onnxruntime as ort
    providers = ort.get_available_providers()
    if "CUDAExecutionProvider" in providers:
        globals.execution_providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
    else:
        globals.execution_providers = ["CPUExecutionProvider"]
    globals.frame_processors = ["face_swapper"]


@app.on_event("startup")
async def startup():
    _init_providers()
    print(f"[SERVER] Providers: {globals.execution_providers}")


@app.get("/health")
async def health():
    return {"status": "ok", "providers": globals.execution_providers}


@app.post("/source")
async def upload_source(file: UploadFile = File(...)):
    """Upload source face image."""
    global _source_face
    contents = await file.read()
    nparr = np.frombuffer(contents, np.uint8)
    img = cv2.imdecode(nparr, cv2.IMREAD_COLOR)
    if img is None:
        return JSONResponse(status_code=400, content={"error": "Invalid image"})

    face = get_one_face(img)
    if face is None:
        return JSONResponse(status_code=400, content={"error": "No face detected in source"})

    with _source_face_lock:
        _source_face = face
    return {"status": "ok", "message": "Source face loaded"}


@app.websocket("/ws/video")
async def video_stream(ws: WebSocket):
    """WebSocket endpoint: captures webcam, applies face swap, streams JPEG frames."""
    await ws.accept()

    cap = cv2.VideoCapture(0)
    if not cap.isOpened():
        await ws.send_json({"error": "Cannot open camera"})
        await ws.close()
        return

    cap.set(cv2.CAP_PROP_FRAME_WIDTH, 1280)
    cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 720)

    frame_processors = get_frame_processors_modules(globals.frame_processors)

    try:
        while True:
            ret, frame = cap.read()
            if not ret:
                await asyncio.sleep(0.01)
                continue

            with _source_face_lock:
                source = _source_face

            # Apply face swap if source face is set
            if source is not None:
                for processor in frame_processors:
                    if processor.NAME == "DLC.FACE-SWAPPER":
                        target_face = get_one_face(frame)
                        if target_face is not None:
                            frame = processor.swap_face(source, target_face, frame)

            # Encode as JPEG and send
            _, buffer = cv2.imencode(".jpg", frame, [cv2.IMWRITE_JPEG_QUALITY, 80])
            await ws.send_bytes(buffer.tobytes())

            # Yield to event loop (~30fps target)
            await asyncio.sleep(0.033)

    except WebSocketDisconnect:
        pass
    finally:
        cap.release()


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8008, log_level="info")
