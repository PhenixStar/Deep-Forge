# Setup AMD Ryzen AI SDK for VitisAI NPU acceleration.
# Requires: Windows 11, AMD Ryzen AI 9 HX 370+ processor, NPU driver 32.0.203.280+
#
# This script downloads the VitisAI-enabled onnxruntime.dll and config files.
# After running, set ORT_LIB_PATH to point at the output directory.
$ErrorActionPreference = 'Stop'

$OutDir = (Resolve-Path "$PSScriptRoot\..\core\rust-engine\ort-dml-libs").Path

Write-Host @"
=== AMD Ryzen AI NPU Setup ===

Prerequisites:
  1. AMD NPU Driver >= 32.0.203.280 (check Device Manager > Neural Processors)
  2. Ryzen AI Software SDK v1.7+ installed
     Download: https://ryzenai.docs.amd.com/en/latest/inst.html

After installing the SDK, copy the VitisAI-enabled ORT DLLs:

  Copy-Item "C:\Program Files\RyzenAI\1.7.0\onnxruntime\*" "$OutDir"

Or if using conda environment:

  Copy-Item "$env:CONDA_PREFIX\Lib\site-packages\onnxruntime\capi\*.dll" "$OutDir"

Then create vaip_config.json (XDNA2 config):

  Copy-Item "C:\Program Files\RyzenAI\1.7.0\voe-4.0-win_amd64\vaip_config.json" "."

To run Deep Forge with NPU:

  $env:DEEP_FORGE_EP = "npu"
  $env:DEEP_FORGE_NPU_CONFIG = "vaip_config.json"
  deep-forge-server.exe --models-dir models

Note: NPU requires INT8 quantized models. Quantize with:

  pip install amd-quark
  python -m quark.onnx --input_model inswapper_128.onnx --output_model inswapper_128_int8.onnx

"@
