//! Windows sidecar launcher for Deep Live Cam.
//!
//! Tiny binary that resolves paths relative to its own location and spawns
//! the bundled Python interpreter with server.py. Tauri's externalBin
//! expects a real .exe on Windows (batch files won't work).

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let exe_dir: PathBuf = env::current_exe()
        .expect("cannot resolve exe path")
        .parent()
        .expect("exe has no parent dir")
        .to_path_buf();

    let sidecar = exe_dir.join("..").join("sidecar");
    let python = sidecar.join("venv").join("Scripts").join("python.exe");
    let server = sidecar.join("app").join("server.py");

    let status = Command::new(&python)
        .env("PYTHONHOME", sidecar.join("python"))
        .env("PYTHONPATH", sidecar.join("app"))
        .env("DEEP_LIVE_CAM_MODELS_DIR", sidecar.join("models"))
        .arg(&server)
        .args(env::args().skip(1))
        .status()
        .unwrap_or_else(|e| {
            eprintln!("[LAUNCHER] Failed to start Python: {e}");
            eprintln!("[LAUNCHER] Expected python at: {}", python.display());
            std::process::exit(1);
        });

    std::process::exit(status.code().unwrap_or(1));
}
