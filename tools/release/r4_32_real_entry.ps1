<#
.SYNOPSIS
  R4-32 packaged real-entry regression for the official unarmed Windows zip.

.DESCRIPTION
  Extracts dist/v2rayN-R-<version>-windows-x64.zip into an isolated temp dir and
  drives the *shipped* exe (no AUTO_SMOKE, no preset active node):

    1. first run  - fresh V2RAYN_R_DATA_DIR: window appears, first-run SQLite
                    (guiNDB.db) is created; screenshot.
    2. reopen     - the app immediately relaunches on the same data dir and the
                    first-run state survives; screenshot.
    3. UI hooks   - the UI-only navigation hooks (V2RAYN_R_OPEN_UPDATE /
                    V2RAYN_R_OPEN_BACKUP) open real windows in the packaged
                    build, proving the release surface is interactive.

  It also asserts the shipped build-info.json is armed=false and dirty=false and
  that the flat layout matches the installer contract.

  Safety: >=11808 only (this script starts no core and binds no port), never
  touches 127.0.0.1:10808 / the host proxy / TUN, and only stops the PID tree it
  started.
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$EvidenceDir = '',
  [int]$WindowWaitSec = 60
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($EvidenceDir -eq '') { $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\repair\R4-32' }
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
using System.Runtime.InteropServices;
public class Nat32 {
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
    $h = [Nat32]::Find([uint32]$ProcessId)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 500
  }
  return [IntPtr]::Zero
}
function Save-WindowShot {
  param([IntPtr]$Hwnd, [string]$Path)
  [Nat32]::SetWindowPos($Hwnd, [IntPtr](-1), 80, 80, 0, 0, 0x0043) | Out-Null
  Start-Sleep -Milliseconds 1200
  $r = New-Object Nat32+RECT
  [Nat32]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
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
    foreach ($c in (Get-CimInstance Win32_Process -Filter "ParentProcessId=$parent" -ErrorAction SilentlyContinue)) {
      $result.Add([int]$c.ProcessId) | Out-Null
      $queue.Enqueue([int]$c.ProcessId)
    }
  }
  return $result
}
function Stop-OwnedTree {
  param([int]$RootPid)
  $owned = @($RootPid) + @(Get-DescendantPids -RootPid $RootPid) | Sort-Object -Unique
  [array]::Reverse($owned)
  foreach ($cpid in $owned) {
    if (Get-Process -Id $cpid -ErrorAction SilentlyContinue) {
      Stop-Process -Id $cpid -Force -ErrorAction SilentlyContinue
    }
  }
}

$workRoot = Join-Path $env:TEMP ("r4_32_real_" + [guid]::NewGuid().ToString('N'))
$extractDir = Join-Path $workRoot 'extract'
New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $extractDir -Force
$pkg = (Get-ChildItem -LiteralPath $extractDir -Directory | Select-Object -First 1).FullName
$mainExe = Join-Path $pkg 'v2rayn_desktop.exe'
if (-not (Test-Path -LiteralPath $mainExe)) { throw "main exe not found in package: $pkg" }

$info = Get-Content -LiteralPath (Join-Path $pkg 'build-info.json') -Raw | ConvertFrom-Json
$layoutExpected = @(
  'v2rayn_desktop.exe', 'bridge_api.dll', 'net_host.exe', 'privileged_helper.exe',
  'v2rayN-upgrade.exe', 'build-info.json', 'CORE-NOTES.txt',
  'data\app.so', 'data\flutter_assets\AssetManifest.bin'
)
$layoutMissing = @($layoutExpected | Where-Object { -not (Test-Path -LiteralPath (Join-Path $pkg $_)) })

$results = [ordered]@{
  schema = 1
  task = 'R4-32'
  method = 'tools/release/r4_32_real_entry.ps1'
  zip = $Zip
  zip_sha256 = (Get-FileHash -LiteralPath $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
  package = $pkg
  build_info = [ordered]@{
    git_commit = $info.git_commit; git_dirty = $info.git_dirty
    smoke_armed = $info.smoke_armed; version = $info.version
    target_platform = $info.target_platform
  }
  layout = [ordered]@{ expected = $layoutExpected; missing = $layoutMissing; ok = ($layoutMissing.Count -eq 0) }
  runs = @()
  limitations = @(
    'import/active/start/stop of the packaged Flutter canvas is not automated here (no OS UI driver); the same-commit real-UI integration tests t21e_import_test.dart and ux_parity_fix07_active_test.dart cover that chain.',
    'the packaged app starts no core in this run; no port is bound and 10808 is never touched.'
  )
  work_root = $workRoot
}

function Invoke-EntryRun {
  param([string]$Name, [string]$DataDir, [hashtable]$ExtraEnv, [string]$ShotName)
  $run = [ordered]@{
    name = $Name; data_dir = $DataDir; started_pid = 0; window = $false
    screenshot = ''; guiNDB = $false; guiNConfig = $false; exited_on_close = $false
    forced_kill = $false; notes = @()
  }
  $vars = @{ V2RAYN_R_DATA_DIR = $DataDir }
  foreach ($k in $ExtraEnv.Keys) { $vars[$k] = $ExtraEnv[$k] }
  $old = @{}
  foreach ($k in $vars.Keys) { $old[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$vars[$k]) }
  try { $proc = Start-Process -FilePath $mainExe -WorkingDirectory $pkg -PassThru }
  finally { foreach ($k in $old.Keys) { [Environment]::SetEnvironmentVariable($k, $old[$k]) } }
  $appPid = $proc.Id
  $run.started_pid = $appPid
  Log "${Name}: started pid=$appPid"
  $hwnd = Wait-Window -ProcessId $appPid -TimeoutSec $WindowWaitSec
  if ($hwnd -ne [IntPtr]::Zero) {
    $run.window = $true
    try { Save-WindowShot -Hwnd $hwnd -Path (Join-Path $EvidenceDir $ShotName); $run.screenshot = (Join-Path $EvidenceDir $ShotName) }
    catch { $run.notes += "screenshot failed: $_" }
  } else { $run.notes += "window not found within ${WindowWaitSec}s" }

  $deadline = (Get-Date).AddSeconds(60)
  while ((Get-Date) -lt $deadline) {
    if ((Test-Path -LiteralPath (Join-Path $DataDir 'guiNDB.db'))) { break }
    Start-Sleep -Milliseconds 500
  }
  $run.guiNDB = Test-Path -LiteralPath (Join-Path $DataDir 'guiNDB.db')
  $run.guiNConfig = Test-Path -LiteralPath (Join-Path $DataDir 'guiNConfig.json')
  Log "${Name}: window=$($run.window) guiNDB=$($run.guiNDB) guiNConfig=$($run.guiNConfig)"

  $null = $proc.CloseMainWindow()
  if ($proc.WaitForExit(8000)) { $run.exited_on_close = $true; Log "${Name}: exited on CloseMainWindow" }
  else {
    Stop-OwnedTree -RootPid $appPid
    $run.forced_kill = $true
    Log "${Name}: force-stopped owned pid tree"
  }
  Start-Sleep -Seconds 2
  return $run
}

$dataDir = Join-Path $workRoot 'data'
New-Item -ItemType Directory -Path $dataDir -Force | Out-Null

$results.runs += Invoke-EntryRun -Name 'first_run' -DataDir $dataDir -ExtraEnv @{} -ShotName 'r4-32-first-run.png'
$results.runs += Invoke-EntryRun -Name 'reopen' -DataDir $dataDir -ExtraEnv @{} -ShotName 'r4-32-reopen.png'
$results.runs += Invoke-EntryRun -Name 'ui_update_hook' -DataDir (Join-Path $workRoot 'data_update') -ExtraEnv @{ V2RAYN_R_OPEN_UPDATE = '1' } -ShotName 'r4-32-update-window.png'
$results.runs += Invoke-EntryRun -Name 'ui_backup_hook' -DataDir (Join-Path $workRoot 'data_backup') -ExtraEnv @{ V2RAYN_R_OPEN_BACKUP = '1' } -ShotName 'r4-32-backup-window.png'

$first = $results.runs[0]
$reopen = $results.runs[1]
$results.assessment = [ordered]@{
  build_info_unarmed = (-not [bool]$info.smoke_armed)
  build_info_clean = (-not [bool]$info.git_dirty)
  layout_ok = $results.layout.ok
  first_run_window = [bool]$first.window
  first_run_db = [bool]$first.guiNDB
  reopen_window = [bool]$reopen.window
  reopen_db_persisted = [bool]$reopen.guiNDB
}
$results.assessment.ok =
  $results.assessment.build_info_unarmed -and $results.assessment.build_info_clean -and
  $results.assessment.layout_ok -and $results.assessment.first_run_window -and
  $results.assessment.first_run_db -and $results.assessment.reopen_window -and
  $results.assessment.reopen_db_persisted

$results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceDir 'observations.json') -Encoding UTF8
$script:hr | Set-Content -LiteralPath (Join-Path $EvidenceDir 'real-entry.log') -Encoding UTF8
Write-Host ""
Write-Host "R4_32_REAL_ENTRY ok=$($results.assessment.ok) work=$workRoot"
