# T18 stability runner.
#
# 20x apply/stop loop against the real net_host + Xray smoke session, a single
# 30s runtime session with CPU/RSS sampling, the app-reopen drift measurement
# (Rust) and the existing T03 fault matrix. Port >= 11808 only; never 10808.
# Every launched process is recorded by PID and only those are ever stopped.
#
# Usage: pwsh -NoProfile -File benchmarks/T18/run_stability.ps1

param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
  [int]$Iterations = 20,
  [int]$Port = 11808,
  [int]$RuntimeHoldSeconds = 30,
  [switch]$SkipFaults
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$NetHost = Join-Path $RepoRoot 'target\debug\net_host.exe'
$Client = Join-Path $RepoRoot 'target\debug\t03_client.exe'
$Xray = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe'
$OutDir = Join-Path $RepoRoot 'benchmarks\T18'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$Stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$Script:OwnedPids = New-Object System.Collections.Generic.List[int]

function Stop-Owned([int]$procId) {
  if ($procId -le 0) { return }
  $p = Get-Process -Id $procId -ErrorAction SilentlyContinue
  if ($p) { Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue }
}

function Wait-Pipe([string]$wire, [int]$timeoutMs) {
  $suffix = "\$wire"
  $deadline = (Get-Date).AddMilliseconds($timeoutMs)
  while ((Get-Date) -lt $deadline) {
    try {
      foreach ($p in [System.IO.Directory]::GetFiles('\\.\pipe\')) {
        if ($p.EndsWith($suffix, [System.StringComparison]::OrdinalIgnoreCase)) { return $true }
      }
    } catch { }
    Start-Sleep -Milliseconds 100
  }
  return $false
}

function Wait-PortListening([int]$port, [bool]$listening, [int]$timeoutMs) {
  $deadline = (Get-Date).AddMilliseconds($timeoutMs)
  while ((Get-Date) -lt $deadline) {
    $conn = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
    if ([bool]$conn -eq $listening) { return $true }
    Start-Sleep -Milliseconds 150
  }
  return $false
}

function Wait-ProcessGone([int]$procId, [int]$timeoutMs) {
  $deadline = (Get-Date).AddMilliseconds($timeoutMs)
  while ((Get-Date) -lt $deadline) {
    if (-not (Get-Process -Id $procId -ErrorAction SilentlyContinue)) { return $true }
    Start-Sleep -Milliseconds 150
  }
  return $false
}

function Assert-PortFree([int]$port) {
  $conn = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
  if ($conn) { throw "port $port already in use" }
}

function Start-NetHost([string]$case, [string]$wire) {
  $runRoot = Join-Path $env:TEMP "v2rayn-t18-$case-$Stamp"
  New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
  $env:V2RAYN_R_PIPE = "\\.\pipe\$wire"
  $env:V2RAYN_R_RUN_ROOT = $runRoot
  $env:V2RAYN_R_XRAY_BIN = $Xray
  $env:V2RAYN_R_HEARTBEAT_MS = '1000'
  $env:V2RAYN_R_DISCONNECT_GRACE_MS = '6000'
  $env:V2RAYN_R_READY_TIMEOUT_MS = '15000'
  $env:V2RAYN_R_READY_INTERVAL_MS = '250'
  $out = Join-Path $env:TEMP "t18-nh-$case-$Stamp.out"
  $err = Join-Path $env:TEMP "t18-nh-$case-$Stamp.err"
  $p = Start-Process -FilePath $NetHost -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
  [void]$Script:OwnedPids.Add($p.Id)
  $ready = Wait-Pipe $wire 10000
  return [pscustomobject]@{ pid = $p.Id; proc = $p; runRoot = $runRoot; ready = $ready }
}

function Run-Client([string]$wire, [string[]]$argList, [int]$timeoutMs, [string]$tag) {
  $env:V2RAYN_R_PIPE = "\\.\pipe\$wire"
  $out = Join-Path $env:TEMP "t18-cli-$tag-$Stamp.out"
  $err = Join-Path $env:TEMP "t18-cli-$tag-$Stamp.err"
  $p = Start-Process -FilePath $Client -ArgumentList $argList -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
  [void]$Script:OwnedPids.Add($p.Id)
  $completed = $p.WaitForExit($timeoutMs)
  if (-not $completed) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
  return [pscustomobject]@{
    completed = $completed
    exit      = $p.ExitCode
    out       = if (Test-Path $out) { Get-Content $out -Raw } else { '' }
  }
}

function Get-Detail([string]$text) {
  $m = [regex]::Match($text, '(?m)^DETAIL (.+)$')
  if (-not $m.Success) { return $null }
  try { return $m.Groups[1].Value | ConvertFrom-Json } catch { return $null }
}

# ---------------------------------------------------------------------------
# Build debug binaries if needed
# ---------------------------------------------------------------------------
if (-not (Test-Path $NetHost) -or -not (Test-Path $Client)) {
  $cargo = Join-Path $HOME '.cargo\bin\cargo.exe'
  & $cargo build -p net_host --bin net_host --bin t03_client --locked 2>&1 | Out-Null
}
$SmokeExample = Join-Path $RepoRoot 'target\debug\examples\t03_smoke_config.exe'
if (-not (Test-Path $SmokeExample)) {
  $cargo = Join-Path $HOME '.cargo\bin\cargo.exe'
  & $cargo build -p bridge_api --example t03_smoke_config --locked 2>&1 | Out-Null
}
if (-not (Test-Path $NetHost)) { throw "net_host not built" }
if (-not (Test-Path $Client)) { throw "t03_client not built" }

# ---------------------------------------------------------------------------
# 20x apply/stop loop
# ---------------------------------------------------------------------------
$loop = New-Object System.Collections.Generic.List[object]
$startedAt = Get-Date
for ($i = 1; $i -le $Iterations; $i++) {
  $wire = "v2rayn-r-t18-loop$i-$Stamp"
  $entry = [ordered]@{ iteration = $i; apply = $false; stop = $false; residual_xray = 0; port_free_after = $false; elapsed_ms = 0 }
  $nh = $null
  $xrayPid = 0
  try {
    Assert-PortFree $Port
    $nh = Start-NetHost "loop$i" $wire
    if (-not $nh.ready) { throw "net_host not ready" }
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $apply = Run-Client $wire @('apply', "$Port") 60000 "loop$i-apply"
    $detail = Get-Detail $apply.out
    if (-not $apply.completed -or $apply.exit -ne 0 -or -not $detail -or $detail.state -ne 'Running') {
      throw "apply failed exit=$($apply.exit) state=$($detail.state)"
    }
    $entry.apply = $true
    $xrayPid = [int]$detail.pid
    if (-not (Wait-PortListening $Port $true 5000)) { throw "port not listening" }
    $owner = (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1).OwningProcess
    if ($owner -ne $xrayPid) { throw "port owner $owner != xray $xrayPid" }
    $stop = Run-Client $wire @('stop') 30000 "loop$i-stop"
    if (-not $stop.completed -or $stop.exit -ne 0) { throw "stop failed exit=$($stop.exit)" }
    $entry.stop = $true
    if (-not (Wait-ProcessGone $xrayPid 10000)) { throw "xray $xrayPid survived" }
    if (-not (Wait-PortListening $Port $false 5000)) { throw "port still listening" }
    $entry.port_free_after = $true
    $sw.Stop()
    $entry.elapsed_ms = [math]::Round($sw.Elapsed.TotalMilliseconds, 1)
  } catch {
    $entry.error = "$_"
  } finally {
    if ($xrayPid -gt 0 -and (Get-Process -Id $xrayPid -ErrorAction SilentlyContinue)) {
      $entry.residual_xray = 1
      Stop-Owned $xrayPid
    }
    if ($nh) {
      Stop-Owned $nh.pid
      $entry.net_host_gone = -not [bool](Get-Process -Id $nh.pid -ErrorAction SilentlyContinue)
    }
  }
  $loop.Add([pscustomobject]$entry)
  Write-Output ("T18 loop {0}/{1} apply={2} stop={3} elapsed={4}ms {5}" -f $i, $Iterations, $entry.apply, $entry.stop, $entry.elapsed_ms, $(if ($entry.error) { "ERR: $($entry.error)" } else { '' }))
}
$loopOk = @($loop | Where-Object { $_.apply -and $_.stop -and $_.port_free_after -and $_.residual_xray -eq 0 -and $_.net_host_gone -ne $false }).Count

# leak trend: only PIDs this script started can be counted (the host may run
# its own unrelated net_host/xray from other tooling; those must never be killed
# or reported as our leak).
Start-Sleep -Seconds 2
$ownedAlive = @($Script:OwnedPids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
$residualOwned = $ownedAlive.Count
$portBusy = [bool](Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue)

# ---------------------------------------------------------------------------
# Runtime session CPU/RSS
# ---------------------------------------------------------------------------
$runtime = [ordered]@{ status = 'failed' }
$wire = "v2rayn-r-t18-runtime-$Stamp"
$nh = $null
$hold = $null
try {
  Assert-PortFree $Port
  $nh = Start-NetHost 'runtime' $wire
  if (-not $nh.ready) { throw "net_host not ready" }
  $env:V2RAYN_R_PIPE = "\\.\pipe\$wire"
  $holdOut = Join-Path $env:TEMP "t18-hold-$Stamp.out"
  $holdErr = Join-Path $env:TEMP "t18-hold-$Stamp.err"
  $hold = Start-Process -FilePath $Client -ArgumentList @('apply-hold', "$Port", "$RuntimeHoldSeconds") -NoNewWindow -PassThru -RedirectStandardOutput $holdOut -RedirectStandardError $holdErr
  [void]$Script:OwnedPids.Add($hold.Id)
  $deadline = (Get-Date).AddSeconds(45)
  $detail = $null
  while ((Get-Date) -lt $deadline) {
    $text = if (Test-Path $holdOut) { Get-Content $holdOut -Raw } else { '' }
    $detail = Get-Detail $text
    if ($detail -and $detail.state -eq 'Running') { break }
    if ($hold.HasExited) { break }
    Start-Sleep -Milliseconds 250
  }
  if (-not $detail -or $detail.state -ne 'Running') { throw "hold did not reach Running" }
  $xrayPid = [int]$detail.pid
  $netHostProc = Get-Process -Id $nh.pid -ErrorAction SilentlyContinue
  $xrayProc = Get-Process -Id $xrayPid -ErrorAction SilentlyContinue
  $cpu0 = ($netHostProc.TotalProcessorTime + $xrayProc.TotalProcessorTime).TotalSeconds
  $sampleDeadline = (Get-Date).AddSeconds(15)
  $nhRss = New-Object System.Collections.Generic.List[double]
  $xrayRss = New-Object System.Collections.Generic.List[double]
  while ((Get-Date) -lt $sampleDeadline -and -not $hold.HasExited) {
    $nhRss.Add([double](Get-Process -Id $nh.pid -ErrorAction SilentlyContinue).WorkingSet64)
    $xrayRss.Add([double](Get-Process -Id $xrayPid -ErrorAction SilentlyContinue).WorkingSet64)
    Start-Sleep -Milliseconds 500
  }
  Start-Sleep -Seconds 1
  $cpu1 = ((Get-Process -Id $nh.pid -ErrorAction SilentlyContinue).TotalProcessorTime + (Get-Process -Id $xrayPid -ErrorAction SilentlyContinue).TotalProcessorTime).TotalSeconds
  $runtime.status = 'ok'
  $runtime.net_host_pid = $nh.pid
  $runtime.xray_pid = $xrayPid
  $runtime.sample_seconds = 15
  $runtime.net_host_rss_mb = [math]::Round((($nhRss | Measure-Object -Average).Average) / 1MB, 2)
  $runtime.xray_rss_mb = [math]::Round((($xrayRss | Measure-Object -Average).Average) / 1MB, 2)
  $runtime.combined_cpu_seconds = [math]::Round($cpu1 - $cpu0, 3)
  # Stop via the real stop path, then confirm the core is gone.
  $stop = Run-Client $wire @('stop') 30000 "runtime-stop"
  $gone = Wait-ProcessGone $xrayPid 10000
  $runtime.core_gone_after_stop = $gone
  $runtime.port_free_after = Wait-PortListening $Port $false 5000
} catch {
  $runtime.error = "$_"
} finally {
  if ($hold) { Stop-Owned $hold.Id }
  if ($nh) { Stop-Owned $nh.pid }
}
$runtime | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutDir 'runtime_session.json') -Encoding UTF8
Write-Output ("T18 runtime status={0} nh_rss={1}MB xray_rss={2}MB cpu_s={3} gone_after_stop={4}" -f $runtime.status, $runtime.net_host_rss_mb, $runtime.xray_rss_mb, $runtime.combined_cpu_seconds, $runtime.core_gone_after_stop)

# ---------------------------------------------------------------------------
# App reopen drift (Rust) + log throughput lines
# ---------------------------------------------------------------------------
$reopen = $null
$cargo = Join-Path $HOME '.cargo\bin\cargo.exe'
Push-Location $RepoRoot
try {
  $testOut = & $cargo test -p application --test t18_stability --locked -- --nocapture 2>&1 | Out-String
} finally {
  Pop-Location
}
$m = [regex]::Match($testOut, 'T18_REOPEN (\{.*\})')
if ($m.Success) { $reopen = $m.Groups[1].Value | ConvertFrom-Json }
$ioErrors = [regex]::Matches($testOut, 'T18_IO_ERROR (\{.*\})') | ForEach-Object { $_.Groups[1].Value | ConvertFrom-Json }
Write-Output ("T18 reopen iterations={0} drift={1}" -f $reopen.iterations, $reopen.drift)

# ---------------------------------------------------------------------------
# T03 fault matrix rerun (client kill / net-host kill / port conflict / core quick-exit)
# ---------------------------------------------------------------------------
$faults = $null
if (-not $SkipFaults) {
  $faultScript = Join-Path $RepoRoot 'services\net_host\tests\t03_faults.ps1'
  if (Test-Path $faultScript) {
    Push-Location $RepoRoot
    try {
      $faultOut = & $faultScript 2>&1 | Out-String
    } finally {
      Pop-Location
    }
    $faultCases = @()
    foreach ($line in ($faultOut -split "`n")) {
      $fm = [regex]::Match($line.Trim(), '^RESULT (\S+) (\S+)$')
      if ($fm.Success) { $faultCases += [pscustomobject]@{ case = $fm.Groups[1].Value; status = $fm.Groups[2].Value } }
    }
    $faults = [pscustomobject]@{ cases = $faultCases; raw_tail = ($faultOut -split "`n" | Select-Object -Last 15) }
  }
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
$summary = [ordered]@{
  generated_at        = (Get-Date).ToString('o')
  port                = $Port
  session_loop        = [ordered]@{
    iterations        = $Iterations
    ok                = $loopOk
    residual_owned    = $residualOwned
    port_busy         = $portBusy
    entries           = $loop
  }
  runtime_session     = $runtime
  reopen_drift        = $reopen
  io_error_paths      = $ioErrors
  fault_matrix        = $faults
  limitations         = @(
    'Session loop uses the T03 smoke plan (socks inbound on 127.0.0.1:11808), not a real user config.',
    'Runtime CPU/RSS sampled for ~15s inside a 30s hold, not a 24h soak.',
    'Reopen drift is measured by the Rust engine (10x open over one data dir), not by launching the GUI 10 times.',
    'Fault matrix is the existing T03 script rerun; it covers client kill, net-host kill, port conflict and core quick-exit.',
    'Disk read-only is approximated by an IO write-failure path in the Rust test, not a real read-only volume.'
  )
}
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDir 'stability.json') -Encoding UTF8
Write-Output "T18 stability summary -> $(Join-Path $OutDir 'stability.json')"
