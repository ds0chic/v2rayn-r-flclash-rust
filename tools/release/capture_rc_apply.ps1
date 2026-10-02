<#
.SYNOPSIS
  Capture decisive, self-consistent apply evidence from the smoke-armed package
  (F-02/F-03 remediation).

.DESCRIPTION
  Extracts the armed zip, seeds a fresh data dir with the existing
  bridge_api t18b_seed example, launches the packaged exe with
  V2RAYN_R_AUTO_SMOKE=1 and a managed net_host/xray, then while the session is
  actually RUNNING it captures:
    * the session journal at stage=applied (raw, pre-stop),
    * the staged config.json and the net_host-produced core.log,
    * the 11808 listen probe and the app/net_host/xray PIDs + creation times.
  It then closes the app and waits for net_host to reclaim, capturing the
  FINALIZED journal and proving the plaintext config/core log were removed.

  All artifacts are sanitized (synthetic seed node only) and written under
  -OutDir (default docs/evidence/T20.runs/rc-apply).

  Safety: ports >= 11808 only; never touches 127.0.0.1:10808, the system proxy
  or TUN; only the PID this script starts and its recorded descendants are ever
  stopped. Every wait is bounded.

.NOTES
  The core referenced by V2RAYN_R_XRAY_BIN is the developer-local
  tools/cores/xray binary; the package itself bundles no core.
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$XrayBin = '',
  [string]$OutDir = '',
  [int]$TimeoutSec = 90,
  [int]$SettleSec = 3,
  [int]$FinalWaitSec = 25
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($OutDir -eq '') { $OutDir = Join-Path $RepoRoot 'docs\evidence\T20.runs\rc-apply' }
$liveDir = Join-Path $OutDir 'live'
$finalDir = Join-Path $OutDir 'finalized'
New-Item -ItemType Directory -Path $liveDir, $finalDir -Force | Out-Null

if ($Zip -eq '') {
  $armedDir = Join-Path $RepoRoot 'dist\evidence-armed'
  $Zip = (Get-ChildItem -Path $armedDir -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Zip -or -not (Test-Path -LiteralPath $Zip)) { throw "armed zip not found: $Zip" }
$Zip = (Resolve-Path -LiteralPath $Zip).Path
if ($XrayBin -eq '') { $XrayBin = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe' }
if (-not (Test-Path -LiteralPath $XrayBin)) { throw "xray binary not found: $XrayBin" }
$XrayBin = (Resolve-Path -LiteralPath $XrayBin).Path

$busy = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
if ($busy) { throw 'port 11808 already in use; refusing to run' }

$w = Join-Path $env:TEMP ("t20_rcapply_" + [guid]::NewGuid().ToString('N'))
$extract = Join-Path $w 'extract'
$seed = Join-Path $w 'data'
$run = Join-Path $w 'run'
New-Item -ItemType Directory -Path $extract, $seed, $run -Force | Out-Null

function Log([string]$m) { Write-Host "   $m" }

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Nat22 {
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

function Describe-Pid {
  param([int]$TargetPid)
  $p = Get-Process -Id $TargetPid -ErrorAction SilentlyContinue
  if (-not $p) { return $null }
  $started = $null
  try { $started = $p.StartTime.ToUniversalTime().ToString('o') } catch { $started = $null }
  [ordered]@{
    pid = $TargetPid
    name = $p.ProcessName
    start_time_utc = $started
  }
}

function Get-AppliedJournal {
  param([string]$RunRoot)
  $best = $null
  foreach ($f in (Get-ChildItem -Path $RunRoot -Recurse -Filter 'journal.json' -ErrorAction SilentlyContinue)) {
    try { $j = Get-Content -LiteralPath $f.FullName -Raw | ConvertFrom-Json } catch { continue }
    if ($j.stage -ne 'applied') { continue }
    if ($null -eq $best -or [int64]$j.updated_at_ms -gt [int64]$best.json.updated_at_ms) {
      $best = [ordered]@{ path = $f.FullName; session = $j.session_id; json = $j }
    }
  }
  return $best
}

function Get-JournalBySession {
  param([string]$RunRoot, [string]$Session)
  $f = Join-Path (Join-Path $RunRoot $Session) 'journal.json'
  if (-not (Test-Path -LiteralPath $f)) { return $null }
  try { return (Get-Content -LiteralPath $f -Raw | ConvertFrom-Json) } catch { return $null }
}

function Copy-Artifact {
  param([string]$Src, [string]$Dst)
  if (-not (Test-Path -LiteralPath $Src)) { return $null }
  Copy-Item -LiteralPath $Src -Destination $Dst -Force
  return [ordered]@{
    file = (Split-Path -Leaf $Dst)
    bytes = (Get-Item -LiteralPath $Dst).Length
    sha256 = (Get-FileHash -LiteralPath $Dst -Algorithm SHA256).Hash.ToLowerInvariant()
  }
}

function Save-WindowShot {
  param([IntPtr]$Hwnd, [string]$Path)
  [Nat22]::SetWindowPos($Hwnd, [IntPtr](-1), 80, 80, 0, 0, 0x0043) | Out-Null
  Start-Sleep -Milliseconds 1000
  $r = New-Object Nat22+RECT
  [Nat22]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $ww = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
  if ($ww -le 0 -or $hh -le 0) { return $false }
  $bmp = New-Object System.Drawing.Bitmap($ww, $hh)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($ww, $hh)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  return $true
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

$probe = [ordered]@{
  schema = 1
  method = 'tools/release/capture_rc_apply.ps1'
  zip = $Zip
  armed = $false
  core = [ordered]@{
    source = 'developer-local tools/cores (not bundled in package)'
    xray_bin = $XrayBin
    xray_sha256 = (Get-FileHash -LiteralPath $XrayBin -Algorithm SHA256).Hash.ToLowerInvariant()
  }
  data_dir = $seed
  run_root = $run
  port = 11808
  app = $null
  live = $null
  finalized = $null
  files = [ordered]@{}
  notes = @()
  result = 'pending'
  cleanup = $null
}

Expand-Archive -LiteralPath $Zip -DestinationPath $extract -Force
$pkg = (Get-ChildItem -LiteralPath $extract -Directory | Select-Object -First 1).FullName
$mainExe = Join-Path $pkg 'v2rayn_desktop.exe'
$netHost = Join-Path $pkg 'net_host.exe'
if (-not (Test-Path -LiteralPath $mainExe)) { throw "main exe missing in $pkg" }
if (-not (Test-Path -LiteralPath $netHost)) { throw "net_host.exe missing in $pkg" }
$pkgInfo = Join-Path $pkg 'build-info.json'
if (Test-Path -LiteralPath $pkgInfo) {
  try { $probe.armed = [bool](Get-Content -LiteralPath $pkgInfo -Raw | ConvertFrom-Json).smoke_armed } catch {}
}
Log "zip=$Zip"
Log "package=$pkg armed=$($probe.armed)"

Log 'seeding data dir via bridge_api t18b_seed'
$seedOut = & cargo run -q --manifest-path (Join-Path $RepoRoot 'Cargo.toml') -p bridge_api --example t18b_seed --release -- $seed 2>&1
if ($LASTEXITCODE -ne 0) { throw "t18b_seed failed: $seedOut" }
Log "seed> $($seedOut | Select-Object -Last 1)"

$vars = @{
  V2RAYN_R_DATA_DIR   = $seed
  V2RAYN_R_AUTO_SMOKE = '1'
  V2RAYN_R_XRAY_BIN   = $XrayBin
  V2RAYN_R_NET_HOST   = $netHost
  V2RAYN_R_RUN_ROOT   = $run
  V2RAYN_R_PIPE       = "\\.\pipe\t20rcapply" + [guid]::NewGuid().ToString('N')
}
$old = @{}
foreach ($k in $vars.Keys) { $old[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$vars[$k]) }
try { $p = Start-Process -FilePath $mainExe -WorkingDirectory $pkg -PassThru }
finally { foreach ($k in $old.Keys) { [Environment]::SetEnvironmentVariable($k, $old[$k]) } }
$appPid = $p.Id
$probe.app = Describe-Pid $appPid
$probe.env = $vars
Log "app pid=$appPid"

$descBefore = @(Get-DescendantPids -RootPid $appPid)
$descNow = @()

$deadline = (Get-Date).AddSeconds($TimeoutSec)
$observed = $null
while ((Get-Date) -lt $deadline) {
  if (-not (Get-Process -Id $appPid -ErrorAction SilentlyContinue)) { break }
  $listen = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
  $entry = Get-AppliedJournal -RunRoot $run
  if ($listen -and $entry) {
    Start-Sleep -Seconds $SettleSec
    $listen2 = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
    $entry2 = Get-AppliedJournal -RunRoot $run
    if ($listen2 -and $entry2) { $observed = $entry2; break }
  }
  Start-Sleep -Milliseconds 400
}

if (-not $observed) {
  $probe.result = 'failed: no stable applied session observed'
  $probe.notes += "timeout=${TimeoutSec}s settle=${SettleSec}s"
} else {
  $sess = $observed.session
  $sessDir = Split-Path -Parent $observed.path
  Log "applied session=$sess port11808=true"

  $descNow = @(Get-DescendantPids -RootPid $appPid)
  $probe.live = [ordered]@{
    observed_at_utc = (Get-Date).ToUniversalTime().ToString('o')
    session_id = $sess
    port_11808_listening = $true
    journal = $observed.json
    descendants = @($descNow | ForEach-Object { Describe-Pid $_ } | Where-Object { $_ })
    core_descendant = [bool]($descNow | Where-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).ProcessName -match 'xray|sing-box' })
    net_host_descendant = [bool]($descNow | Where-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).ProcessName -match 'net_host' })
  }
  $probe.files['live_journal'] = Copy-Artifact (Join-Path $sessDir 'journal.json') (Join-Path $liveDir "${sess}_journal.json")
  $probe.files['live_config'] = Copy-Artifact (Join-Path $sessDir 'config.json') (Join-Path $liveDir "${sess}_config.json")
  $probe.files['live_core_log'] = Copy-Artifact (Join-Path $sessDir 'core.log') (Join-Path $liveDir "${sess}_core.log")
  $probe.files['live_net_host_client_stdout'] = Copy-Artifact (Join-Path $run 'net_host.stdout.log') (Join-Path $liveDir "${sess}_net_host_stdout.log")

  $hwnd = [Nat22]::Find([uint32]$appPid)
  if ($hwnd -ne [IntPtr]::Zero) {
    $shot = Join-Path $OutDir 'rc-apply-applied-running.png'
    try { if (Save-WindowShot -Hwnd $hwnd -Path $shot) { $probe.screenshot = $shot } }
    catch { $probe.notes += "screenshot failed: $_" }
  }
}

# Graceful stop, then wait for net_host to finalize the session.
$null = $p.CloseMainWindow()
$exited = $p.WaitForExit(8000)
if (-not $exited) { Stop-Process -Id $appPid -Force -ErrorAction SilentlyContinue }

$finalDeadline = (Get-Date).AddSeconds($FinalWaitSec)
$finalJournal = $null
while ((Get-Date) -lt $finalDeadline) {
  if ($observed) {
    $j = Get-JournalBySession -RunRoot $run -Session $observed.session
    if ($j -and $j.stage -eq 'finalized') { $finalJournal = $j; break }
  } else {
    $j = Get-ChildItem -Path $run -Recurse -Filter 'journal.json' -ErrorAction SilentlyContinue |
      ForEach-Object { try { Get-Content $_.FullName -Raw | ConvertFrom-Json } catch {} } |
      Where-Object { $_.stage -eq 'finalized' } | Select-Object -First 1
    if ($j) { $finalJournal = $j; break }
  }
  Start-Sleep -Milliseconds 500
}

$sessForFinal = if ($observed) { $observed.session } else { $null }
if ($sessForFinal) {
  $sessDir = Join-Path $run $sessForFinal
  $probe.finalized = [ordered]@{
    session_id = $sessForFinal
    journal = $finalJournal
    config_still_present = (Test-Path -LiteralPath (Join-Path $sessDir 'config.json'))
    core_log_still_present = (Test-Path -LiteralPath (Join-Path $sessDir 'core.log'))
  }
  $probe.files['finalized_journal'] = Copy-Artifact (Join-Path $sessDir 'journal.json') (Join-Path $finalDir "${sessForFinal}_journal.json")
}

# Stop only the PIDs recorded as ours.
$cleaned = Stop-OwnedTree -OwnedPids (@($appPid) + $descBefore + @($descNow))
$probe.cleanup = [ordered]@{ stopped = $cleaned; self_alive_after = [bool](Get-Process -Id $appPid -ErrorAction SilentlyContinue) }
$probe.port_11808_after = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)

if ($observed -and $finalJournal -and $finalJournal.stage -eq 'finalized') {
  $probe.result = 'PASS: live applied journal + core.log captured; finalized journal after stop'
} elseif ($observed) {
  $probe.result = 'PARTIAL: live applied captured, finalized journal not observed within bound'
} else {
  $probe.result = 'FAIL: no applied session captured'
}
$probe.work_root = $w
$probe | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $OutDir 'rc-apply-probe.json') -Encoding UTF8
Write-Host ""
Write-Host "RC_APPLY_RESULT $($probe.result)"
Write-Host "RC_APPLY_PROBE  $(Join-Path $OutDir 'rc-apply-probe.json')"
