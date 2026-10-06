# SP-26 renderer probe (read-only, no prod code changes).
# Dumps: GPU inventory, Flutter engine identity, renderer switch strings
# present in flutter_windows.dll, embedder header knobs, runner wiring state.
# Does NOT launch the app, does NOT touch proxy/ports.
param([string]$OutFile = "")

$ErrorActionPreference = "Stop"
$repo = "C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn"
$engineDir = "C:\Users\Colby\toolchains\flutter\bin\cache\artifacts\engine\windows-x64-release"
$log = New-Object System.Text.StringBuilder
function Emit([string]$s) { $log.AppendLine($s) | Out-Null; Write-Output $s }

Emit "=== SP-26 renderer probe ==="
Emit ("time_utc=" + (Get-Date).ToUniversalTime().ToString("o"))
Emit ("flutter=" + (& "C:\Users\Colby\toolchains\flutter\bin\flutter.bat" --version 2>&1 | Select-Object -First 3 | Out-String).Trim())
Emit ""
Emit "=== GPU inventory (Win32_VideoController) ==="
Get-CimInstance Win32_VideoController | ForEach-Object {
  Emit ("name=" + $_.Name + " | driver=" + $_.DriverVersion + " | arch=" + $_.VideoArchitecture)
}
Emit ""
Emit "=== engine artifacts ==="
$dll = Join-Path $engineDir "flutter_windows.dll"
$hdr = Join-Path $engineDir "flutter_windows.h"
Emit ("dll_sha256=" + (Get-FileHash $dll -Algorithm SHA256).Hash)
Emit ("dll_bytes=" + (Get-Item $dll).Length)
Emit ("hdr_sha256=" + (Get-FileHash $hdr -Algorithm SHA256).Hash)
Emit ""
Emit "=== renderer switch strings in flutter_windows.dll (ASCII scan) ==="
Emit (& python3 (Join-Path $repo "tools\perf\engine_strings.py"))
Emit ""
Emit "=== embedder header knobs (flutter_windows.h enums) ==="
Emit ((Select-String -Path $hdr -Pattern "FlutterDesktopGpuPreference|FlutterDesktopImpellerSwitch|FlutterDesktopEngineGetGraphicsAdapter|gpu_preference|impeller_switch" | ForEach-Object { $_.LineNumber.ToString() + ":" + $_.Line.Trim() }) -join "`n")
Emit ""
Emit "=== runner wiring state (expect zero HWA/impeller/gpu hits) ==="
$hits = Get-ChildItem -Recurse (Join-Path $repo "apps\desktop\windows\runner") -Include *.cpp,*.h | Select-String -Pattern "hwa|HWA|impeller|Impeller|GpuPreference|gpu_preference|GetGraphicsAdapter"
if ($hits) { $hits | ForEach-Object { Emit ($_.Path + ":" + $_.LineNumber + ":" + $_.Line) } }
else { Emit "runner_hwa_consumer_hits=0 (gap G-03 confirmed)" }
Emit ""
Emit "=== app-side HWA state (persist only) ==="
$hits2 = Get-ChildItem -Recurse (Join-Path $repo "apps\desktop\lib\features\settings") | Select-String -Pattern "EnableHWA"
$hits2 | ForEach-Object { Emit ($_.Path + ":" + $_.LineNumber + ":" + $_.Line.Trim()) }

if ($OutFile -ne "") { [System.IO.File]::WriteAllText($OutFile, $log.ToString()) }
