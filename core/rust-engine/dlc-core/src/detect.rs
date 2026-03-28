//! SCRFD face detection via ONNX Runtime.

use crate::DetectedFace;
use anyhow::Result;

/// Face detector using SCRFD model from buffalo_l.
pub struct FaceDetector {
    // TODO: ort::Session
}

impl FaceDetector {
    pub fn new(_model_path: &std::path::Path) -> Result<Self> {
        todo!("Week 5: load SCRFD model")
    }

    /// Detect faces in a BGR frame. Returns faces sorted by area (largest first).
    pub fn detect(&self, _frame: &crate::Frame, _threshold: f32) -> Result<Vec<DetectedFace>> {
        todo!("Week 5: SCRFD inference + NMS")
    }
}
