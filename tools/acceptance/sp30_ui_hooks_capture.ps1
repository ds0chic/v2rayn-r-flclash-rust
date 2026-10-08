<#
.SYNOPSIS
  SP-30 UI-hook window capture for the official (unarmed) Windows zip.

.DESCRIPTION
  Extracts the packaged zip into an isolated temp dir and, for each UI-only
  navigation hook (V2RAYN_R_OPEN_*), launches the shipped exe with a FRESH
  isolated data dir plus that hook set, waits for a visible titled window owned
  by the launched PID, captures ONLY that window rect to a PNG, stops the PID
  tree it started, and records a JSON summary.

  It starts no core, binds no port, never touches 127.0.0.1:10808, the host
  proxy/WinINET, Run-key, routes or TUN. Screenshots are window-rect only (no
  full-screen capture).

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_ui_hooks_capture.ps1 `
    -Zip dist/v2rayN-R-1.0.0+1-windows-x64.zip `
    -EvidenceDir docs/evidence/stable-port/SP-30/runs/<candidate>/ui-hooks
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$EvidenceDir = '',
  [int]$WindowWaitSec = 40,
  [int]$SettleMs = 1500,
  [string[]]$Hooks = @(
    'OPEN_SUBS', 'OPEN_GROUP', 'OPEN_TEMPLATE', 'OPEN_SETTINGS', 'OPEN_THEME',
    'OPEN_HOTKEY', 'OPEN_ROUTING', 'OPEN_DNS', 'OPEN_BACKUP', 'OPEN_UPDATE'
  )
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($Zip -eq '') {
  $Zip = (Get-ChildItem -Path (Join-Path $RepoRoot 'dist') -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Zip -or -not (Test-Path -LiteralPath $Zip)) { throw "zip not found: $Zip" }
$Zip = (Resolve-Path -LiteralPath $Zip).Path
if ($EvidenceDir -eq '') {
  $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\stable-port\SP-30\runs\ui-hooks'
}
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Sp30Win {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static IntPtr Find(uint target) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
'@

function Wait-Window {
  param([int]$ProcessId, [int]$TimeoutSec)
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $h = [Sp30Win]::Find([uint32]$ProcessId)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 400
  }
  return [IntPtr]::Zero
}

function Save-WindowShot {
  param([IntPtr]$Hwnd, [string]$Path)
  [Sp30Win]::SetWindowPos($Hwnd, [IntPtr](-1), 80, 80, 0, 0, 0x0043) | Out-Null
  Start-Sleep -Milliseconds 1200
  $r = New-Object Sp30Win+RECT
  [Sp30Win]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  if ($w -le 0 -or $h -le 0) { throw "window rect empty for $Path" }
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

$work = Join-Path $env:TEMP ('sp30_hooks_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $work
$pkg = (Get-ChildItem -Path $work -Directory | Select-Object -First 1).FullName
$exe = Join-Path $pkg 'v2rayn_desktop.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "packaged exe missing: $exe" }

$results = @()
foreach ($hook in $Hooks) {
  $data = Join-Path $work ('data_' + $hook)
  New-Item -ItemType Directory -Path $data -Force | Out-Null
  $env:V2RAYN_R_DATA_DIR = $data
  Set-Item -Path ("Env:V2RAYN_R_" + $hook) -Value '1' -ErrorAction Stop
  $proc = Start-Process -FilePath $exe -PassThru
  $hwnd = Wait-Window -ProcessId $proc.Id -TimeoutSec $WindowWaitSec
  $shot = Join-Path $EvidenceDir ('ui-hook-' + $hook.ToLowerInvariant() + '.png')
  $windowFound = $hwnd -ne [IntPtr]::Zero
  $exitedBeforeShot = $proc.HasExited
  if ($windowFound -and -not $exitedBeforeShot) {
    Start-Sleep -Milliseconds $SettleMs
    Save-WindowShot -Hwnd $hwnd -Path $shot
  }
  $alive = -not $proc.HasExited
  if ($alive) { & taskkill.exe /PID $proc.Id /T /F 2>&1 | Out-Null }
  Remove-Item ("Env:V2RAYN_R_" + $hook) -ErrorAction SilentlyContinue
  $results += [ordered]@{
    hook               = $hook
    window_found       = $windowFound
    exited_before_shot = $exitedBeforeShot
    screenshot         = if ($windowFound -and -not $exitedBeforeShot) { $shot } else { '' }
  }
  Start-Sleep -Milliseconds 900
}
$env:V2RAYN_R_DATA_DIR = $null

$summary = [ordered]@{
  mode        = 'sp30-ui-hooks'
  zip         = $Zip
  evidence    = $EvidenceDir
  hooks_total = $results.Count
  hooks_ok    = ($results | Where-Object { $_.window_found -and -not $_.exited_before_shot }).Count
  results     = $results
  side_effects = 'none (isolated temp data dirs; no core; no port bind; 10808 untouched; no proxy/registry/TUN/Run-key change)'
}
$out = Join-Path $EvidenceDir 'ui-hooks-result.json'
$summary | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $out -Encoding UTF8
Write-Output ("SP30_UI_HOOKS ok=" + $summary.hooks_ok + "/" + $summary.hooks_total + " out=" + $out)
