# T18 performance runner.
#
# Drives the release build in benchmark mode (V2RAYN_R_T18_BENCH=1) to measure
# cold start to first frame, virtualized-table scroll frame times, and process
# working set, then folds the Rust log-buffer throughput line into the summary.
#
# Safety: every launch uses an isolated data dir and a synthetic-only bridge;
# no core is started, port 10808 is never used, and only PIDs this script
# started are ever stopped. Every wait is bounded (a timeout is a failure, not
# a hang).
#
# Usage: pwsh -NoProfile -File benchmarks/T18/run_perf.ps1

param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
  [int]$StartupIterations = 10,
  [int]$StartupRows = 1000,
  [int[]]$ScrollRows = @(10000, 50000),
  [int]$ScrollSteps = 600,
  [int]$HoldSeconds = 6
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Exe = Join-Path $RepoRoot 'apps\desktop\build\windows\x64\runner\Release\v2rayn_desktop.exe'
$OutDir = Join-Path $RepoRoot 'benchmarks\T18'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if (-not (Test-Path $Exe)) { throw "release build not found: $Exe" }

function Percentile([double[]]$values, [int]$p) {
  if ($values.Count -eq 0) { return 0 }
  $sorted = $values | Sort-Object
  $index = [math]::Round(($p / 100.0) * ($sorted.Count - 1))
  return $sorted[[int]$index]
}

function Set-BenchEnv([string]$scenario, [int]$rows, [string]$tag) {
  $env:V2RAYN_R_T18_BENCH = '1'
  $env:V2RAYN_R_T18_SCENARIO = $scenario
  $env:V2RAYN_R_T18_ROWS = "$rows"
  $env:V2RAYN_R_T18_STEPS = "$ScrollSteps"
  $env:V2RAYN_R_T18_DIR = $OutDir
  $env:V2RAYN_R_T18_TAG = $tag
}

function Clear-BenchEnv {
  foreach ($name in @('V2RAYN_R_T18_BENCH', 'V2RAYN_R_T18_SCENARIO', 'V2RAYN_R_T18_ROWS', 'V2RAYN_R_T18_STEPS', 'V2RAYN_R_T18_DIR', 'V2RAYN_R_T18_TAG', 'V2RAYN_R_DATA_DIR')) {
    Remove-Item "Env:$name" -ErrorAction SilentlyContinue
  }
}

function Wait-File([string]$path, $proc, [int]$timeoutMs) {
  $deadline = (Get-Date).AddMilliseconds($timeoutMs)
  while ((Get-Date) -lt $deadline) {
    if (Test-Path $path) { return $true }
    if ($proc -and $proc.HasExited) { return (Test-Path $path) }
    Start-Sleep -Milliseconds 20
  }
  return $false
}

function Stop-Owned($proc) {
  if ($proc -and -not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  }
}

# ---------------------------------------------------------------------------
# Cold start to first frame
# ---------------------------------------------------------------------------
$startupRuns = New-Object System.Collections.Generic.List[object]
for ($i = 1; $i -le $StartupIterations; $i++) {
  $runDir = Join-Path $env:TEMP "t18_start_$i`_$([guid]::NewGuid().ToString('N').Substring(0,6))"
  $dataDir = "$runDir-data"
  New-Item -ItemType Directory -Force -Path $runDir | Out-Null
  $marker = Join-Path $OutDir "startup.json"
  Remove-Item -LiteralPath $marker -ErrorAction SilentlyContinue
  Set-BenchEnv 'startup' $StartupRows ''
  $env:V2RAYN_R_DATA_DIR = $dataDir
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $proc = Start-Process -FilePath $Exe -PassThru
  $ok = Wait-File $marker $proc 60000
  $sw.Stop()
  $launchMs = $sw.Elapsed.TotalMilliseconds
  $internal = $null
  if ($ok) {
    try {
      $json = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
      $internal = [double]$json.main_to_first_frame_ms
    } catch { }
  }
  if (-not $proc.HasExited) {
    [void]$proc.WaitForExit(5000)
    Stop-Owned $proc
  }
  Stop-Owned $proc
  $startupRuns.Add([pscustomobject]@{
      iteration            = $i
      marker               = $ok
      launch_to_marker_ms  = [math]::Round($launchMs, 2)
      main_to_first_frame_ms = $internal
    })
  Remove-Item -Recurse -Force $runDir -ErrorAction SilentlyContinue
  Clear-BenchEnv
}

$launchValues = @($startupRuns | Where-Object { $_.marker } | ForEach-Object { $_.launch_to_marker_ms })
$internalValues = @($startupRuns | Where-Object { $null -ne $_.main_to_first_frame_ms } | ForEach-Object { $_.main_to_first_frame_ms })
$startupSummary = [ordered]@{
  iterations          = $StartupIterations
  rows                = $StartupRows
  method              = 'process launch -> first_frame marker file and in-process main()->first_frame'
  samples_ok          = $launchValues.Count
  launch_p50_ms       = Percentile $launchValues 50
  launch_p95_ms       = Percentile $launchValues 95
  internal_p50_ms     = Percentile $internalValues 50
  internal_p95_ms     = Percentile $internalValues 95
  runs                = $startupRuns
}
$startupSummary | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutDir 'startup_runs.json') -Encoding UTF8
Write-Output ("T18 startup ok={0}/{1} launch_p50={2}ms launch_p95={3}ms" -f $launchValues.Count, $StartupIterations, $startupSummary.launch_p50_ms, $startupSummary.launch_p95_ms)

# ---------------------------------------------------------------------------
# Scroll frame times
# ---------------------------------------------------------------------------
$scrollSummaries = @()
foreach ($rows in $ScrollRows) {
  $tag = ''
  $outFile = Join-Path $OutDir "scroll_$rows.json"
  Remove-Item -LiteralPath $outFile -ErrorAction SilentlyContinue
  $runDir = Join-Path $env:TEMP "t18_scroll_$rows`_$([guid]::NewGuid().ToString('N').Substring(0,6))"
  New-Item -ItemType Directory -Force -Path $runDir | Out-Null
  Set-BenchEnv 'scroll' $rows $tag
  $env:V2RAYN_R_DATA_DIR = "$runDir-data"
  $proc = Start-Process -FilePath $Exe -PassThru
  $ok = Wait-File $outFile $proc 300000
  Stop-Owned $proc
  if ($ok) {
    $json = Get-Content -LiteralPath $outFile -Raw | ConvertFrom-Json
    $scrollSummaries += [pscustomobject]@{
      rows            = $rows
      status          = 'ok'
      sample_count    = $json.summary.sample_count
      build_p50_us    = $json.summary.build_p50_us
      build_p95_us    = $json.summary.build_p95_us
      raster_p50_us   = $json.summary.raster_p50_us
      raster_p95_us   = $json.summary.raster_p95_us
      total_p50_us    = $json.summary.total_p50_us
      total_p95_us    = $json.summary.total_p95_us
      dropped_frames  = $json.summary.dropped_frames
      dropped_rate    = $json.summary.dropped_rate
      file            = $outFile
    }
    Write-Output ("T18 scroll rows={0} samples={1} build_p95={2}us raster_p95={3}us dropped_rate={4}" -f $rows, $json.summary.sample_count, $json.summary.build_p95_us, $json.summary.raster_p95_us, $json.summary.dropped_rate)
  } else {
    $scrollSummaries += [pscustomobject]@{ rows = $rows; status = 'timeout'; file = $outFile }
    Write-Output ("T18 scroll rows={0} TIMEOUT" -f $rows)
  }
  Remove-Item -Recurse -Force $runDir -ErrorAction SilentlyContinue
  Clear-BenchEnv
}

# ---------------------------------------------------------------------------
# Working set (idle / N rows)
# ---------------------------------------------------------------------------
$memoryRuns = @()
foreach ($rows in @(0, 10000)) {
  $runDir = Join-Path $env:TEMP "t18_mem_$rows`_$([guid]::NewGuid().ToString('N').Substring(0,6))"
  New-Item -ItemType Directory -Force -Path $runDir | Out-Null
  Set-BenchEnv 'hold' $rows ''
  $env:V2RAYN_R_DATA_DIR = "$runDir-data"
  $ready = Join-Path $OutDir 'ready.json'
  Remove-Item -LiteralPath $ready -ErrorAction SilentlyContinue
  $proc = Start-Process -FilePath $Exe -PassThru
  $ok = Wait-File $ready $proc 120000
  $samples = New-Object System.Collections.Generic.List[double]
  $deadline = (Get-Date).AddSeconds($HoldSeconds)
  while ($ok -and (Get-Date) -lt $deadline -and -not $proc.HasExited) {
    try {
      $p = Get-Process -Id $proc.Id -ErrorAction Stop
      $samples.Add([double]$p.WorkingSet64)
    } catch { break }
    Start-Sleep -Milliseconds 250
  }
  Stop-Owned $proc
  $arr = @($samples)
  $memoryRuns += [pscustomobject]@{
    rows            = $rows
    status          = if ($ok) { 'ok' } else { 'timeout' }
    sample_count    = $arr.Count
    working_set_p50_mb = if ($arr.Count) { [math]::Round((Percentile $arr 50) / 1MB, 2) } else { $null }
    working_set_max_mb = if ($arr.Count) { [math]::Round(($arr | Measure-Object -Maximum).Maximum / 1MB, 2) } else { $null }
  }
  Remove-Item -Recurse -Force $runDir -ErrorAction SilentlyContinue
  Clear-BenchEnv
}
$memoryRuns | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutDir 'memory.json') -Encoding UTF8

# ---------------------------------------------------------------------------
# Log-buffer throughput (Rust ring, real measurement)
# ---------------------------------------------------------------------------
$logBench = $null
$logBytes = $null
$cargo = Join-Path $HOME '.cargo\bin\cargo.exe'
Push-Location $RepoRoot
try {
  $out = & $cargo test -p application --test t18_stability --locked -- --nocapture 2>&1 | Out-String
} finally {
  Pop-Location
}
$m = [regex]::Match($out, 'T18_LOG_BENCH (\{.*\})')
if ($m.Success) { $logBench = $m.Groups[1].Value | ConvertFrom-Json }
$m2 = [regex]::Match($out, 'T18_LOG_BYTES (\{.*\})')
if ($m2.Success) { $logBytes = $m2.Groups[1].Value | ConvertFrom-Json }
[pscustomobject]@{ throughput = $logBench; bytes = $logBytes } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutDir 'log_throughput.json') -Encoding UTF8

$summary = [ordered]@{
  generated_at    = (Get-Date).ToString('o')
  exe             = $Exe
  startup         = $startupSummary
  scroll          = $scrollSummaries
  memory          = $memoryRuns
  log_throughput  = $logBench
  log_bytes       = $logBytes
  limitations     = @(
    'Cold start measured from process launch to a first-frame marker file (release); OS file cache is warm after iteration 1.',
    'Scroll uses scripted jumpTo (full-table rebuild), matching the T01 S1 skeleton; it is an upper-bound probe, not a real drag gesture.',
    '50k rows is a stress probe; synthetic rows come from the Rust generator via FRB.',
    'Memory is the app process WorkingSet64 while idle (hold scenario); no core process is involved.',
    'No 24h soak; this is a short sampled stability run.'
  )
}
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDir 'summary.json') -Encoding UTF8
Write-Output "T18 perf summary -> $(Join-Path $OutDir 'summary.json')"
