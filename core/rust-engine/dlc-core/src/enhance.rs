//! Face enhancement (GFPGAN, GPEN) via ONNX Runtime.

use crate::Frame;
use anyhow::Result;

/// Face enhancer using GFPGAN or GPEN models.
pub struct FaceEnhancer {
    // TODO: ort::Session
}

impl FaceEnhancer {
    pub fn new(_model_path: &std::path::Path) -> Result<Self> {
        todo!("Week 7: load enhancer model")
    }

    /// Enhance face quality in the frame region.
    pub fn enhance(&self, _frame: &mut Frame, _bbox: &[f32; 4]) -> Result<()> {
        todo!("Week 7: enhancer inference")
    }
}
