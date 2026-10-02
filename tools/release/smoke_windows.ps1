<#
.SYNOPSIS
  Clean-environment smoke test for the packaged Windows x64 release candidate.

.DESCRIPTION
  Extracts the portable zip into an isolated temp directory, launches the
  packaged exe with V2RAYN_R_DATA_DIR pointing at a fresh temp data dir, and
  verifies: process start, window appears, first-run data files are created,
  the backup/update windows open via the existing V2RAYN_R_OPEN_* hooks, and
  the process tree is cleaned up on exit.

  With -WithSeed it additionally uses the existing
  crates/bridge_api/examples/t18b_seed.rs to seed a synthetic node and triggers
  the app-internal apply path (V2RAYN_R_AUTO_SMOKE=1) against a loopback port.

  Safety: never touches 127.0.0.1:10808, the system proxy or TUN. Only the app
  PID started by this script and its descendants are ever stopped.

.NOTES
  GUI automation is limited to the app's own evidence hooks. If a dialog cannot
  be driven, the script records that honestly instead of faking success.
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$EvidenceDir = '',
  [string]$XrayBin = '',
  [int]$WindowWaitSec = 60,
  [int]$ApplyWaitSec = 45,
  [switch]$WithSeed
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($EvidenceDir -eq '') { $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\T20.screenshots' }
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null

if ($Zip -eq '') {
  $Zip = (Get-ChildItem -Path (Join-Path $RepoRoot 'dist') -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Zip -or -not (Test-Path -LiteralPath $Zip)) { throw "zip not found: $Zip" }
$Zip = (Resolve-Path -LiteralPath $Zip).Path

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class Nat20 {
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
  public static long[] FindForceful(uint target) {
    var ids = new List<long>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { ids.Add(h.ToInt64()); }
      return true;
    }, IntPtr.Zero);
    return ids.ToArray();
  }
}
'@

$script:hr = [System.Collections.Generic.List[string]]::new()
function Log([string]$line) {
  $ts = (Get-Date).ToString('HH:mm:ss')
  $script:hr.Add("[$ts] $line") | Out-Null
  Write-Host "   $line"
}

function Wait-Window {
  param([int]$ProcessId, [int]$TimeoutSec)
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $h = [Nat20]::Find([uint32]$ProcessId)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 500
  }
  return [IntPtr]::Zero
}

function Save-WindowShot {
  param([IntPtr]$Hwnd, [string]$Path)
  [Nat20]::SetWindowPos($Hwnd, [IntPtr](-1), 80, 80, 0, 0, 0x0043) | Out-Null
  Start-Sleep -Milliseconds 1200
  $r = New-Object Nat20+RECT
  [Nat20]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left
  $h = $r.Bottom - $r.Top
  if ($w -le 0 -or $h -le 0) { throw "window rect empty for $Path" }
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Log "screenshot $Path (${w}x${h})"
}

function Get-DescendantPids {
  param([int]$RootPid)
  $result = [System.Collections.Generic.List[int]]::new()
  $queue = [System.Collections.Generic.Queue[int]]::new()
  $queue.Enqueue($RootPid)
  while ($queue.Count -gt 0) {
    $parent = $queue.Dequeue()
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId=$parent" -ErrorAction SilentlyContinue
    foreach ($c in $children) {
      $result.Add([int]$c.ProcessId) | Out-Null
      $queue.Enqueue([int]$c.ProcessId)
    }
  }
  return $result
}

function Set-ChildEnv {
  param([hashtable]$Vars)
  $old = @{}
  foreach ($k in $Vars.Keys) {
    $old[$k] = [Environment]::GetEnvironmentVariable($k)
    [Environment]::SetEnvironmentVariable($k, [string]$Vars[$k])
  }
  return $old
}
function Restore-ChildEnv {
  param([hashtable]$Old)
  foreach ($k in $Old.Keys) { [Environment]::SetEnvironmentVariable($k, $Old[$k]) }
}

function Stop-OwnedTree {
  param([int[]]$OwnedPids)
  $stopped = [System.Collections.Generic.List[int]]::new()
  $owned = @($OwnedPids) | Sort-Object -Unique
  [array]::Reverse($owned)
  foreach ($cpid in $owned) {
    $p = Get-Process -Id $cpid -ErrorAction SilentlyContinue
    if ($p) { Stop-Process -Id $cpid -Force -ErrorAction SilentlyContinue; $stopped.Add($cpid) | Out-Null }
  }
  return $stopped
}

$workRoot = Join-Path $env:TEMP ("t20_smoke_" + [guid]::NewGuid().ToString('N'))
$extractDir = Join-Path $workRoot 'extract'
New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $extractDir -Force
$pkg = (Get-ChildItem -LiteralPath $extractDir -Directory | Select-Object -First 1).FullName
$mainExe = Join-Path $pkg 'v2rayn_desktop.exe'
if (-not (Test-Path -LiteralPath $mainExe)) { throw "main exe not found in package: $pkg" }

Log "zip=$Zip"
Log "package=$pkg"
Log "mainExe=$mainExe"

$results = [ordered]@{
  zip = $Zip
  package = $pkg
  main_exe = $mainExe
  work_root = $workRoot
  runs = @()
  dev_path_dependency = 'not_proven (no Process Monitor); GUI has no default file log'
  limitations = @()
}

function Invoke-SmokeRun {
  param(
    [string]$Name,
    [hashtable]$ExtraEnv,
    [string]$ScreenshotName,
    [string]$DataDirOverride = ''
  )
  $run = [ordered]@{ name = $Name; env = @{}; started_pid = 0; window = $false; screenshot = ''; data_files = @{}; exited_on_close = $false; forced_kill = $false; applied = $false; notes = @() }
  $dataDir = if ($DataDirOverride -ne '') { $DataDirOverride } else { Join-Path $workRoot ("data_" + $Name) }
  New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
  $vars = @{ V2RAYN_R_DATA_DIR = $dataDir }
  foreach ($k in $ExtraEnv.Keys) { $vars[$k] = $ExtraEnv[$k] }
  $run.env = $vars
  $old = Set-ChildEnv -Vars $vars
  try {
    $proc = Start-Process -FilePath $mainExe -WorkingDirectory $pkg -PassThru
  } finally {
    Restore-ChildEnv -Old $old
  }
  $appPid = $proc.Id
  $run.started_pid = $appPid
  Log "${Name}: started pid=$appPid"

  $hwnd = Wait-Window -ProcessId $appPid -TimeoutSec $WindowWaitSec
  if ($hwnd -ne [IntPtr]::Zero) {
    $run.window = $true
    $shot = Join-Path $EvidenceDir $ScreenshotName
    try { Save-WindowShot -Hwnd $hwnd -Path $shot; $run.screenshot = $shot }
    catch { $run.notes += "screenshot failed: $_" }
  } else {
    $run.notes += "window not found within ${WindowWaitSec}s"
  }

  # First-run data files (poll up to 60s).
  $deadline = (Get-Date).AddSeconds(60)
  while ((Get-Date) -lt $deadline) {
    $cfg = Test-Path -LiteralPath (Join-Path $dataDir 'guiNConfig.json')
    $db = Test-Path -LiteralPath (Join-Path $dataDir 'guiNNDB.db')
    if ($cfg -and $db) { break }
    Start-Sleep -Milliseconds 500
  }
  foreach ($f in @('guiNConfig.json', 'guiNNDB.db')) {
    $fp = Join-Path $dataDir $f
    $run.data_files[$f] = (Test-Path -LiteralPath $fp)
  }
  $run.data_dir = $dataDir
  Log "${Name}: window=$($run.window) guiNConfig=$($run.data_files['guiNConfig.json']) guiNNDB=$($run.data_files['guiNNDB.db'])"

  # Record owned descendants before shutdown so late children are never missed.
  $beforeDesc = @(Get-DescendantPids -RootPid $appPid)

  $run.applied = $false
  if ($ExtraEnv.ContainsKey('V2RAYN_R_AUTO_SMOKE')) {
    $deadline = (Get-Date).AddSeconds($ApplyWaitSec)
    $xraySeen = $false
    $portSeen = $false
    while ((Get-Date) -lt $deadline) {
      $descNow = @(Get-DescendantPids -RootPid $appPid)
      $xraySeen = $false
      if ($descNow.Count -gt 0) {
        foreach ($dpid in $descNow) {
          $dp = Get-Process -Id $dpid -ErrorAction SilentlyContinue
          if ($dp -and $dp.ProcessName -match 'xray|sing-box') { $xraySeen = $true }
        }
      }
      $portSeen = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
      if ($portSeen) { break }
      Start-Sleep -Milliseconds 300
    }
    $run.has_core_descendant = $xraySeen
    $run.port_11808_listening = $portSeen
    $run.applied = $portSeen
    if ($portSeen) {
      $shot2 = Join-Path $EvidenceDir $ScreenshotName
      $hwnd2 = [Nat20]::Find([uint32]$appPid)
      if ($hwnd2 -ne [IntPtr]::Zero) {
        $shotApplied = $shot2 -replace '\.png$', '_applied.png'
        try { Save-WindowShot -Hwnd $hwnd2 -Path $shotApplied; $run.screenshot_applied = $shotApplied }
        catch { $run.notes += "applied screenshot failed: $_" }
      }
    }
    Log "${Name}: applied=$($run.applied) port11808=$portSeen coreChild=$xraySeen"
  }

  # Graceful close, then bounded wait.
  $null = $proc.CloseMainWindow()
  $exited = $proc.WaitForExit(8000)
  if ($exited) {
    $run.exited_on_close = $true
    Log "${Name}: exited on CloseMainWindow"
  } else {
    Stop-Process -Id $appPid -Force -ErrorAction SilentlyContinue
    $run.forced_kill = $true
    Log "${Name}: did not exit on CloseMainWindow, force-stopped app pid"
  }

  # Wait for children to disappear, then clean only the IDs we recorded.
  Start-Sleep -Seconds 8
  $remaining = @($beforeDesc | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
  if ($remaining.Count -gt 0) {
    $stopped = Stop-OwnedTree -OwnedPids $beforeDesc
    $run.notes += "cleaned remaining descendants: $($stopped -join ',')"
  }
  Start-Sleep -Seconds 2
  $final = @(Get-Process -Id $appPid -ErrorAction SilentlyContinue).Count
  $run.self_alive_after = ($final -gt 0)
  if ($run.self_alive_after) { $run.notes += "app pid still alive after cleanup" }

  return $run
}

# Run 1: update window hook.
$results.runs += Invoke-SmokeRun -Name 'update' -ExtraEnv @{ V2RAYN_R_OPEN_UPDATE = '1' } -ScreenshotName 'T20-smoke-update-window.png'
# Run 2: backup window hook.
$results.runs += Invoke-SmokeRun -Name 'backup' -ExtraEnv @{ V2RAYN_R_OPEN_BACKUP = '1' } -ScreenshotName 'T20-smoke-backup-window.png'

# Optional seeded apply run.
if ($WithSeed) {
  if ($XrayBin -eq '') {
    $candidate = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe'
    if (Test-Path -LiteralPath $candidate) { $XrayBin = $candidate }
  }
  $portBusy = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
  if (-not $XrayBin -or -not (Test-Path -LiteralPath $XrayBin)) {
    $results.limitations += 'seeded apply skipped: xray binary not found'
    Log 'seeded apply skipped: xray binary not found'
  } elseif ($portBusy) {
    $results.limitations += 'seeded apply skipped: port 11808 already in use'
    Log 'seeded apply skipped: port 11808 already in use'
  } else {
    # Seed a fresh app-data dir. (Seeding a dir the app has already opened
    # would collide with net_host's data-dir lock, so this is its own dir.)
    $seedDir = Join-Path $workRoot 'data_seed_app'
    New-Item -ItemType Directory -Path $seedDir -Force | Out-Null
    Log "seeding ${seedDir} via bridge_api example t18b_seed"
    & cargo run --manifest-path (Join-Path $RepoRoot 'Cargo.toml') -p bridge_api --example t18b_seed --release -- $seedDir 2>&1 | ForEach-Object { Log "   seed> $_" }
    if ($LASTEXITCODE -ne 0) {
      $results.limitations += 'seeded apply skipped: t18b_seed failed'
    } else {
      $runRoot = Join-Path $workRoot 'run'
      $pipe = "\\.\pipe\t20smoke" + [guid]::NewGuid().ToString('N')
      $extra = @{
        V2RAYN_R_AUTO_SMOKE = '1'
        V2RAYN_R_XRAY_BIN = $XrayBin
        V2RAYN_R_NET_HOST = (Join-Path $pkg 'net_host.exe')
        V2RAYN_R_RUN_ROOT = $runRoot
        V2RAYN_R_PIPE = $pipe
      }
      $results.runs += Invoke-SmokeRun -Name 'seed_apply' -ExtraEnv $extra -ScreenshotName 'T20-smoke-seed-apply.png' -DataDirOverride $seedDir
    }
  }
}

$results.limitations += 'packaged apply is only verified if run seed_apply reports applied=true; GUI click-through is not automated'
$summaryPath = Join-Path $EvidenceDir 'T20-smoke-summary.json'
$results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
$logPath = Join-Path $EvidenceDir 'T20-smoke-log.txt'
$script:hr | Set-Content -LiteralPath $logPath -Encoding UTF8

Write-Host ""
Write-Host "SMOKE SUMMARY  $summaryPath"
Write-Host "SMOKE LOG      $logPath"
Write-Host "T20_SMOKE_DONE work=$workRoot"
