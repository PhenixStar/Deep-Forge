# Build the Rust server binary as a Tauri sidecar on Windows.
# Produces: app/src-tauri/binaries/deep-forge-server-x86_64-pc-windows-msvc.exe
#
# IMPORTANT: On Windows we MUST use the DirectML onnxruntime.dll (not download-binaries).
# The workspace Cargo.toml has download-binaries for Linux compat, but on Windows
# that downloads a CPU-only ORT that shadows DirectML. We override via cargo flags.
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path "$PSScriptRoot\..").Path
$BinariesDir = "$RepoRoot\app\src-tauri\binaries"
$OrtLibsDir = "$RepoRoot\core\rust-engine\ort-dml-libs"

# Ensure DirectML DLLs are available
if (-not (Test-Path "$OrtLibsDir\onnxruntime.dll")) {
    Write-Host "[BUILD] DirectML DLLs not found. Running setup..."
    & pwsh "$PSScriptRoot\setup-directml-dlls.ps1"
}

Write-Host "[BUILD] Building Rust server for Windows x86_64 (DirectML)..."
Push-Location "$RepoRoot\core\rust-engine"

# Point ORT at the DirectML DLLs (not the download-binaries CPU ones).
$env:ORT_LIB_PATH = $OrtLibsDir

# Override ort features: remove download-binaries, add directml.
# This is done via RUSTFLAGS cfg, but the simplest approach is to
# temporarily patch Cargo.toml (restored after build).
$CargoToml = Get-Content "$RepoRoot\core\rust-engine\Cargo.toml" -Raw
$Patched = $CargoToml -replace 'features = \["std", "download-binaries", "tls-native"\]', 'features = ["std", "directml", "copy-dylibs"]'
Set-Content "$RepoRoot\core\rust-engine\Cargo.toml" $Patched

try {
    cargo build --release -p deep-forge-server --features dlc-capture/opencv
} finally {
    # Restore original Cargo.toml for cross-platform compat
    Set-Content "$RepoRoot\core\rust-engine\Cargo.toml" $CargoToml
}

Pop-Location

Write-Host "[BUILD] Copying binary + DLLs to Tauri binaries..."
New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null
Copy-Item "$RepoRoot\core\rust-engine\target\release\deep-forge-server.exe" "$BinariesDir\deep-forge-server-x86_64-pc-windows-msvc.exe"

# Copy DirectML DLLs next to the binary (required at runtime)
Copy-Item "$OrtLibsDir\*.dll" $BinariesDir -Force

Write-Host "[BUILD] Done. DirectML GPU acceleration enabled."
