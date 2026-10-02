<#
.SYNOPSIS
  Capture a packaged-app "applied / Running" screenshot for T20 evidence.

.DESCRIPTION
  Seeds a fresh data dir, launches the packaged exe with V2RAYN_R_AUTO_SMOKE=1
  and a managed net_host/xray, waits until port 11808 is listening, screenshots
  the window, copies the staged config.json + core.log + journal, then stops only
  the PIDs it started.

  Ports >=11808 only. Never touches 10808, the system proxy or TUN.
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$XrayBin = '',
  [string]$EvidenceDir = ''
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($EvidenceDir -eq '') { $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\T20.screenshots' }
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null

if ($Zip -eq '') {
  $Zip = (Get-ChildItem (Join-Path $RepoRoot 'dist') -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if ($XrayBin -eq '') { $XrayBin = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe' }

$w = Join-Path $env:TEMP ("t20_apply_" + [guid]::NewGuid().ToString('N'))
$extract = Join-Path $w 'extract'
$seed = Join-Path $w 'data'
$run = Join-Path $w 'run'
New-Item -ItemType Directory -Path $extract, $seed, $run -Force | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $extract -Force
$pkg = (Get-ChildItem $extract -Directory | Select-Object -First 1).FullName

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Nat21 {
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

& cargo run -q --manifest-path (Join-Path $RepoRoot 'Cargo.toml') -p bridge_api --example t18b_seed --release -- $seed |
  Select-Object -Last 1

$vars = @{
  V2RAYN_R_DATA_DIR   = $seed
  V2RAYN_R_AUTO_SMOKE = '1'
  V2RAYN_R_XRAY_BIN   = (Resolve-Path $XrayBin).Path
  V2RAYN_R_NET_HOST   = (Join-Path $pkg 'net_host.exe')
  V2RAYN_R_RUN_ROOT   = $run
  V2RAYN_R_PIPE       = "\\.\pipe\t20apply" + [guid]::NewGuid().ToString('N')
}
$old = @{}
foreach ($k in $vars.Keys) { $old[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$vars[$k]) }
try { $p = Start-Process -FilePath (Join-Path $pkg 'v2rayn_desktop.exe') -WorkingDirectory $pkg -PassThru }
finally { foreach ($k in $old.Keys) { [Environment]::SetEnvironmentVariable($k, $old[$k]) } }
$appPid = $p.Id
Write-Host "pid=$appPid"

$state = 'timeout'
$deadline = (Get-Date).AddSeconds(60)
while ((Get-Date) -lt $deadline) {
  if ([bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)) {
    $state = 'running'
    break
  }
  Start-Sleep -Milliseconds 300
}

$shot = Join-Path $EvidenceDir 'T20-smoke-applied-running.png'
if ($state -eq 'running') {
  Start-Sleep -Seconds 2
  $hwnd = [Nat21]::Find([uint32]$appPid)
  if ($hwnd -ne [IntPtr]::Zero) {
    [Nat21]::SetWindowPos($hwnd, [IntPtr](-1), 80, 80, 0, 0, 0x0043) | Out-Null
    Start-Sleep -Milliseconds 1200
    $r = New-Object Nat21+RECT
    [Nat21]::GetWindowRect($hwnd, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap(($r.Right - $r.Left), ($r.Bottom - $r.Top))
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size(($r.Right - $r.Left), ($r.Bottom - $r.Top))))
    $bmp.Save($shot, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "screenshot=$shot"
  }
}

# Preserve the live artifacts before teardown.
$artDir = Join-Path $EvidenceDir 'T20-applied-artifacts'
New-Item -ItemType Directory -Path $artDir -Force | Out-Null
Get-ChildItem $run -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
  Copy-Item $_.FullName (Join-Path $artDir ($_.Directory.Name + '_' + $_.Name)) -Force
}

$null = $p.CloseMainWindow()
$p.WaitForExit(8000) | Out-Null
if (-not $p.HasExited) { Stop-Process -Id $appPid -Force -ErrorAction SilentlyContinue }
Get-CimInstance Win32_Process -Filter "ParentProcessId=$appPid" -ErrorAction SilentlyContinue |
  ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Write-Host "state=$state compressed=$([bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue))"
