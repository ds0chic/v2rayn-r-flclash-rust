<#
.SYNOPSIS
  Minimal real-session matrix for the locked cores.

  For each core it writes a synthetic loopback-only config (no real nodes),
  picks a free port >= 11808 (probed first), starts the real process using the
  adapter-contract run args / env / working directory, confirms a listener,
  then stops only the PID it started and verifies cleanup.

  Hard constraints: never touches 127.0.0.1:10808, the system proxy, routing or
  TUN; only stops processes this script started.
#>
[CmdletBinding()]
param(
  [string[]]$Only,
  [int]$TimeoutSeconds = 25,
  [string]$OutFile = (Join-Path $PSScriptRoot 'logs\session-results.json')
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) 'v2rayn-r-core-sessions'

function Get-FreePort {
  param([int]$Start = 11808)
  for ($p = $Start; $p -lt ($Start + 900); $p++) {
    if ($p -eq 10808) { continue }
    $listener = $null
    try {
      $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $p)
      $listener.Start()
      $listener.Stop()
      return $p
    } catch {
      if ($listener) { try { $listener.Stop() } catch {} }
    }
  }
  throw "no free port in range $Start..$($Start + 900)"
}

function Test-Listening {
  param([int]$Port)
  $conn = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
  return [bool]$conn
}

function Test-PortListening {
  param([int]$Port, [string]$Protocol = 'tcp')
  if ($Protocol -eq 'udp') {
    return [bool](Get-NetUDPEndpoint -LocalPort $Port -ErrorAction SilentlyContinue)
  }
  return [bool](Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue)
}

# Generate a throwaway self-signed PEM cert/key pair in TEMP using the locked
# hysteria2 `cert` subcommand. Only the cert hash/pin is recorded, never the key.
function New-SyntheticCert {
  param([string]$CertDir, [string]$HysteriaExe)
  $cert = Join-Path $CertDir 'server.crt'
  $key = Join-Path $CertDir 'server.key'
  if (-not (Test-Path -LiteralPath $HysteriaExe)) { throw "cert generator missing: $HysteriaExe" }
  $genOut = Join-Path $CertDir 'gen.out'
  $genErr = Join-Path $CertDir 'gen.err'
  $p = Start-Process -FilePath $HysteriaExe -PassThru -NoNewWindow `
    -ArgumentList @('cert', '--cert', $cert, '--key', $key, '--host', '127.0.0.1,example.com,localhost', '--overwrite') `
    -RedirectStandardOutput $genOut -RedirectStandardError $genErr
  $null = $p.WaitForExit(15000)
  if (-not $p.HasExited) { try { $p.Kill($true) } catch {} }
  if (-not (Test-Path -LiteralPath $cert) -or -not (Test-Path -LiteralPath $key)) {
    throw "cert generation failed: $((Get-Content -Raw $genErr) + (Get-Content -Raw $genOut))"
  }
  $pin = $null
  $gen = "$(Get-Content -Raw $genOut)$(Get-Content -Raw $genErr)"
  if ($gen -match 'pinSHA256:\s*([0-9a-fA-F]{64})') { $pin = $Matches[1].ToLower() }
  [pscustomobject]@{
    cert        = $cert
    key         = $key
    pin_sha256  = $pin
    cert_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $cert).Hash.ToLower()
    generator   = 'hysteria cert'
  }
}

# Two-process session: an explicit TLS server (self-signed cert) plus a client
# launched through the adapter contract (run_args + cwd), then a real proxied
# HTTP GET through the client listener. Also probes the failure path first by
# pointing a client at a closed port. Every process is tracked by PID.
function Invoke-PairSession {
  param(
    [pscustomobject]$Core,
    [string]$ExePath,
    [string]$RepoRoot,
    [int]$TimeoutSeconds
  )
  $listenProto = if ($Core.listen_protocol) { $Core.listen_protocol } else { 'tcp' }
  $dir = Join-Path $TempRoot ("{0}-pair-{1}" -f $Core.core, [guid]::NewGuid().ToString('N').Substring(0,8))
  $certDir = Join-Path $dir 'certs'
  $srvDir = Join-Path $dir 'server'
  $cliDir = Join-Path $dir 'client'
  $negDir = Join-Path $dir 'negative'
  New-Item -ItemType Directory -Force -Path $certDir, $srvDir, $cliDir, $negDir | Out-Null

  $hysteriaExe = Join-Path $RepoRoot 'tools/cores/hysteria/v2.12.3/hysteria-windows-amd64.exe'
  $certInfo = New-SyntheticCert -CertDir $certDir -HysteriaExe $hysteriaExe

  $sp = Get-FreePort
  $cp = Get-FreePort ($sp + 1)
  $tp = Get-FreePort ($sp + 2)
  $negPort = Get-FreePort ($sp + 3)

  $scfgPath = Join-Path $srvDir 'config.json'
  $ccfgPath = Join-Path $cliDir 'config.json'
  $ncfgPath = Join-Path $negDir 'config.json'

  $certJson = $certInfo.cert.Replace('\', '\\')
  $keyJson = $certInfo.key.Replace('\', '\\')
  $scfg = $Core.server_cfg.Replace('{SP}', "$sp").Replace('{TP}', "$tp").Replace('{CERT}', $certJson).Replace('{KEY}', $keyJson)
  $ccfg = $Core.cfg.Replace('{SP}', "$sp").Replace('{CP}', "$cp")
  $ncfg = $Core.cfg.Replace('{SP}', "$negPort").Replace('{CP}', "$cp")
  Set-Content -LiteralPath $scfgPath -Value $scfg -Encoding utf8 -NoNewline
  Set-Content -LiteralPath $ccfgPath -Value $ccfg -Encoding utf8 -NoNewline
  Set-Content -LiteralPath $ncfgPath -Value $ncfg -Encoding utf8 -NoNewline

  $clientCwd = if ($Core.cwd -eq 'cfgdir') { $cliDir } else { $RepoRoot }
  $negCwd = if ($Core.cwd -eq 'cfgdir') { $negDir } else { $RepoRoot }

  # Local HTTP target for the end-to-end proxied GET. Spawned as its own
  # process (not Start-Job) so a blocked accept can always be stopped by PID.
  $targetHelper = Join-Path $dir 'http_target.ps1'
  $targetScript = @'
param([int]$Port, [string]$Body)
$l = [System.Net.HttpListener]::new()
$l.Prefixes.Add("http://127.0.0.1:$Port/")
$l.Start()
try {
  $ctx = $l.GetContext()
  $b = [System.Text.Encoding]::UTF8.GetBytes($Body)
  $ctx.Response.StatusCode = 200
  $ctx.Response.OutputStream.Write($b, 0, $b.Length)
  $ctx.Response.Close()
} catch { } finally { try { $l.Stop() } catch { } }
'@
  Set-Content -LiteralPath $targetHelper -Value $targetScript -Encoding utf8 -NoNewline
  $pwshExe = Join-Path $PSHOME 'pwsh.exe'
  $tgtOut = Join-Path $dir 'target.out'
  $tgtErr = Join-Path $dir 'target.err'
  $targetProc = Start-Process -FilePath $pwshExe -PassThru -NoNewWindow -WorkingDirectory $dir `
    -ArgumentList @('-NoProfile', '-File', $targetHelper, '-Port', "$tp", '-Body', $Core.expected_body) `
    -RedirectStandardOutput $tgtOut -RedirectStandardError $tgtErr
  Start-Sleep -Milliseconds 800

  $serverArgs = @()
  foreach ($a in $Core.server_args) { $serverArgs += $a.Replace('{scfg}', $scfgPath) }
  $clientArgs = @()
  foreach ($a in $Core.args) { $clientArgs += $a.Replace('{cfg}', $ccfgPath) }
  $negArgs = @()
  foreach ($a in $Core.args) { $negArgs += $a.Replace('{cfg}', $ncfgPath) }

  $srvOut = Join-Path $srvDir 'stdout.txt'; $srvErr = Join-Path $srvDir 'stderr.txt'
  $cliOut = Join-Path $cliDir 'stdout.txt'; $cliErr = Join-Path $cliDir 'stderr.txt'
  $negOut = Join-Path $negDir 'stdout.txt'; $negErr = Join-Path $negDir 'stderr.txt'

  $status = 'blocked'; $reason = ''
  $srvProc = $null; $cliProc = $null; $negProc = $null; $targetProc = $null
  $srvListened = $false; $cliListened = $false
  $proxiedBody = $null; $proxiedExit = $null
  $negExited = $false; $negExitCode = $null; $negWaitedMs = 0
  $srvStopped = $false; $cliStopped = $false; $negStopped = $false
  $srvStill = $false; $cliStill = $false; $negStill = $false

  try {
    # Failure semantics: a client pointed at a closed port must not establish a tunnel.
    $negSw = [System.Diagnostics.Stopwatch]::StartNew()
    $negProc = Start-Process -FilePath $ExePath -ArgumentList $negArgs -PassThru -NoNewWindow `
      -WorkingDirectory $negCwd -RedirectStandardOutput $negOut -RedirectStandardError $negErr
    while (-not $negProc.HasExited -and $negSw.ElapsedMilliseconds -lt 9000) { Start-Sleep -Milliseconds 300 }
    $negExited = $negProc.HasExited
    $negWaitedMs = [int]$negSw.Elapsed.TotalMilliseconds
    if (-not $negProc.HasExited) { try { Stop-Process -Id $negProc.Id -Force } catch {}; $negStopped = $true; $negProc.WaitForExit(3000) | Out-Null }
    if ($negProc.HasExited) { $negExitCode = $negProc.ExitCode }
    $negStill = [bool](Get-Process -Id $negProc.Id -ErrorAction SilentlyContinue)

    # Success path: TLS server, then the adapter-contract client.
    $srvProc = Start-Process -FilePath $ExePath -ArgumentList $serverArgs -PassThru -NoNewWindow `
      -WorkingDirectory $srvDir -RedirectStandardOutput $srvOut -RedirectStandardError $srvErr
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
      if (Test-PortListening -Port $sp -Protocol $listenProto) { $srvListened = $true; break }
      if ($srvProc.HasExited) { break }
      Start-Sleep -Milliseconds 400
    }
    if ($srvListened) {
      $cliProc = Start-Process -FilePath $ExePath -ArgumentList $clientArgs -PassThru -NoNewWindow `
        -WorkingDirectory $clientCwd -RedirectStandardOutput $cliOut -RedirectStandardError $cliErr
      $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
      while ((Get-Date) -lt $deadline) {
        if (Test-PortListening -Port $cp -Protocol 'tcp') { $cliListened = $true; break }
        if ($cliProc.HasExited) { break }
        Start-Sleep -Milliseconds 400
      }
    }
    if ($cliListened) {
      $proxiedBody = (& curl.exe -s --max-time 10 --socks5 "127.0.0.1:$cp" "http://127.0.0.1:$tp/" 2>&1 | Out-String).Trim()
      $proxiedExit = $LASTEXITCODE
    }
    if ($srvListened -and $cliListened -and ($proxiedBody -eq $Core.expected_body)) {
      $status = 'ok'
    } else {
      $status = 'blocked'
      if (-not $srvListened) { $reason = 'TLS server did not listen' }
      elseif (-not $cliListened) { $reason = 'client did not bind the local listener' }
      else { $reason = "proxied check failed (body='$proxiedBody')" }
    }
  } catch {
    $status = 'blocked'; $reason = "pair error: $($_.Exception.Message)"
  } finally {
    foreach ($proc in @($cliProc, $srvProc, $negProc, $targetProc)) {
      if ($proc -and -not $proc.HasExited) {
        try { Stop-Process -Id $proc.Id -Force -ErrorAction Stop } catch {}
        try { $proc.WaitForExit(5000) | Out-Null } catch {}
      }
    }
    $srvStill = if ($srvProc) { [bool](Get-Process -Id $srvProc.Id -ErrorAction SilentlyContinue) } else { $false }
    $cliStill = if ($cliProc) { [bool](Get-Process -Id $cliProc.Id -ErrorAction SilentlyContinue) } else { $false }
    $negStill = if ($negProc) { [bool](Get-Process -Id $negProc.Id -ErrorAction SilentlyContinue) } else { $false }
    $tgtStill = if ($targetProc) { [bool](Get-Process -Id $targetProc.Id -ErrorAction SilentlyContinue) } else { $false }
    $srvStopped = -not $srvStill
    $cliStopped = -not $cliStill
    $negStopped = -not $negStill
    $tgtStopped = -not $tgtStill
  }

  $result = [pscustomobject]@{
    core                   = $Core.core
    version                = $Core.version
    mode                   = 'pair'
    status                 = $status
    reason                 = $reason
    server_port            = $sp
    client_port            = $cp
    target_port            = $tp
    server_listen_protocol = $listenProto
    server_listened        = $srvListened
    client_listened        = $cliListened
    proxied_http_body      = $proxiedBody
    proxied_curl_exit      = $proxiedExit
    negative_probe         = [pscustomobject]@{
      exited    = $negExited
      exit_code = $negExitCode
      waited_ms = $negWaitedMs
      note      = 'client against a closed server port'
    }
    cert                   = [pscustomobject]@{
      sha256     = $certInfo.cert_sha256
      pin_sha256 = $certInfo.pin_sha256
      generator  = $certInfo.generator
    }
    adapter_run_args       = $clientArgs
    server_run_args        = $serverArgs
    cwd                    = $clientCwd
    server_stopped         = $srvStopped
    server_still_running   = $srvStill
    client_stopped         = $cliStopped
    client_still_running   = $cliStill
    negative_stopped       = $negStopped
    negative_still_running = $negStill
    target_stopped         = $tgtStopped
    target_still_running   = $tgtStill
    server_stdout          = if (Test-Path $srvOut) { @((Get-Content -Raw $srvOut) -split "`r?`n" | Where-Object { $_.Trim() -ne '' } | Select-Object -First 8) } else { @() }
    server_stderr          = if (Test-Path $srvErr) { @((Get-Content -Raw $srvErr) -split "`r?`n" | Where-Object { $_.Trim() -ne '' } | Select-Object -First 8) } else { @() }
    client_stdout          = if (Test-Path $cliOut) { @((Get-Content -Raw $cliOut) -split "`r?`n" | Where-Object { $_.Trim() -ne '' } | Select-Object -First 8) } else { @() }
    client_stderr          = if (Test-Path $cliErr) { @((Get-Content -Raw $cliErr) -split "`r?`n" | Where-Object { $_.Trim() -ne '' } | Select-Object -First 8) } else { @() }
    negative_stderr        = if (Test-Path $negErr) { @((Get-Content -Raw $negErr) -split "`r?`n" | Where-Object { $_.Trim() -ne '' } | Select-Object -First 6) } else { @() }
  }
  if (Test-Path $dir) { Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue }
  return $result
}

$cores = @(
  [pscustomobject]@{
    core='xray'; exe='tools/cores/xray/v26.3.27/xray.exe'; version='v26.3.27';
    args=@('run','-c','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":PORT,"protocol":"socks","settings":{"udp":false}}],"outbounds":[{"protocol":"freedom"}]}'
  },
  [pscustomobject]@{
    core='sing-box'; exe='tools/cores/singbox/v1.14.2/sing-box.exe'; version='v1.14.2';
    args=@('run','-c','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"log":{"level":"warning"},"inbounds":[{"type":"socks","tag":"in","listen":"127.0.0.1","listen_port":PORT}],"outbounds":[{"type":"direct","tag":"direct"}]}'
  },
  [pscustomobject]@{
    core='v2fly'; exe='tools/cores/v2fly/v4.45.2/v2ray.exe'; version='v4.45.2';
    args=@('-config','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":PORT,"protocol":"socks","settings":{"udp":false}}],"outbounds":[{"protocol":"freedom"}]}'
  },
  [pscustomobject]@{
    core='v2fly_v5'; exe='tools/cores/v2fly/v5.53.0/v2ray.exe'; version='v5.53.0';
    args=@('run','-c','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":PORT,"protocol":"socks","settings":{"udp":false}}],"outbounds":[{"protocol":"freedom"}]}'
  },
  [pscustomobject]@{
    core='mihomo'; exe='tools/cores/mihomo/v1.19.32/mihomo-windows-amd64-v1.exe'; version='v1.19.32';
    args=@('-f','{cfg}','-d','{dir}'); cwd='default'; env=''; cfgName='config.yaml';
    cfg="mixed-port: PORT`nallow-lan: false`nbind-address: 127.0.0.1`nmode: direct`nlog-level: warning`n"
  },
  [pscustomobject]@{
    core='hysteria'; exe='tools/cores/hysteria/v1.3.5/hysteria.exe'; version='v1.3.5';
    args=@(); cwd='cfgdir'; env=''; cfgName='config.json';
    cfg='{"server":"127.0.0.1:1","auth_str":"synthetic","up_mbps":20,"down_mbps":20,"lazy_start":true,"socks5":{"listen":"127.0.0.1:PORT"}}'
  },
  [pscustomobject]@{
    core='naiveproxy'; exe='tools/cores/naiveproxy/v154.0.8037.49-2/naive.exe'; version='v154.0.8037.49-2';
    args=@('{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"listen":"socks://127.0.0.1:PORT","proxy":"https://synthetic:synthetic@127.0.0.1:1"}'
  },
  [pscustomobject]@{
    core='tuic'; exe='tools/cores/tuic/v1.0.0/tuic-client.exe'; version='v1.0.0';
    args=@('-c','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"relay":{"server":"127.0.0.1:1","uuid":"00000000-0000-0000-0000-000000000000","password":"synthetic"},"local":{"server":"127.0.0.1:PORT"}}'
  },
  [pscustomobject]@{
    core='juicity'; exe='tools/cores/juicity/v0.5.0/juicity-client.exe'; version='v0.5.0';
    args=@('run','-c','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"listen":"127.0.0.1:PORT","server":"127.0.0.1:1","uuid":"00000000-0000-0000-0000-000000000000","password":"synthetic","sni":"example.com","allow_insecure":true,"congestion_control":"bbr","log_level":"warn"}'
  },
  [pscustomobject]@{
    core='hysteria2'; exe='tools/cores/hysteria/v2.12.3/hysteria-windows-amd64.exe'; version='v2.12.3';
    kind='pair';
    args=@(); cwd='cfgdir'; env=''; cfgName='config.json';
    cfg='{"server":"127.0.0.1:{SP}","auth":"synthetic","disableUpdateCheck":true,"tls":{"sni":"example.com","insecure":true},"socks5":{"listen":"127.0.0.1:{CP}"}}';
    server_args=@('server','-c','{scfg}');
    server_cfg='{"listen":"127.0.0.1:{SP}","disableUpdateCheck":true,"tls":{"cert":"{CERT}","key":"{KEY}"},"auth":{"type":"password","password":"synthetic"}}';
    listen_protocol='udp'; expected_body='HY2-OK'
  },
  [pscustomobject]@{
    core='brook'; exe='tools/cores/brook/v20270101/brook_windows_amd64.exe'; version='v20270101';
    args=@('{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='socks5 --listen 127.0.0.1:PORT'
  },
  [pscustomobject]@{
    core='overtls'; exe='tools/cores/overtls/v0.3.15/overtls-bin.exe'; version='v0.3.15';
    kind='pair';
    args=@('-r','client','-c','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"remarks":"synthetic","test_timeout_secs":10,"method":"none","password":"synthetic","tunnel_path":"/synthetic-tunnel-path/","client_settings":{"disable_tls":false,"client_id":"00000000-0000-0000-0000-000000000000","server_host":"127.0.0.1","server_port":{SP},"server_domain":"example.com","cafile":"","dangerous_mode":true,"advertise_ip":"127.0.0.1","max_lifetime":3600,"pool_max_size":30,"listen":"mixed://127.0.0.1:{CP}"}}';
    server_args=@('-r','server','-c','{scfg}');
    server_cfg='{"remarks":"synthetic","test_timeout_secs":10,"method":"none","password":"synthetic","tunnel_path":"/synthetic-tunnel-path/","server_settings":{"disable_tls":false,"certfile":"{CERT}","keyfile":"{KEY}","forward_addr":"http://127.0.0.1:{TP}","listen_host":"127.0.0.1","listen_port":{SP}}}';
    listen_protocol='tcp'; expected_body='OV-OK'
  },
  [pscustomobject]@{
    core='shadowquic'; exe='tools/cores/shadowquic/v0.4.0/shadowquic.exe'; version='v0.4.0';
    args=@('-c','{cfg}'); cwd='default'; env=''; cfgName='config.yaml';
    cfg="inbounds:`n- type: socks`n  tag: local-socks`n  bind-addr: `"127.0.0.1:PORT`"`noutbounds:`n- type: direct`n  tag: direct`nrouter:`n  default-outbound: direct`n"
  },
  [pscustomobject]@{
    core='mieru'; exe='tools/cores/mieru/v3.38.0/mieru.exe'; version='v3.38.0';
    args=@('run'); cwd='default'; env='MIERU_CONFIG_JSON_FILE={cfg}'; cfgName='config.json';
    cfg='{"profiles":[{"profileName":"default","user":{"name":"synthetic","password":"synthetic"},"servers":[{"ipAddress":"127.0.0.1","domainName":"","portBindings":[{"port":1,"protocol":"TCP"}]}],"mtu":1400}],"activeProfile":"default","rpcPort":PORT2,"socks5Port":PORT,"loggingLevel":"INFO","socks5ListenLAN":false}'
  }
)

$selected = if ($Only) { $cores | Where-Object { $Only -contains $_.core } } else { $cores }
$results = [System.Collections.Generic.List[object]]::new()
New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null

foreach ($c in $selected) {
  Write-Output "==> $($c.core) $($c.version)"
  $exePath = Join-Path $RepoRoot $c.exe
  if (-not (Test-Path -LiteralPath $exePath)) {
    $results.Add([pscustomobject]@{ core=$c.core; version=$c.version; status='blocked'; reason='executable missing' })
    Write-Output "    blocked: executable missing"
    continue
  }

  if ($c.kind -eq 'pair') {
    Write-Output "    pair session: self-signed TLS server + adapter-contract client + proxied HTTP GET"
    $pairResult = Invoke-PairSession -Core $c -ExePath $exePath -RepoRoot $RepoRoot -TimeoutSeconds $TimeoutSeconds
    $results.Add($pairResult)
    Write-Output ("    status={0} server_listened={1} client_listened={2} proxied='{3}' neg_exit={4} server_still={5} client_still={6} {7}" -f `
      $pairResult.status, $pairResult.server_listened, $pairResult.client_listened, $pairResult.proxied_http_body, `
      $pairResult.negative_probe.exit_code, $pairResult.server_still_running, $pairResult.client_still_running, $pairResult.reason)
    continue
  }

  $port = Get-FreePort
  $port2 = $port + 1
  $dir = Join-Path $TempRoot ("{0}-{1}" -f $c.core, [guid]::NewGuid().ToString('N').Substring(0,8))
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  $cfgPath = Join-Path $dir $c.cfgName
  $body = $c.cfg.Replace('{PORT2}', "$port2").Replace('PORT2', "$port2").Replace('PORT', "$port")
  Set-Content -LiteralPath $cfgPath -Value $body -Encoding utf8 -NoNewline

  $finalArgs = @()
  foreach ($a in $c.args) { $finalArgs += ($a.Replace('{cfg}', $cfgPath).Replace('{dir}', $dir)) }
  $envPairs = @()
  if ($c.env) { $envPairs = @($c.env.Replace('{cfg}', $cfgPath)) }

  $procOut = Join-Path $dir 'stdout.txt'
  $procErr = Join-Path $dir 'stderr.txt'
  $cwd = if ($c.cwd -eq 'cfgdir') { $dir } else { $RepoRoot }

  $proc = $null
  $listened = $false
  $exitedEarly = $false
  $status = 'blocked'
  $reason = ''
  try {
    $startArgs = @{
      FilePath = $exePath
      PassThru = $true
      NoNewWindow = $true
      RedirectStandardOutput = $procOut
      RedirectStandardError = $procErr
      WorkingDirectory = $cwd
    }
    if ($finalArgs.Count -gt 0) { $startArgs.ArgumentList = $finalArgs }
    # Start-Process sets env per-process via -Environment (PowerShell 7.4+).
    if ($envPairs.Count -gt 0) {
      $envHash = @{}
      foreach ($pair in $envPairs) { $kv = $pair -split '=', 2; $envHash[$kv[0]] = $kv[1] }
      $startArgs.Environment = $envHash
    }
    $proc = Start-Process @startArgs
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
      if (Test-Listening -Port $port) { $listened = $true; break }
      if ($proc.HasExited) { $exitedEarly = $true; break }
      Start-Sleep -Milliseconds 400
    }
    if ($listened) {
      $status = 'ok'
    } else {
      $status = 'blocked'
      $reason = if ($exitedEarly) { "process exited early (code $($proc.ExitCode))" } else { 'no listener before timeout' }
    }
  } catch {
    $status = 'blocked'
    $reason = "start error: $($_.Exception.Message)"
  } finally {
    $stopped = $false
    if ($proc -and -not $proc.HasExited) {
      try { Stop-Process -Id $proc.Id -Force -ErrorAction Stop; $stopped = $true } catch {}
      try { $proc.WaitForExit(5000) | Out-Null } catch {}
    } elseif ($proc) {
      $stopped = $proc.HasExited
    }
    $stillRunning = $false
    if ($proc) {
      $stillRunning = [bool](Get-Process -Id $proc.Id -ErrorAction SilentlyContinue)
    }
    $stdout = if (Test-Path $procOut) { Get-Content -Raw $procOut } else { '' }
    $stderr = if (Test-Path $procErr) { Get-Content -Raw $procErr } else { '' }
    $result = [pscustomobject]@{
      core          = $c.core
      version       = $c.version
      status        = $status
      reason        = $reason
      port          = $port
      listened      = $listened
      exit_code     = if ($proc -and $proc.HasExited) { $proc.ExitCode } else { $null }
      stopped       = $stopped
      still_running = $stillRunning
      run_args      = $finalArgs
      cwd           = $cwd
      env           = $envPairs
      stdout        = ($stdout -split "`r?`n" | Where-Object { $_.Trim() -ne '' } | Select-Object -First 8)
      stderr        = ($stderr -split "`r?`n" | Where-Object { $_.Trim() -ne '' } | Select-Object -First 8)
    }
    $results.Add($result)
    Write-Output ("    status={0} listened={1} port={2} stopped={3} stillRunning={4} {5}" -f $status,$listened,$port,$stopped,$stillRunning,$reason)
  }
  if (Test-Path $dir) { Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue }
}

New-Item -ItemType Directory -Force -Path (Split-Path $OutFile -Parent) | Out-Null
$results | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $OutFile -Encoding utf8
Write-Output "wrote $OutFile"
