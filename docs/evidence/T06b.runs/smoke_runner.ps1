# T06b real data-path smoke: generate internal-capable configs, start the real
# cores, fetch from a local HTTP server over the generated socks inbound, and
# stop everything. Never references 10808, never starts TUN, never touches the
# system proxy. Only processes started by this script are stopped.
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
    [int]$HttpPort = 11880,
    [int]$SocksPort = 11808
)
$ErrorActionPreference = 'Stop'
$out = Join-Path $RepoRoot 'target/t06b/smoke'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$Xray = Join-Path $RepoRoot 'tools/cores/xray/v26.3.27/xray.exe'
$Sbox = Join-Path $RepoRoot 'tools/cores/singbox/v1.14.2/sing-box.exe'
$NodeServer = Join-Path $RepoRoot 'target/t06b/http_server.js'
$timeline = New-Object System.Collections.ArrayList
function T($msg) { [void]$timeline.Add(("{0} {1}" -f (Get-Date -Format 'HH:mm:ss'), $msg)); Write-Host $msg }

# ---- local HTTP server (owned by this script) ----
$httpBody = 'T06B-SMOKE-OK'
$httpProc = Start-Process node -ArgumentList "`"$NodeServer`" $HttpPort" -PassThru -WindowStyle Hidden
T "HTTP server started 127.0.0.1:$HttpPort pid=$($httpProc.Id)"
for ($i = 0; $i -lt 20; $i++) { Start-Sleep -Milliseconds 250; if ((curl.exe -s --max-time 2 "http://127.0.0.1:$HttpPort/") -eq $httpBody) { break } }

function Write-XrayConfig([string]$path, [string]$tag, [string]$socksUser, [string]$socksPass) {
    $settings = @{ auth = if ($socksUser) { 'password' } else { 'noauth' }; udp = $true; allowTransparent = $false }
    if ($socksUser) { $settings.accounts = @(@{ user = $socksUser; pass = $socksPass }) }
    $cfg = @{
        log = @{ loglevel = 'warning' }
        inbounds = @(@{
            tag = 'socks'; port = $SocksPort; protocol = 'mixed'; listen = '127.0.0.1'
            settings = $settings
            sniffing = @{ enabled = $true; destOverride = @('http','tls'); routeOnly = $false }
        })
        outbounds = @(@{ tag = $tag; protocol = 'freedom' })
    }
    $cfg | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $path -Encoding utf8
}

function Write-SboxConfig([string]$path, [string]$tag, [string]$socksUser, [string]$socksPass) {
    $inbound = @{ type = 'mixed'; tag = 'socks'; listen = '127.0.0.1'; listen_port = $SocksPort }
    if ($socksUser) { $inbound.users = @(@{ username = $socksUser; password = $socksPass }) }
    $cfg = @{
        log = @{ level = 'warn'; timestamp = $true }
        inbounds = @($inbound)
        outbounds = @(@{ type = 'direct'; tag = $tag })
        route = @{ rules = @(); final = $tag }
    }
    $cfg | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $path -Encoding utf8
}

function Start-Core([string]$bin, [string]$cfg) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $bin
    $psi.ArgumentList.Add('run'); $psi.ArgumentList.Add('-c'); $psi.ArgumentList.Add($cfg)
    $psi.UseShellExecute = $false; $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true; $psi.CreateNoWindow = $true
    $p = [System.Diagnostics.Process]::new(); $p.StartInfo = $psi; [void]$p.Start()
    return $p
}

function Invoke-Smoke([string]$core, [string]$bin, [string]$cfg, [string]$label, [string]$user, [string]$pass) {
    $log = Join-Path $out "$core--$label.log"
    $proc = Start-Core $bin $cfg
    $outTask = $proc.StandardOutput.ReadToEndAsync()
    $errTask = $proc.StandardError.ReadToEndAsync()
    T "$core $label started pid=$($proc.Id) cfg=$cfg"
    $ok = $false; $body = $null
    for ($i = 0; $i -lt 16; $i++) {
        Start-Sleep -Milliseconds 500
        if ($proc.HasExited) { T "$core $label exited early code=$($proc.ExitCode)"; break }
        $curlArgs = @('-s','--max-time','4','-o','-')
        if ($user) { $curlArgs += @('--proxy-user', "$user`:$pass") }
        $curlArgs += @('-x', 'socks5h://127.0.0.1:11808', "http://127.0.0.1:$HttpPort/")
        $body = & curl.exe @curlArgs 2>$null
        if ($body -eq $httpBody) { $ok = $true; break }
    }
    if ($ok) { T "$core $label probe OK: '$body'" } else { T "$core $label probe FAILED: '$body'" }
    if (-not $proc.HasExited) { $proc.Kill(); T "$core $label stopped pid=$($proc.Id)" }
    $proc.WaitForExit(5000) | Out-Null
    ($outTask.Result + "`n" + $errTask.Result) | Set-Content -LiteralPath $log -Encoding utf8
    return [pscustomobject]@{ core = $core; label = $label; pid = $proc.Id; ok = $ok; body = $body; log = $log }
}

$results = @()
foreach ($auth in @($false, $true)) {
    $suffix = if ($auth) { 'auth' } else { 'noauth' }
    $u = if ($auth) { 'smokeuser' } else { '' }
    $pw = if ($auth) { 'smokepass' } else { '' }

    $xc = Join-Path $out "xray-smoke-$suffix.json"
    Write-XrayConfig $xc 'smokedirect' $u $pw
    $results += Invoke-Smoke 'xray' $Xray $xc $suffix $u $pw

    $sc = Join-Path $out "singbox-smoke-$suffix.json"
    Write-SboxConfig $sc 'smokedirect' $u $pw
    $results += Invoke-Smoke 'singbox' $Sbox $sc $suffix $u $pw
}

if (-not $httpProc.HasExited) { Stop-Process -Id $httpProc.Id -Force }
T "HTTP server stopped pid=$($httpProc.Id)"

$results | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $out 'smoke-results.json') -Encoding utf8
$timeline | Set-Content -LiteralPath (Join-Path $out 'smoke-timeline.txt') -Encoding utf8
Write-Host ("SMOKE: {0}/{1} ok" -f (($results | Where-Object ok).Count), $results.Count)
