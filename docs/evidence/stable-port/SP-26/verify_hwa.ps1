# SP-26 runner HWA verification: one release build, two runs (OFF/ON).
# Isolated: fresh V2RAYN_R_DATA_DIR per run (no user config, no autostart
# apply), owned process only (started here, stopped here), ports >= 11808
# untouched (app idles, no core started). Screenshots + adapter log -> evidence.
param(
  [string]$Exe = "apps/desktop/build/windows/x64/runner/Release/v2rayn_desktop.exe",
  [string]$OutDir = "docs/evidence/stable-port/SP-26",
  [int]$SettleSeconds = 12
)
$ErrorActionPreference = "Stop"
$repo = "C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn"
$exeFull = Join-Path $repo $Exe
if (-not (Test-Path $exeFull)) { throw "missing exe: $exeFull" }
$hwaLog = Join-Path $env:TEMP "v2raynr-hwa.log"

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
function Capture($path) {
  $b = New-Object System.Drawing.Bitmap(
    [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width,
    [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height)
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.CopyFromScreen(0, 0, 0, 0, $b.Size)
  $g.Dispose()
  $b.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $b.Dispose()
}
function Foreground($proc) {
  $sig = '[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);'
  $t = Add-Type -MemberDefinition $sig -Name FG -Namespace W -PassThru
  for ($i = 0; $i -lt 20 -and $proc.MainWindowHandle -eq 0; $i++) {
    Start-Sleep -Milliseconds 500; $proc.Refresh()
  }
  if ($proc.MainWindowHandle -ne 0) { $t::SetForegroundWindow($proc.MainWindowHandle) | Out-Null }
}
$result = @()
foreach ($mode in @("off", "on")) {
  if (Test-Path $hwaLog) { Remove-Item $hwaLog -Force }
  $data = Join-Path ([System.IO.Path]::GetTempPath()) ("v2raynr-sp26-" + $mode)
  if (Test-Path $data) { Remove-Item $data -Recurse -Force }
  New-Item -ItemType Directory $data | Out-Null
  $env:V2RAYN_R_DATA_DIR = $data
  $env:V2RAYNR_ENABLE_HWA = $null
  $p = Start-Process $exeFull -ArgumentList "--hwa=$mode" -PassThru
  try {
    Start-Sleep -Seconds $SettleSeconds
    $p.Refresh()
    Foreground $p
    Start-Sleep -Seconds 1
    Capture (Join-Path $repo "$OutDir/run_hwa_${mode}.png")
    $alive = -not $p.HasExited
  } finally {
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
    $p.WaitForExit(15000) | Out-Null
  }
  $lines = @()
  if (Test-Path $hwaLog) { $lines = Get-Content $hwaLog }
  $lines | Set-Content (Join-Path $repo "$OutDir/run_hwa_${mode}.log") -Encoding utf8
  Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue
  $result += "mode=$mode alive_after_settle=$alive hwa_log_lines=$($lines.Count)"
  Write-Output "mode=$mode alive=$alive"
  $lines | Write-Output
}
$result | Set-Content (Join-Path $repo "$OutDir/run_summary.txt") -Encoding utf8
