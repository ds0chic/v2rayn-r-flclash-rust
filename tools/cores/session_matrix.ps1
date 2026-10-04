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
    args=@(); cwd='cfgdir'; env=''; cfgName='config.json';
    cfg='{"server":"127.0.0.1:1","auth":"synthetic","tls":{"insecure":true},"socks5":{"listen":"127.0.0.1:PORT"}}'
  },
  [pscustomobject]@{
    core='brook'; exe='tools/cores/brook/v20270101/brook_windows_amd64.exe'; version='v20270101';
    args=@('{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='socks5 --listen 127.0.0.1:PORT'
  },
  [pscustomobject]@{
    core='overtls'; exe='tools/cores/overtls/v0.3.15/overtls-bin.exe'; version='v0.3.15';
    args=@('-r','client','-c','{cfg}'); cwd='default'; env=''; cfgName='config.json';
    cfg='{"tunnel_path":"/synthetic-tunnel-path/","client_settings":{"server_host":"127.0.0.1","server_port":1,"server_domain":"example.com","listen":"mixed://127.0.0.1:PORT"}}'
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
