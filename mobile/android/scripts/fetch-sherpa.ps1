# Downloads the sherpa-onnx Android library at the version the desktop app is
# pinned to (ADR 0002), so phone and desktop always run the same engine version.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)   # ...\mobile
$repo = Split-Path -Parent $root
$version = (Get-Content (Join-Path $repo 'src-tauri\sidecars\sherpa\VERSION')).Trim()
$libs = Join-Path $root 'android\app\libs'
$out = Join-Path $libs "sherpa-onnx-$version.aar"
New-Item -ItemType Directory -Force $libs | Out-Null
if (Test-Path $out) { "Already present: $out"; exit 0 }
$ProgressPreference = 'SilentlyContinue'
Invoke-WebRequest "https://github.com/k2-fsa/sherpa-onnx/releases/download/v$version/sherpa-onnx-$version.aar" -OutFile $out
"Downloaded $out"
