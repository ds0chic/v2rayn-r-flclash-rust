# T03 runtime fault tests.
#
# Runs the five required cases against the real net_host + Xray v26.3.27 on a
# dedicated pipe/run-root/port (>= 11808; never 10808). Every external process
# is started with a recorded PID and every wait is bounded; a timeout is a
# failure, never an infinite hang. All started processes are cleaned up by PID.
#
# Usage:  pwsh -NoProfile -File services/net_host/tests/t03_faults.ps1
# Output: docs/evidence/T03.runs/run-<stamp>.json and per-case .md files.

param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path,
  [int]$Port = 11808
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$NetHost = Join-Path $RepoRoot 'target\debug\net_host.exe'
$Client = Join-Path $RepoRoot 'target\debug\t03_client.exe'
$Xray = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe'
$EvidenceDir = Join-Path $RepoRoot 'docs\evidence\T03.runs'
New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$Stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$Script:OwnedPids = New-Object System.Collections.Generic.List[int]
$Results = New-Object System.Collections.Generic.List[object]

function Now { (Get-Date).ToString('HH:mm:ss.fff') }
function Mark($tl, $msg) { [void]$tl.Add("$(Now) $msg") }

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
    $isListening = [bool]$conn
    if ($isListening -eq $listening) { return $true }
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

function Start-NetHost([string]$case, [string]$wire, [int]$graceMs, [int]$readyMs) {
  $runRoot = Join-Path $env:TEMP "v2rayn-t03-$case-$Stamp"
  New-Item -ItemType Directory -Force -Path $runRoot | Out-Null
  $env:V2RAYN_R_PIPE = "\\.\pipe\$wire"
  $env:V2RAYN_R_RUN_ROOT = $runRoot
  $env:V2RAYN_R_XRAY_BIN = $Xray
  $env:V2RAYN_R_HEARTBEAT_MS = '1000'
  $env:V2RAYN_R_DISCONNECT_GRACE_MS = "$graceMs"
  $env:V2RAYN_R_READY_TIMEOUT_MS = '15000'
  $env:V2RAYN_R_READY_INTERVAL_MS = '250'
  $out = Join-Path $env:TEMP "t03-nh-$case-$Stamp.out"
  $err = Join-Path $env:TEMP "t03-nh-$case-$Stamp.err"
  $p = Start-Process -FilePath $NetHost -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
  [void]$Script:OwnedPids.Add($p.Id)
  $ready = Wait-Pipe $wire $readyMs
  return [pscustomobject]@{ case = $case; pid = $p.Id; proc = $p; runRoot = $runRoot; out = $out; err = $err; ready = $ready; wire = $wire }
}

function Run-Client([string]$wire, [string[]]$argList, [int]$timeoutMs, [string]$tag) {
  $env:V2RAYN_R_PIPE = "\\.\pipe\$wire"
  $out = Join-Path $env:TEMP "t03-cli-$tag-$Stamp.out"
  $err = Join-Path $env:TEMP "t03-cli-$tag-$Stamp.err"
  $p = Start-Process -FilePath $Client -ArgumentList $argList -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
  [void]$Script:OwnedPids.Add($p.Id)
  $completed = $p.WaitForExit($timeoutMs)
  if (-not $completed) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
  return [pscustomobject]@{
    completed = $completed
    exit      = $p.ExitCode
    out       = if (Test-Path $out) { Get-Content $out -Raw } else { '' }
    err       = if (Test-Path $err) { Get-Content $err -Raw } else { '' }
  }
}

function Start-ClientHold([string]$wire, [string[]]$argList, [string]$tag, [int]$readyMs) {
  $env:V2RAYN_R_PIPE = "\\.\pipe\$wire"
  $out = Join-Path $env:TEMP "t03-hold-$tag-$Stamp.out"
  $err = Join-Path $env:TEMP "t03-hold-$tag-$Stamp.err"
  $p = Start-Process -FilePath $Client -ArgumentList $argList -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
  [void]$Script:OwnedPids.Add($p.Id)
  $deadline = (Get-Date).AddMilliseconds($readyMs)
  while ((Get-Date) -lt $deadline) {
    $text = if (Test-Path $out) { Get-Content $out -Raw } else { '' }
    if ($text -match 'HOLDING') { break }
    if ($p.HasExited) { break }
    Start-Sleep -Milliseconds 150
  }
  return [pscustomobject]@{ pid = $p.Id; proc = $p; out = $out; err = $err }
}

function Get-Detail([string]$text) {
  $m = [regex]::Match($text, '(?m)^DETAIL (.+)$')
  if (-not $m.Success) { return $null }
  try { return $m.Groups[1].Value | ConvertFrom-Json } catch { return $null }
}

function Get-Result([string]$text) {
  $m = [regex]::Match($text, '(?m)^RESULT (.+)$')
  if (-not $m.Success) { return $null }
  try { return $m.Groups[1].Value | ConvertFrom-Json } catch { return $null }
}

function Stop-Owned([int]$procId) {
  if ($procId -le 0) { return }
  $p = Get-Process -Id $procId -ErrorAction SilentlyContinue
  if ($p) { Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue }
}

function Save-Case([string]$name, [string]$status, $tl, [hashtable]$extra) {
  $obj = [ordered]@{
    case     = $name
    status   = $status
    timeline = $tl
    extra    = $extra
  }
  $json = $obj | ConvertTo-Json -Depth 8
  Set-Content -Path (Join-Path $EvidenceDir "$name.json") -Value $json -Encoding UTF8
  $md = New-Object System.Collections.Generic.List[string]
  [void]$md.Add("# T03 case $name - $status")
  [void]$md.Add('')
  [void]$md.Add('## Timeline')
  foreach ($line in $tl) { [void]$md.Add("- $line") }
  [void]$md.Add('')
  [void]$md.Add('## Detail')
  [void]$md.Add('```json')
  [void]$md.Add(($extra | ConvertTo-Json -Depth 6))
  [void]$md.Add('```')
  Set-Content -Path (Join-Path $EvidenceDir "$name.md") -Value ($md -join "`n") -Encoding UTF8
  [void]$Results.Add([pscustomobject]@{ case = $name; status = $status; json = (Join-Path $EvidenceDir "$name.json") })
}

function Assert-PortFree([int]$port) {
  $conn = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
  if ($conn) { throw "port $port already in use before test" }
}

# ---------------------------------------------------------------------------
# Case A: normal closed loop
# ---------------------------------------------------------------------------
function Invoke-CaseA {
  $tl = New-Object System.Collections.Generic.List[string]
  $extra = @{}
  $status = 'failed'
  $case = 'a-normal'
  $wire = "v2rayn-r-t03-a-$Stamp"
  try {
    Mark $tl "assert port $Port free"
    Assert-PortFree $Port
    $nh = Start-NetHost $case $wire 6000 10000
    Mark $tl "net_host pid=$($nh.pid) started ready=$($nh.ready)"
    if (-not $nh.ready) { throw "net_host pipe not ready" }

    $apply = Run-Client $wire @('apply', "$Port") 60000 "$case-apply"
    Mark $tl "t03_client apply exit=$($apply.exit) completed=$($apply.completed)"
    $detail = Get-Detail $apply.out
    $result = Get-Result $apply.out
    $extra.apply_exit = $apply.exit
    $extra.result = $result
    $extra.detail = $detail
    if (-not $apply.completed) { throw "apply timed out" }
    if ($apply.exit -ne 0) { throw "apply failed exit=$($apply.exit)" }
    if (-not $detail -or $detail.state -ne 'Running') { throw "detail state not Running" }
    $xrayPid = [int]$detail.pid
    $extra.xray_pid = $xrayPid
    $extra.xray_created_at_ms = $detail.created_at_ms

    if (-not (Wait-PortListening $Port $true 5000)) { throw "port $Port not listening" }
    $owner = (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1).OwningProcess
    $extra.port_owner_pid = $owner
    Mark $tl "port $Port listening owner=$owner xray_pid=$xrayPid"
    if ($owner -ne $xrayPid) { throw "port owner $owner != xray pid $xrayPid" }

    $stop = Run-Client $wire @('stop') 30000 "$case-stop"
    Mark $tl "t03_client stop exit=$($stop.exit)"
    $extra.stop_exit = $stop.exit
    if (-not (Wait-ProcessGone $xrayPid 10000)) { throw "xray pid $xrayPid survived stop" }
    if (-not (Wait-PortListening $Port $false 5000)) { throw "port $Port still listening after stop" }

    $snap = Run-Client $wire @('snapshot') 15000 "$case-snap"
    $snapResult = Get-Result $snap.out
    $extra.snapshot_result = $snapResult
    Mark $tl "snapshot after stop: $($snapResult.kind)/$($snapResult.state)"
    if ($snapResult.state -ne 'Stopped') { throw "snapshot state $($snapResult.state) != Stopped" }

    $status = 'passed'
    Mark $tl 'PASS'
  } catch {
    $extra.error = "$_"
    Mark $tl "FAIL: $_"
  } finally {
    Stop-Owned $nh.pid
    Mark $tl "cleanup net_host pid=$($nh.pid)"
    Save-Case $case $status $tl $extra
  }
}

# ---------------------------------------------------------------------------
# Case B: port conflict (pre-occupied 11808)
# ---------------------------------------------------------------------------
function Invoke-CaseB {
  $tl = New-Object System.Collections.Generic.List[string]
  $extra = @{}
  $status = 'failed'
  $case = 'b-port-conflict'
  $wire = "v2rayn-r-t03-b-$Stamp"
  $listener = $null
  try {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
    $listener.Start()
    Mark $tl "occupied 127.0.0.1:$Port with a local TcpListener"
    $nh = Start-NetHost $case $wire 6000 10000
    Mark $tl "net_host pid=$($nh.pid) ready=$($nh.ready)"
    if (-not $nh.ready) { throw "net_host pipe not ready" }

    $apply = Run-Client $wire @('apply', "$Port") 30000 "$case-apply"
    $result = Get-Result $apply.out
    $extra.apply_exit = $apply.exit
    $extra.result = $result
    Mark $tl "apply exit=$($apply.exit) result kind=$($result.kind) code=$($result.code)"
    if ($apply.exit -ne 3) { throw "expected exit 3, got $($apply.exit)" }
    if ($result.kind -ne 'error') { throw "expected structured error result" }
    if ($result.code -ne 'E_PORT_CONFLICT') { throw "expected E_PORT_CONFLICT, got $($result.code)" }

    $snap = Run-Client $wire @('snapshot') 15000 "$case-snap"
    $snapResult = Get-Result $snap.out
    $extra.snapshot_state = $snapResult.state
    Mark $tl "snapshot state=$($snapResult.state)"
    if ($snapResult.state -notin @('Stopped', 'Degraded')) { throw "unexpected state $($snapResult.state)" }
    $status = 'passed'
    Mark $tl 'PASS'
  } catch {
    $extra.error = "$_"
    Mark $tl "FAIL: $_"
  } finally {
    if ($listener) { $listener.Stop() }
    Stop-Owned $nh.pid
    Mark $tl 'cleanup listener + net_host'
    Save-Case $case $status $tl $extra
  }
}

# ---------------------------------------------------------------------------
# Case C: client hard-kill -> lease reclaim stops the core
# ---------------------------------------------------------------------------
function Invoke-CaseC {
  $tl = New-Object System.Collections.Generic.List[string]
  $extra = @{}
  $status = 'failed'
  $case = 'c-client-kill'
  $wire = "v2rayn-r-t03-c-$Stamp"
  try {
    $nh = Start-NetHost $case $wire 3000 10000
    Mark $tl "net_host pid=$($nh.pid) ready=$($nh.ready) grace=3000ms"
    if (-not $nh.ready) { throw "net_host pipe not ready" }

    $apply = Run-Client $wire @('apply-exit', "$Port") 60000 "$case-apply"
    Mark $tl "apply-exit exit=$($apply.exit) completed=$($apply.completed)"
    $detail = Get-Detail $apply.out
    $extra.detail = $detail
    if (-not $detail -or $detail.state -ne 'Running') { throw "client did not reach Running" }
    $xrayPid = [int]$detail.pid
    $extra.xray_pid = $xrayPid

    if (-not (Wait-ProcessGone $xrayPid 20000)) { throw "xray pid $xrayPid survived client kill" }
    Mark $tl "xray pid=$xrayPid gone after client disconnect (lease reclaim)"
    $extra.xray_exit_seen = $true

    $snap = Run-Client $wire @('snapshot') 15000 "$case-snap"
    $snapResult = Get-Result $snap.out
    $extra.snapshot_state = $snapResult.state
    Mark $tl "snapshot state=$($snapResult.state)"
    if ($snapResult.state -ne 'Stopped') { throw "state $($snapResult.state) != Stopped" }
    $status = 'passed'
    Mark $tl 'PASS'
  } catch {
    $extra.error = "$_"
    Mark $tl "FAIL: $_"
  } finally {
    Stop-Owned $nh.pid
    Mark $tl "cleanup net_host pid=$($nh.pid)"
    Save-Case $case $status $tl $extra
  }
}

# ---------------------------------------------------------------------------
# Case D: net-host hard-kill -> job kills core; restart recovers journal
# ---------------------------------------------------------------------------
function Invoke-CaseD {
  $tl = New-Object System.Collections.Generic.List[string]
  $extra = @{}
  $status = 'failed'
  $case = 'd-net-host-kill'
  $wire = "v2rayn-r-t03-d-$Stamp"
  $nh = $null
  $nh2 = $null
  $hold = $null
  try {
    $nh = Start-NetHost $case $wire 6000 10000
    Mark $tl "net_host pid=$($nh.pid) ready=$($nh.ready)"
    if (-not $nh.ready) { throw "net_host pipe not ready" }

    $hold = Start-ClientHold $wire @('apply-hold', "$Port", '60') "$case-hold" 40000
    Mark $tl "apply-hold client pid=$($hold.pid)"
    $holdText = if (Test-Path $hold.out) { Get-Content $hold.out -Raw } else { '' }
    $detail = Get-Detail $holdText
    $extra.detail = $detail
    if (-not $detail -or $detail.state -ne 'Running') { throw "hold client did not reach Running" }
    $xrayPid = [int]$detail.pid
    $sessionId = $detail.session_id
    $extra.xray_pid = $xrayPid
    $extra.session_id = $sessionId
    $journal = Join-Path (Join-Path $nh.runRoot $sessionId) 'journal.json'
    $pre = if (Test-Path $journal) { Get-Content $journal -Raw | ConvertFrom-Json } else { $null }
    $extra.journal_before = $pre
    Mark $tl "running xray pid=$xrayPid session=$sessionId journal_stage=$($pre.stage)"
    if ($pre.stage -ne 'applied') { throw "journal stage before kill is $($pre.stage), expected applied" }

    Stop-Owned $nh.pid
    Mark $tl "TerminateProcess net_host pid=$($nh.pid)"
    if (-not (Wait-ProcessGone $xrayPid 15000)) { throw "xray pid $xrayPid survived net_host kill (job ineffective)" }
    Mark $tl "xray pid=$xrayPid died with net_host (KILL_ON_JOB_CLOSE)"

    $nh2 = Start-NetHost $case $wire 6000 10000
    Mark $tl "net_host restarted pid=$($nh2.pid) ready=$($nh2.ready)"
    if (-not $nh2.ready) { throw "restarted net_host pipe not ready" }
    Start-Sleep -Milliseconds 800
    $post = if (Test-Path $journal) { Get-Content $journal -Raw | ConvertFrom-Json } else { $null }
    $extra.journal_after = $post
    Mark $tl "journal after recovery stage=$($post.stage) updated=$($post.updated_at_ms)"
    if (-not $post -or $post.stage -ne 'finalized') { throw "recovery did not finalize journal (stage=$($post.stage))" }
    $status = 'passed'
    Mark $tl 'PASS'
  } catch {
    $extra.error = "$_"
    Mark $tl "FAIL: $_"
  } finally {
    if ($hold) { Stop-Owned $hold.pid }
    if ($nh) { Stop-Owned $nh.pid }
    if ($nh2) { Stop-Owned $nh2.pid }
    Mark $tl 'cleanup hold + net_host instances'
    Save-Case $case $status $tl $extra
  }
}

# ---------------------------------------------------------------------------
# Case E: core quick-exit (bad config) -> Failed, no fake Running
# ---------------------------------------------------------------------------
function Invoke-CaseE {
  $tl = New-Object System.Collections.Generic.List[string]
  $extra = @{}
  $status = 'failed'
  $case = 'e-core-quick-exit'
  $wire = "v2rayn-r-t03-e-$Stamp"
  try {
    $nh = Start-NetHost $case $wire 6000 10000
    Mark $tl "net_host pid=$($nh.pid) ready=$($nh.ready)"
    if (-not $nh.ready) { throw "net_host pipe not ready" }

    $apply = Run-Client $wire @('apply-bad', "$Port") 40000 "$case-apply"
    $result = Get-Result $apply.out
    $extra.apply_exit = $apply.exit
    $extra.result = $result
    Mark $tl "apply-bad exit=$($apply.exit) result kind=$($result.kind) code=$($result.code) key=$($result.message_key)"
    if ($apply.exit -ne 3) { throw "expected exit 3, got $($apply.exit)" }
    if ($result.kind -ne 'error') { throw "expected structured error, got $($result.kind)" }
    if ($result.code -notin @('E_INTERNAL', 'E_PORT_CONFLICT', 'E_TIMEOUT')) { throw "unexpected code $($result.code)" }

    $snap = Run-Client $wire @('snapshot') 15000 "$case-snap"
    $snapResult = Get-Result $snap.out
    $extra.snapshot_state = $snapResult.state
    Mark $tl "snapshot state=$($snapResult.state)"
    if ($snapResult.state -eq 'Running') { throw "fake Running after core quick-exit" }
    if (-not (Wait-PortListening $Port $false 3000)) { throw "port $Port still listening" }
    $status = 'passed'
    Mark $tl 'PASS'
  } catch {
    $extra.error = "$_"
    Mark $tl "FAIL: $_"
  } finally {
    Stop-Owned $nh.pid
    Mark $tl "cleanup net_host pid=$($nh.pid)"
    Save-Case $case $status $tl $extra
  }
}

# ---------------------------------------------------------------------------
# Codegen smoke config validated by the real Xray binary
# ---------------------------------------------------------------------------
function Invoke-SmokeConfig {
  $tl = New-Object System.Collections.Generic.List[string]
  $extra = @{}
  $status = 'failed'
  $case = 'smoke-config'
  $cfg = Join-Path $env:TEMP "t03-smoke-config-$Stamp.json"
  $genOut = Join-Path $env:TEMP "t03-smoke-gen-$Stamp.json"
  $genErr = Join-Path $env:TEMP "t03-smoke-gen-$Stamp.err"
  $xtOut = Join-Path $env:TEMP "t03-smoke-xtest-$Stamp.out"
  $xtErr = Join-Path $env:TEMP "t03-smoke-xtest-$Stamp.err"
  try {
    $exe = Join-Path $RepoRoot 'target\debug\examples\t03_smoke_config.exe'
    if (-not (Test-Path $exe)) { throw "example not built: $exe" }
    $gen = Start-Process -FilePath $exe -NoNewWindow -PassThru -RedirectStandardOutput $genOut -RedirectStandardError $genErr
    if (-not $gen.WaitForExit(30000)) { Stop-Process -Id $gen.Id -Force; throw "smoke config generation timed out" }
    if ($gen.ExitCode -ne 0) { throw "smoke config generation failed exit=$($gen.ExitCode)" }
    $text = Get-Content $genOut -Raw
    $extra.config = $text | ConvertFrom-Json
    Mark $tl "generated codegen Xray smoke config ($($text.Length) bytes)"
    if ($text -match '10808') { throw "generated config contains forbidden port 10808" }
    if ($text -notmatch '"port":\s*11808') { throw "generated config missing port 11808" }

    $xrayTest = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe'
    $q = Start-Process -FilePath $xrayTest -ArgumentList 'run', '-test', '-c', $genOut -NoNewWindow -PassThru -RedirectStandardOutput $xtOut -RedirectStandardError $xtErr
    if (-not $q.WaitForExit(20000)) { Stop-Process -Id $q.Id -Force; throw "xray -test timed out" }
    $extra.xray_test_exit = $q.ExitCode
    $extra.xray_test_output = (Get-Content $xtOut -Raw)
    Mark $tl "xray run -test -c smoke.json exit=$($q.ExitCode)"
    if ($q.ExitCode -ne 0) { throw "xray rejected the generated config" }
    $status = 'passed'
    Mark $tl 'PASS'
  } catch {
    $extra.error = "$_"
    Mark $tl "FAIL: $_"
  } finally {
    Save-Case $case $status $tl $extra
  }
}

# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------
$summary = [ordered]@{
  stamp       = $Stamp
  port        = $Port
  net_host    = $NetHost
  client      = $Client
  xray        = $Xray
  xray_sha256 = '15c2d007954ac53ba69b80ec91242786b3c0b71d52649165b4ca1d5cc96ef8f1'
  started_at  = (Get-Date).ToString('o')
  cases       = @()
}

try {
  Invoke-SmokeConfig
  Invoke-CaseA
  Invoke-CaseB
  Invoke-CaseC
  Invoke-CaseD
  Invoke-CaseE
} finally {
  # Clean up every PID we recorded, by PID only.
  foreach ($procId in $Script:OwnedPids) { Stop-Owned $procId }
  $summary.cases = $Results
  $summary.finished_at = (Get-Date).ToString('o')
  $jsonPath = Join-Path $EvidenceDir "run-$Stamp.json"
  ($summary | ConvertTo-Json -Depth 8) | Set-Content -Path $jsonPath -Encoding UTF8
  Write-Output "SUMMARY $jsonPath"
  foreach ($r in $Results) { Write-Output ("RESULT {0} {1}" -f $r.case, $r.status) }
}
