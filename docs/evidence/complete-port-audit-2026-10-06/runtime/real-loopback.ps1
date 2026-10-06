$ErrorActionPreference = 'Stop'
$auditRepository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..\..'))
$auditHostBinary = Join-Path $auditRepository 'target\runtime-complete-port-audit\debug\runtime_audit_host.exe'
$auditClientBinary = Join-Path $auditRepository 'target\runtime-complete-port-audit\debug\runtime_audit_client.exe'
$auditXray = Join-Path $auditRepository 'tools\cores\xray\v26.3.27\xray.exe'
$auditRunRoot = Join-Path $PSScriptRoot ("run-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $auditRunRoot | Out-Null

function Find-AuditPort([int] $from) {
    for ($candidate = $from; $candidate -lt 65000; $candidate++) {
        $probe = $null
        try {
            $probe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $candidate)
            $probe.Start()
            $probe.Stop()
            return $candidate
        } catch { if ($probe) { $probe.Stop() } }
    }
    throw 'No available audit port'
}

function Invoke-AuditClient([string] $operation, [int] $port) {
    $lines = @(& $auditClientBinary $operation $port)
    $code = $LASTEXITCODE
    $lines | Write-Output
    if ($code -ne 0 -and -not ($operation -eq 'apply-bad' -and $code -eq 3)) { throw "audit client $operation failed: $code" }
    return
}

function Read-AuditDetail([string[]] $lines) {
    $detailLine = @($lines | Where-Object { $_.StartsWith('DETAIL ') })[-1]
    return $detailLine.Substring(7) | ConvertFrom-Json
}

function Test-AuditSocks([int] $port) {
    $socket = [Net.Sockets.TcpClient]::new()
    try {
        $socket.Connect('127.0.0.1', $port)
        $stream = $socket.GetStream()
        $stream.ReadTimeout = 1000
        $stream.Write([byte[]](5,1,0), 0, 3)
        $reply = [byte[]]::new(2)
        $count = $stream.Read($reply,0,2)
        return $count -eq 2 -and $reply[0] -eq 5 -and $reply[1] -eq 0
    } catch { return $false } finally { $socket.Dispose() }
}

$auditPortA = Find-AuditPort 11977
$auditPortB = Find-AuditPort ($auditPortA + 1)
$env:V2RAYN_R_PIPE = '\\.\pipe\v2rayn-complete-port-audit-' + [guid]::NewGuid().ToString('N')
$env:V2RAYN_R_RUN_ROOT = $auditRunRoot
$env:V2RAYN_R_XRAY_BIN = $auditXray
$env:V2RAYN_R_DEV_MODE = '1'
$env:V2RAYN_R_DISCONNECT_GRACE_MS = '60000'
$env:V2RAYN_R_READY_INTERVAL_MS = '100'
$env:V2RAYN_R_READY_TIMEOUT_MS = '5000'
Remove-Item Env:\V2RAYN_R_SKIP_CONFIG_CHECK -ErrorAction SilentlyContinue

$auditHost = Start-Process -FilePath $auditHostBinary -PassThru -WindowStyle Hidden -RedirectStandardOutput (Join-Path $auditRunRoot 'host.stdout.log') -RedirectStandardError (Join-Path $auditRunRoot 'host.stderr.log')
$auditOwnedCore = $null
try {
    Start-Sleep -Milliseconds 800
    Write-Output "baseline=a7aa0a5 host_pid=$($auditHost.Id) ports=$auditPortA,$auditPortB run_root=$auditRunRoot"
    $initial = @(Invoke-AuditClient 'apply' $auditPortA)
    $initial | Write-Output
    $detailA = Read-AuditDetail $initial
    if ($detailA.state -ne 'running' -or -not (Test-AuditSocks $auditPortA)) { throw 'initial Xray SOCKS session failed' }
    $bad = @(Invoke-AuditClient 'apply-bad' $auditPortA)
    $bad | Write-Output
    $detailBad = Read-AuditDetail $bad
    if ($detailBad.pid -ne $detailA.pid -or -not (Test-AuditSocks $auditPortA)) { throw 'precheck failure killed prior healthy session' }
    $switched = @(Invoke-AuditClient 'apply' $auditPortB)
    $switched | Write-Output
    $detailB = Read-AuditDetail $switched
    if ($detailB.state -ne 'running' -or -not (Test-AuditSocks $auditPortB) -or (Test-AuditSocks $auditPortA)) { throw 'real port switch failed' }

    # Hold an identity-checked handle to exactly the core this audit's unique
    # host and synthetic plan started; never terminate by executable name.
    $auditOwnedCore = [Diagnostics.Process]::GetProcessById([int]$detailB.pid)
    $created = [DateTimeOffset]::new($auditOwnedCore.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds()
    if ($created -ne [long]$detailB.created_at_ms -or $auditOwnedCore.MainModule.FileName -ne $auditXray) { throw 'managed core identity did not match audit record' }
    $auditOwnedCore.Kill()
    $auditOwnedCore.WaitForExit(3000) | Out-Null
    Write-Output "fault_injected=own_managed_core_exit pid=$($detailB.pid) created_at_ms=$created session=$($detailB.session_id)"
    Start-Sleep -Milliseconds 3000
    $after = @(Invoke-AuditClient 'snapshot' $auditPortB)
    $after | Write-Output
    $detailAfter = Read-AuditDetail $after
    $livePort = Test-AuditSocks $auditPortB
    Write-Output "OBSERVED_AFTER_CORE_EXIT snapshot_state=$($detailAfter.state) snapshot_pid=$($detailAfter.pid) socks_port_alive=$livePort"
    if ($detailAfter.state -eq 'running' -and -not $livePort) { Write-Output 'REPRODUCED: post-readiness core exit leaves a fabricated Running cache and stale applied endpoint' }
    Invoke-AuditClient 'stop' $auditPortB
    Invoke-AuditClient 'shutdown' $auditPortB
    Write-Output 'AUDIT_LOOPBACK_DONE'
} finally {
    if ($auditOwnedCore) { $auditOwnedCore.Dispose() }
    if ($auditHost -and -not $auditHost.HasExited) {
        $auditHost.Kill()
        $auditHost.WaitForExit(3000) | Out-Null
    }
    if ($auditHost) { $auditHost.Dispose() }
    Write-Output 'CLEANUP: audit host stopped; managed core was stopped through its recorded identity; synthetic run artifacts retained'
}
