# RR-10 real-xray evidence: a bad switch is rejected during precheck, so the
# running session keeps serving; then a good switch really swaps the core.
#
# Constraints honoured: no 10808, all ports >= 11808 and probed first; only
# processes this script started are stopped; synthetic configs only; no system
# proxy / registry / route / TUN changes.
$ErrorActionPreference = 'Stop'

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')
$NetHost  = Join-Path $RepoRoot 'target\debug\net_host.exe'
$Client   = Join-Path $RepoRoot 'target\debug\t03_client.exe'
$Xray     = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe'

if (-not (Test-Path $NetHost)) { throw "missing $NetHost (cargo build -p net_host)" }
if (-not (Test-Path $Client))  { throw "missing $Client" }
if (-not (Test-Path $Xray))    { throw "missing pinned xray $Xray" }

# --- probe ports >= 11808 (never 10808) ---
function Get-FreePort([int]$Base) {
    for ($p = $Base; $p -lt 65000; $p++) {
        if ($p -eq 10808) { continue }
        $l = $null
        try {
            $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $p)
            $l.Start()
            $l.Stop()
            return $p
        } catch { if ($l) { $l.Stop() } }
    }
    throw 'no free port'
}

$OldPort = Get-FreePort 11808
$NewPort = Get-FreePort ($OldPort + 1)

$Wire = "v2rayn-rr10-$([System.Diagnostics.Process]::GetCurrentProcess().Id)-$([guid]::NewGuid().ToString('N').Substring(0,8))"
$RunRoot = Join-Path $env:TEMP "v2rayn-rr10-$([guid]::NewGuid().ToString('N').Substring(0,8))"
New-Item -ItemType Directory -Force -Path $RunRoot | Out-Null

$env:V2RAYN_R_PIPE = "\\.\pipe\$Wire"
$env:V2RAYN_R_RUN_ROOT = $RunRoot
$env:V2RAYN_R_XRAY_BIN = $Xray
$env:V2RAYN_R_READY_TIMEOUT_MS = '20000'
$env:V2RAYN_R_READY_INTERVAL_MS = '250'

$hostLog = Join-Path $RunRoot 'net_host.log'
$hostProc = Start-Process -FilePath $NetHost -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput $hostLog -RedirectStandardError "$hostLog.err"
Write-Output "net_host pid=$($hostProc.Id) pipe=$Wire old=$OldPort new=$NewPort"

try {
    Start-Sleep -Milliseconds 800
    Write-Output '=== 1. apply good config (real xray socks) ==='
    & $Client apply $OldPort
    $apply = $LASTEXITCODE
    Write-Output "apply exit=$apply"
    if ($apply -ne 0) { throw 'initial apply failed' }

    Write-Output '=== 2. apply-bad: precheck must reject, old session survives ==='
    & $Client apply-bad $OldPort
    $bad = $LASTEXITCODE
    Write-Output "apply-bad exit=$bad"
    if ($bad -ne 3) { throw "apply-bad expected exit 3, got $bad" }

    Write-Output '=== 3. snapshot: still Running on old port ==='
    $snap = & $Client snapshot
    $snap | Write-Output

    Write-Output '=== 4. apply good config on a NEW port (real switch) ==='
    & $Client apply $NewPort
    $switch = $LASTEXITCODE
    Write-Output "switch exit=$switch"
    if ($switch -ne 0) { throw 'switch failed' }

    Write-Output '=== 5. stop ==='
    & $Client stop
    Write-Output 'RR10_REAL_XRAY_OK'
}
finally {
    if ($hostProc -and -not $hostProc.HasExited) {
        Stop-Process -Id $hostProc.Id -Force
        Write-Output "stopped net_host pid=$($hostProc.Id)"
    }
    Remove-Item -Recurse -Force $RunRoot -ErrorAction SilentlyContinue
}
