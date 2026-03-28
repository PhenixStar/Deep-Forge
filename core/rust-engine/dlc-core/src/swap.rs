//! Face swap using inswapper_128 ONNX model.

use crate::{DetectedFace, Frame};
use anyhow::Result;

/// Face swapper using inswapper_128 model.
pub struct FaceSwapper {
    // TODO: ort::Session
}

impl FaceSwapper {
    pub fn new(_model_path: &std::path::Path) -> Result<Self> {
        todo!("Week 6: load inswapper model")
    }

    /// Swap source face onto target face in frame. Returns modified frame.
    pub fn swap(&self, _source: &DetectedFace, _target: &DetectedFace, _frame: &mut Frame) -> Result<()> {
        todo!("Week 6: inswapper inference + blending")
    }
}
