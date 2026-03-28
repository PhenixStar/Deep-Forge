# Build portable Python sidecar for Windows x86_64
$ErrorActionPreference = 'Stop'

$PythonVersion = "3.11.11"
$PbsRelease = "20250317"
$Triple = "x86_64-pc-windows-msvc"

$RepoRoot = (Resolve-Path "$PSScriptRoot\..").Path
$SidecarDir = "$RepoRoot\app\src-tauri\sidecar"
$BinariesDir = "$RepoRoot\app\src-tauri\binaries"

$Url = "https://github.com/astral-sh/python-build-standalone/releases/download/$PbsRelease/cpython-$PythonVersion+$PbsRelease-$Triple-install_only.tar.gz"

Write-Host "[BUILD] Target: Windows $Triple"
Write-Host "[BUILD] Python: $PythonVersion (PBS $PbsRelease)"

# Clean previous build
if (Test-Path $SidecarDir) { Remove-Item -Recurse -Force $SidecarDir }
New-Item -ItemType Directory -Force -Path $SidecarDir | Out-Null
New-Item -ItemType Directory -Force -Path $BinariesDir | Out-Null

# 1. Download and extract Python
Write-Host "[BUILD] Downloading python-build-standalone..."
$TarGz = "$env:TEMP\python-standalone.tar.gz"
Invoke-WebRequest -Uri $Url -OutFile $TarGz
tar -xzf $TarGz -C $SidecarDir
Remove-Item $TarGz

# 2. Create virtual environment
Write-Host "[BUILD] Creating virtual environment..."
& "$SidecarDir\python\python.exe" -m venv "$SidecarDir\venv"

# 3. Install dependencies
Write-Host "[BUILD] Installing Python dependencies..."
& "$SidecarDir\venv\Scripts\pip.exe" install --no-cache-dir `
  numpy "opencv-python==4.10.0.84" "insightface==0.7.3" `
  "onnxruntime-gpu==1.24.2" `
  fastapi "uvicorn[standard]" python-multipart `
  "psutil==5.9.8" "protobuf==4.25.1"

# 4. Copy application source
Write-Host "[BUILD] Copying app source..."
New-Item -ItemType Directory -Force -Path "$SidecarDir\app" | Out-Null
Copy-Item "$RepoRoot\core\server.py" "$SidecarDir\app\"
Copy-Item -Recurse "$RepoRoot\core\modules" "$SidecarDir\app\"

# 5. Create models directory (populated at first run)
New-Item -ItemType Directory -Force -Path "$SidecarDir\models" | Out-Null

# 6. Windows wrapper is a compiled Rust launcher (see sidecar-launcher/)
# The .exe is built separately via: cargo build --release --manifest-path app/src-tauri/sidecar-launcher/Cargo.toml
# Then copied to: app/src-tauri/binaries/deep-live-cam-server-x86_64-pc-windows-msvc.exe
Write-Host "[BUILD] NOTE: Windows wrapper .exe must be built from app/src-tauri/sidecar-launcher/"
Write-Host "[BUILD] Run: cargo build --release --manifest-path app/src-tauri/sidecar-launcher/Cargo.toml"

Write-Host "[BUILD] Sidecar built successfully for $Triple"
