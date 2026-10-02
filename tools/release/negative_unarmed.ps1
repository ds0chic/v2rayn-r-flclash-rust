<#
.SYNOPSIS
  ISSUE-08 negative test (F-04 verification): the OFFICIAL, non-armed release
  package must NOT respond to the smoke/benchmark automation environment.

.DESCRIPTION
  Seeds a fresh data dir, then launches the official (smoke_armed=false) package
  with every state-changing automation variable set:
    V2RAYN_R_AUTO_SMOKE=1, V2RAYN_R_AUTOSTART=1,
    V2RAYN_R_T18_BENCH=1 (+ scenario/rows/dir), and a managed core path.
  It runs for a bounded window and asserts that the app:
    * spawns no core (xray / sing-box) descendant,
    * does not listen on 11808,
    * writes no applied journal under the run root,
    * writes no core.log / T18 benchmark output.

  The result JSON is written to docs/evidence/T20.runs/rc-apply/negative-unarmed.json
  (fields: pid, env, children, port11808, journal, result, ...).

  Safety: same bounds as the other release scripts — >=11808 only, never touches
  10808 / the system proxy / TUN, and only stops the PID it started plus its
  recorded descendants.
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$XrayBin = '',
  [string]$OutDir = '',
  [int]$RunSec = 35
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($OutDir -eq '') { $OutDir = Join-Path $RepoRoot 'docs\evidence\T20.runs\rc-apply' }
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

if ($Zip -eq '') {
  # Explicitly the official dist package (never dist/evidence-armed).
  $Zip = (Get-ChildItem -Path (Join-Path $RepoRoot 'dist') -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Zip -or -not (Test-Path -LiteralPath $Zip)) { throw "official zip not found: $Zip" }
$Zip = (Resolve-Path -LiteralPath $Zip).Path
if ($XrayBin -eq '') { $XrayBin = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe' }
$xrayResolved = if (Test-Path -LiteralPath $XrayBin) { (Resolve-Path $XrayBin).Path } else { $XrayBin }

$busy = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
if ($busy) { throw 'port 11808 already in use; refusing to run' }

$w = Join-Path $env:TEMP ("t20_neg_" + [guid]::NewGuid().ToString('N'))
$extract = Join-Path $w 'extract'
$seed = Join-Path $w 'data'
$run = Join-Path $w 'run'
$bench = Join-Path $w 'bench'
New-Item -ItemType Directory -Path $extract, $seed, $run, $bench -Force | Out-Null

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
function Describe-Pid {
  param([int]$TargetPid)
  $p = Get-Process -Id $TargetPid -ErrorAction SilentlyContinue
  if (-not $p) { return $null }
  $started = $null
  try { $started = $p.StartTime.ToUniversalTime().ToString('o') } catch {}
  [ordered]@{ pid = $TargetPid; name = $p.ProcessName; start_time_utc = $started }
}

Expand-Archive -LiteralPath $Zip -DestinationPath $extract -Force
$pkg = (Get-ChildItem -LiteralPath $extract -Directory | Select-Object -First 1).FullName
$mainExe = Join-Path $pkg 'v2rayn_desktop.exe'
$netHost = Join-Path $pkg 'net_host.exe'
if (-not (Test-Path -LiteralPath $mainExe)) { throw "main exe missing in $pkg" }

$armedFlag = $null
$pkgInfo = Join-Path $pkg 'build-info.json'
if (Test-Path -LiteralPath $pkgInfo) {
  try { $armedFlag = [bool](Get-Content -LiteralPath $pkgInfo -Raw | ConvertFrom-Json).smoke_armed } catch {}
}

$seedOut = & cargo run -q --manifest-path (Join-Path $RepoRoot 'Cargo.toml') -p bridge_api --example t18b_seed --release -- $seed 2>&1
if ($LASTEXITCODE -ne 0) { throw "t18b_seed failed: $seedOut" }

$vars = @{
  V2RAYN_R_DATA_DIR    = $seed
  V2RAYN_R_AUTO_SMOKE  = '1'
  V2RAYN_R_AUTOSTART   = '1'
  V2RAYN_R_T18_BENCH   = '1'
  V2RAYN_R_T18_SCENARIO = 'startup'
  V2RAYN_R_T18_ROWS    = '100'
  V2RAYN_R_T18_DIR     = $bench
  V2RAYN_R_T18_TAG     = 'negative-unarmed'
  V2RAYN_R_XRAY_BIN    = $xrayResolved
  V2RAYN_R_NET_HOST    = $netHost
  V2RAYN_R_RUN_ROOT    = $run
  V2RAYN_R_PIPE        = "\\.\pipe\t20neg" + [guid]::NewGuid().ToString('N')
}
$old = @{}
foreach ($k in $vars.Keys) { $old[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, [string]$vars[$k]) }
try { $p = Start-Process -FilePath $mainExe -WorkingDirectory $pkg -PassThru }
finally { foreach ($k in $old.Keys) { [Environment]::SetEnvironmentVariable($k, $old[$k]) } }
$appPid = $p.Id

$appStarted = $null
try { $appStarted = (Get-Process -Id $appPid).StartTime.ToUniversalTime().ToString('o') } catch {}

# Bounded observation window.
$deadline = (Get-Date).AddSeconds($RunSec)
$portEverSeen = $false
while ((Get-Date) -lt $deadline) {
  if (-not (Get-Process -Id $appPid -ErrorAction SilentlyContinue)) { break }
  if ([bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)) { $portEverSeen = $true }
  Start-Sleep -Milliseconds 500
}

$desc = @(Get-DescendantPids -RootPid $appPid)
$children = @($desc | ForEach-Object { Describe-Pid $_ } | Where-Object { $_ })
$coreChildren = @($desc | Where-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).ProcessName -match 'xray|sing-box' })
$netHostChildren = @($desc | Where-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).ProcessName -match 'net_host' })
$port11808 = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
$portEverSeen = $portEverSeen -or $port11808

$journals = @()
foreach ($f in (Get-ChildItem -Path $run -Recurse -Filter 'journal.json' -ErrorAction SilentlyContinue)) {
  try { $j = Get-Content -LiteralPath $f.FullName -Raw | ConvertFrom-Json; $journals += [ordered]@{ file = $f.FullName; stage = $j.stage; port = $j.port } } catch {}
}
$coreLogs = @(Get-ChildItem -Path $run -Recurse -Filter 'core.log' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
$appliedJournal = @($journals | Where-Object { $_.stage -eq 'applied' })
$benchFiles = @(Get-ChildItem -Path $bench -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })

# Graceful stop, bounded.
$null = $p.CloseMainWindow()
$exited = $p.WaitForExit(8000)
if (-not $exited) { Stop-Process -Id $appPid -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2
$stopped = @($appPid) + $desc | Sort-Object -Unique
$stoppedList = [System.Collections.Generic.List[int]]::new()
[array]::Reverse($stopped)
foreach ($cpid in $stopped) {
  if (Get-Process -Id $cpid -ErrorAction SilentlyContinue) { Stop-Process -Id $cpid -Force -ErrorAction SilentlyContinue; $stoppedList.Add($cpid) | Out-Null }
}

$ok = (-not $coreChildren) -and (-not $portEverSeen) -and ($appliedJournal.Count -eq 0) -and ($coreLogs.Count -eq 0) -and ($benchFiles.Count -eq 0)
$result = [ordered]@{
  schema = 1
  method = 'tools/release/negative_unarmed.ps1'
  zip = $Zip
  package = $pkg
  package_smoke_armed = $armedFlag
  pid = $appPid
  pid_start_time_utc = $appStarted
  env = $vars
  run_seconds = $RunSec
  children = $children
  net_host_descendant = ($netHostChildren.Count -gt 0)
  core_descendant = ($coreChildren.Count -gt 0)
  port11808 = $port11808
  port11808_ever_seen = $portEverSeen
  journal = $journals
  core_logs = $coreLogs
  t18_bench_output = $benchFiles
  result = if ($ok) { 'PASS: official package ignored all automation env vars (no core, no 11808, no applied journal, no core.log, no bench output)' } else { 'FAIL' }
  cleanup = [ordered]@{ stopped = $stoppedList; self_alive_after = [bool](Get-Process -Id $appPid -ErrorAction SilentlyContinue) }
  port_11808_after = [bool](Get-NetTCPConnection -LocalPort 11808 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1)
  work_root = $w
}
$result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $OutDir 'negative-unarmed.json') -Encoding UTF8
Write-Host ""
Write-Host "NEGATIVE_RESULT $($result.result)"
Write-Host "NEGATIVE_JSON   $(Join-Path $OutDir 'negative-unarmed.json')"
