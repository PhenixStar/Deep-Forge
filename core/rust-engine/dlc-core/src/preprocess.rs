//! Image preprocessing utilities: resize, normalize, affine transforms.

use ndarray::Array4;
use anyhow::Result;

/// Resize and normalize image for SCRFD input (640x640, CHW, float32).
pub fn preprocess_detection(_frame: &crate::Frame) -> Result<Array4<f32>> {
    todo!("Week 5: resize + normalize + HWC->CHW")
}

/// Align face using 5 landmarks to canonical 112x112 for ArcFace.
pub fn align_face_arcface(
    _frame: &crate::Frame,
    _landmarks: &[[f32; 2]; 5],
) -> Result<crate::Frame> {
    todo!("Week 6: affine transform to canonical pose")
}

/// Align face using 5 landmarks for inswapper (128x128).
pub fn align_face_swap(
    _frame: &crate::Frame,
    _landmarks: &[[f32; 2]; 5],
) -> Result<crate::Frame> {
    todo!("Week 6: affine transform for swap")
}
