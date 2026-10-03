# FIX-15B real dual-process single-instance / wake test.
#
# Launches the built v2rayN-R runner twice with an isolated data dir, hides the
# first window with a Win32 call, then verifies the second process exits and the
# first window is restored to visible/foreground by the wake message.
#
# Constraints honoured: no 127.0.0.1:10808 use, no system proxy/registry/route
# changes, only kills the two PIDs this script started.

param(
  [string]$Exe = 'C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn\apps\desktop\build\windows\x64\runner\Release\v2rayn_desktop.exe',
  [string]$DataDir = "$env:TEMP\v2raynr-fix15b\data",
  [string]$LogFile = 'C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn\docs\evidence\UX-PARITY-FIX-15B\dual-process.log'
)

$ErrorActionPreference = 'Stop'
$lines = New-Object System.Collections.Generic.List[string]
function Log([string]$m) {
  $line = "[{0}] {1}" -f (Get-Date -Format 'HH:mm:ss.fff'), $m
  $lines.Add($line) | Out-Null
  Write-Host $line
}

New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
$env:V2RAYN_R_DATA_DIR = $DataDir
Log "exe=$Exe"
Log "data_dir=$DataDir"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
"@

$proc1 = $null
$proc2 = $null
try {
  Log "launch first instance"
  $proc1 = Start-Process -FilePath $Exe -PassThru
  Log "first pid=$($proc1.Id)"

  $deadline = (Get-Date).AddSeconds(45)
  $h1 = [IntPtr]::Zero
  while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $proc1.Refresh()
    if ($proc1.HasExited) { throw "first instance exited early code=$($proc1.ExitCode)" }
    if ($proc1.MainWindowHandle -ne [IntPtr]::Zero) { $h1 = $proc1.MainWindowHandle; break }
  }
  if ($h1 -eq [IntPtr]::Zero) { throw "first instance never produced a main window" }
  Log "first window handle=0x$($h1.ToInt64().ToString('X')) visible=$([W]::IsWindowVisible($h1))"

  # Hide the owned window so the wake result is observable.
  [void][W]::ShowWindow($h1, 0) # SW_HIDE
  Start-Sleep -Milliseconds 800
  $visibleHidden = [W]::IsWindowVisible($h1)
  Log "after SW_HIDE visible=$visibleHidden"

  Log "launch second instance"
  $t0 = Get-Date
  $proc2 = Start-Process -FilePath $Exe -PassThru
  Log "second pid=$($proc2.Id)"

  $exitDeadline = (Get-Date).AddSeconds(20)
  while ((Get-Date) -lt $exitDeadline) {
    Start-Sleep -Milliseconds 300
    $proc2.Refresh()
    if ($proc2.HasExited) { break }
  }
  $secondExited = $proc2.HasExited
  $secondExitCode = if ($secondExited) { $proc2.ExitCode } else { $null }
  $secondElapsed = [math]::Round(((Get-Date) - $t0).TotalSeconds, 2)
  Log "second exited=$secondExited code=$secondExitCode after=${secondElapsed}s"

  # Give the first instance time to process the posted wake message.
  Start-Sleep -Seconds 2
  $proc1.Refresh()
  $visibleRestored = [W]::IsWindowVisible($h1)
  $fg = [W]::GetForegroundWindow()
  $fgIsFirst = ($fg -eq $h1)
  Log "after wake: first_visible=$visibleRestored foreground_is_first=$fgIsFirst"

  $pass = $secondExited -and $visibleRestored
  Log "RESULT second_exited=$secondExited first_visible_after_hide=$visibleRestored foreground_is_first=$fgIsFirst pass=$pass"
  "PASS=$pass" | Set-Content -LiteralPath "$LogFile.pass"
}
finally {
  foreach ($p in @($proc2, $proc1)) {
    if ($null -ne $p) {
      try {
        $p.Refresh()
        if (-not $p.HasExited) {
          Log "stopping started pid=$($p.Id)"
          Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        }
      } catch {}
    }
  }
  $lines | Set-Content -LiteralPath $LogFile
}
