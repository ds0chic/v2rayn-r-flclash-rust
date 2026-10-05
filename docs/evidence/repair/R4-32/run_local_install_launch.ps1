# R4-21 scenario 1: synthetic install (temporary stage) -> real core launch ->
# local synthetic HTTP service request succeeds, for xray and mihomo.
#
# Safety: binds only 127.0.0.1 on ports probed free starting at 11808; never
# touches 127.0.0.1:10808; no host proxy / registry / route / TUN changes; only
# the PIDs this script started are stopped. Synthetic configs only.
[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
)

$ErrorActionPreference = 'Stop'
$results = [ordered]@{
    task       = 'R4-21'
    generated  = (Get-Date).ToString('s')
    cores      = @()
    bound_port_10808 = $false
}

function Get-FreePort {
    # A high loopback range (>= 11808) avoids the 11808-11999 window other
    # concurrent repair sessions probe.
    param([int]$Start = 21808, [int]$End = 22999, [int[]]$Exclude = @())
    for ($p = $Start; $p -le $End; $p++) {
        if ($p -eq 10808 -or $Exclude -contains $p) { continue }
        try {
            $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $p)
            # Exclusive bind so a concurrently-bound loopback port is detected.
            $l.Server.ExclusiveAddressUse = $true
            $l.Start(); $l.Stop()
            return $p
        }
        catch { continue }
    }
    throw "no free port in $Start..$End"
}

function Wait-Port {
    param([int]$Port, [int]$TimeoutMs = 15000)
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
        try {
            $c = [System.Net.Sockets.TcpClient]::new()
            $c.Connect('127.0.0.1', $Port); $c.Close(); return $true
        }
        catch { Start-Sleep -Milliseconds 200 }
    }
    return $false
}

function Start-SyntheticServer {
    param([int]$Port, [string]$Body)
    $script = Join-Path $PSScriptRoot 'synthetic_http_server.ps1'
    $proc = Start-Process -FilePath (Get-Process -Id $PID).Path `
        -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $script, '-Port', $Port, '-Body', $Body) `
        -PassThru -WindowStyle Hidden
    if (-not (Wait-Port -Port $Port -TimeoutMs 8000)) {
        throw "synthetic HTTP server did not bind $Port"
    }
    Start-Sleep -Milliseconds 200
    if ($proc.HasExited) {
        throw "synthetic HTTP server for $Port exited early (port taken?)"
    }
    return $proc
}

function Invoke-ProxiedGet {
    param([int]$ProxyPort, [string]$Url, [string]$Scheme = 'http')
    # http: plain HTTP proxy (mihomo mixed-port). socks5: xray socks inbound.
    if ($Scheme -eq 'http') {
        $proxy = [System.Net.WebProxy]::new("http://127.0.0.1:$ProxyPort")
    }
    else {
        $proxy = [System.Net.WebProxy]::new("socks5://127.0.0.1:$ProxyPort")
    }
    $handler = [System.Net.Http.HttpClientHandler]::new()
    $handler.Proxy = $proxy
    $handler.UseProxy = $true
    $client = [System.Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds(15)
    try {
        $resp = $client.GetAsync($Url).GetAwaiter().GetResult()
        $body = $resp.Content.ReadAsStringAsync().GetAwaiter().GetResult()
        return @{ ok = $resp.IsSuccessStatusCode; body = $body; status = [int]$resp.StatusCode }
    }
    catch {
        return @{ ok = $false; body = ''; status = -1; error = $_.Exception.Message }
    }
    finally { $client.Dispose(); $handler.Dispose() }
}

function New-ManagedLayout {
    # Mirror runtime::CoreInstallLayout: <root>/<dir>/<version>/<exe>.
    param([string]$Root, [string]$Dir, [string]$Version, [string]$SourceExe)
    $dest = Join-Path $Root (Join-Path $Dir $Version)
    New-Item -ItemType Directory -Force -Path $dest | Out-Null
    $exe = Join-Path $dest (Split-Path $SourceExe -Leaf)
    Copy-Item -LiteralPath $SourceExe -Destination $exe -Force
    return $exe
}

$startedPids = [System.Collections.Generic.List[int]]::new()

try {
    # --- synthetic install into a temporary managed cores root ---------------
    $stageRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("r4-21-cores-" + [Guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Force -Path $stageRoot | Out-Null
    $xraySrc = Join-Path $RepoRoot 'tools\cores\xray\v26.3.27\xray.exe'
    $mihomoSrc = Join-Path $RepoRoot 'tools\cores\mihomo\v1.19.32\mihomo-windows-amd64-v1.exe'
    foreach ($s in @($xraySrc, $mihomoSrc)) { if (-not (Test-Path -LiteralPath $s)) { throw "missing core binary $s" } }

    $xrayExe = New-ManagedLayout -Root $stageRoot -Dir 'xray' -Version '26.3.27' -SourceExe $xraySrc
    $mihomoExe = New-ManagedLayout -Root $stageRoot -Dir 'mihomo' -Version '1.19.32' -SourceExe $mihomoSrc

    # --- synthetic HTTP target (shared) --------------------------------------
    $targetPort = Get-FreePort
    $server = Start-SyntheticServer -Port $targetPort -Body 'R4-21-OK'
    $startedPids.Add($server.Id)

    # --- xray: socks inbound, freedom outbound -------------------------------
    $xrayPort = Get-FreePort -Exclude @($targetPort)
    if ($xrayPort -eq 10808 -or $xrayPort -eq $targetPort) { throw 'port policy violation' }
    $xrayCfg = Join-Path $stageRoot 'xray-config.json'
    $xrayJson = @{
        log       = @{ loglevel = 'warning' }
        inbounds  = @(@{ port = $xrayPort; listen = '127.0.0.1'; protocol = 'socks'; settings = @{ udp = $false } })
        outbounds = @(@{ protocol = 'freedom'; settings = @{} })
    } | ConvertTo-Json -Depth 6
    Set-Content -LiteralPath $xrayCfg -Value $xrayJson -Encoding utf8
    $xrayProc = Start-Process -FilePath $xrayExe -ArgumentList @('run', '-c', $xrayCfg) -PassThru -WindowStyle Hidden
    $startedPids.Add($xrayProc.Id)
    $xrayListened = Wait-Port -Port $xrayPort -TimeoutMs 20000
    $xrayHttp = $null
    if ($xrayListened) {
        $xrayHttp = Invoke-ProxiedGet -ProxyPort $xrayPort -Url "http://127.0.0.1:$targetPort/r4-21" -Scheme 'socks5'
    }
    $results.cores += [ordered]@{
        core = 'xray'; version = '26.3.27'; managed_dir = (Split-Path $xrayExe -Parent)
        args = @('run', '-c', '{cfg}'); proxy_port = $xrayPort
        listened = $xrayListened; proxied_ok = ($xrayHttp -and $xrayHttp.ok); proxied_body = $(if ($xrayHttp) { $xrayHttp.body } else { $null })
        proxied_status = $(if ($xrayHttp) { $xrayHttp.status } else { $null }); proxied_error = $(if ($xrayHttp) { $xrayHttp.error } else { $null })
        pid = $xrayProc.Id
    }

    # --- mihomo: mixed-port, DIRECT ------------------------------------------
    $mihomoPort = Get-FreePort -Exclude @($targetPort, $xrayPort)
    if ($mihomoPort -eq 10808 -or $mihomoPort -eq $targetPort -or $mihomoPort -eq $xrayPort) { throw 'port policy violation' }
    $mihomoDir = Split-Path $mihomoExe -Parent
    $mihomoCfg = Join-Path $mihomoDir 'config.yaml'
    $mihomoYaml = @(
        "mixed-port: $mihomoPort",
        'bind-address: 127.0.0.1',
        'allow-lan: false',
        'mode: rule',
        'log-level: warning',
        'rules:',
        '  - MATCH,DIRECT'
    ) -join "`n"
    Set-Content -LiteralPath $mihomoCfg -Value $mihomoYaml -Encoding utf8
    $mihomoProc = Start-Process -FilePath $mihomoExe -ArgumentList @('-f', $mihomoCfg, '-d', $mihomoDir) -PassThru -WindowStyle Hidden
    $startedPids.Add($mihomoProc.Id)
    $mihomoListened = Wait-Port -Port $mihomoPort -TimeoutMs 20000
    $mihomoHttp = $null
    if ($mihomoListened) {
        $mihomoHttp = Invoke-ProxiedGet -ProxyPort $mihomoPort -Url "http://127.0.0.1:$targetPort/r4-21" -Scheme 'http'
    }
    $results.cores += [ordered]@{
        core = 'mihomo'; version = '1.19.32'; managed_dir = $mihomoDir
        args = @('-f', '{cfg}', '-d', '{dir}'); proxy_port = $mihomoPort
        listened = $mihomoListened; proxied_ok = ($mihomoHttp -and $mihomoHttp.ok); proxied_body = $(if ($mihomoHttp) { $mihomoHttp.body } else { $null })
        proxied_status = $(if ($mihomoHttp) { $mihomoHttp.status } else { $null }); proxied_error = $(if ($mihomoHttp) { $mihomoHttp.error } else { $null })
        pid = $mihomoProc.Id
    }
}
finally {
    # Stop only this script's PIDs; verify nothing we started survives.
    $stopReport = @()
    foreach ($id in $startedPids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 200 }
        $still = Get-Process -Id $id -ErrorAction SilentlyContinue
        $stopReport += [ordered]@{ pid = $id; stopped = (-not $still) }
    }
    $results.stopped = $stopReport
    $results.port_10808_used = $results.cores | Where-Object { $_.proxy_port -eq 10808 } | ForEach-Object { $true }
    if (-not $results.port_10808_used) { $results.port_10808_used = $false }
    $results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'local-install-launch.json') -Encoding utf8
    $results | ConvertTo-Json -Depth 8
}
