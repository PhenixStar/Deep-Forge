//! Shared application state for the server.

/// Server-wide mutable state protected by RwLock.
#[derive(Debug, Clone)]
pub struct AppState {
    pub active_camera: u32,
    pub face_enhancer_gfpgan: bool,
    pub face_enhancer_gpen256: bool,
    pub face_enhancer_gpen512: bool,
    pub frame_processors: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_camera: 0,
            face_enhancer_gfpgan: false,
            face_enhancer_gpen256: false,
            face_enhancer_gpen512: false,
            frame_processors: vec!["face_swapper".into()],
        }
    }
}
