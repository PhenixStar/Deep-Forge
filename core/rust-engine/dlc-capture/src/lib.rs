//! Camera capture and virtual camera output.
//!
//! Uses platform-native backends (V4L2 on Linux, DirectShow on Windows,
//! AVFoundation on macOS) via the nokhwa crate (added in Week 7).

use anyhow::Result;

/// Available camera device info.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CameraInfo {
    pub index: u32,
    pub name: String,
}

/// List available cameras. Platform-aware enumeration.
pub fn list_cameras() -> Result<Vec<CameraInfo>> {
    // TODO Week 7: use nokhwa for cross-platform camera enumeration
    // Stub: return a default camera
    Ok(vec![CameraInfo {
        index: 0,
        name: "Default Camera".into(),
    }])
}
