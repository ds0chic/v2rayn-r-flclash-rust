<#
.SYNOPSIS
  Run the adapter-contract version probe (and optional non-binding test) for
  every locked core, with a hard per-process timeout, and emit JSON evidence.

  Never binds a listening port for the version probe. The optional `test`
  command is only run when it is a pure validation command that exits.
#>
[CmdletBinding()]
param(
  [string]$OutFile = (Join-Path $PSScriptRoot 'logs\smoke-results.json'),
  [int]$TimeoutSeconds = 20
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$lock = Get-Content -Raw (Join-Path $PSScriptRoot 'cores.lock.json') | ConvertFrom-Json

# Adapter-contract version args (mirrors crates/runtime/src/adapter.rs).
$VersionArgs = @{
  'xray'       = @('version')
  'sing-box'   = @('version')
  'v2fly'      = @('-version')
  'v2fly_v5'   = @('version')
  'mihomo'     = @('-v')
  'hysteria'   = @('-v')
  'naiveproxy' = @('--version')
  'tuic'       = @('--version')
  'juicity'    = @('--version')
  'hysteria2'  = @('version')
  'brook'      = @('--version')
  'overtls'    = @('--version')
  'shadowquic' = @('--version')
  'mieru'      = @('version')
}

function Invoke-TimedCommand {
  param([string]$Exe, [string[]]$Arguments, [int]$Timeout)
  $tmp = Join-Path $env:TEMP ("core-smoke-" + [guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Force -Path $tmp | Out-Null
  $outFile = Join-Path $tmp 'stdout.txt'
  $errFile = Join-Path $tmp 'stderr.txt'
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $proc = Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru -NoNewWindow `
    -RedirectStandardOutput $outFile -RedirectStandardError $errFile
  $exited = $proc.WaitForExit($Timeout * 1000)
  if (-not $exited) { try { $proc.Kill($true) } catch {} ; $proc.WaitForExit(5000) | Out-Null }
  $sw.Stop()
  $stdout = if (Test-Path $outFile) { Get-Content -Raw $outFile } else { '' }
  $stderr = if (Test-Path $errFile) { Get-Content -Raw $errFile } else { '' }
  $code = if ($exited) { $proc.ExitCode } else { $null }
  $result = [ordered]@{
    exe          = $Exe
    args         = $Arguments
    exit_code    = $code
    timed_out    = (-not $exited)
    duration_ms  = [int]$sw.Elapsed.TotalMilliseconds
    stdout_first = ($stdout -split "`r?`n" | Where-Object { $_ -ne '' } | Select-Object -First 6)
    stderr_first = ($stderr -split "`r?`n" | Where-Object { $_ -ne '' } | Select-Object -First 6)
  }
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  return [pscustomobject]$result
}

$results = [System.Collections.Generic.List[object]]::new()
foreach ($entry in $lock.cores) {
  $exePath = Join-Path $RepoRoot $entry.executable
  $args = $VersionArgs[$entry.core]
  Write-Output "==> $($entry.core): $(Split-Path $exePath -Leaf) $($args -join ' ')"
  if (-not (Test-Path -LiteralPath $exePath)) {
    $results.Add([pscustomobject]@{ core=$entry.core; status='blocked'; reason="executable missing: $exePath" })
    continue
  }
  $r = Invoke-TimedCommand -Exe $exePath -Arguments $args -Timeout $TimeoutSeconds
  $status = if ($r.timed_out) { 'blocked' } elseif ($r.exit_code -eq 0 -and $r.stdout_first.Count -gt 0) { 'ok' } else { 'attention' }
  $obj = [pscustomobject]@{
    core = $entry.core
    version_args = $args
    status = $status
    exit_code = $r.exit_code
    timed_out = $r.timed_out
    duration_ms = $r.duration_ms
    stdout = $r.stdout_first
    stderr = $r.stderr_first
  }
  $results.Add($obj)
  Write-Output ("    status={0} exit={1} timeout={2} :: {3}" -f $status,$r.exit_code,$r.timed_out, ($r.stdout_first -join ' / '))
}

New-Item -ItemType Directory -Force -Path (Split-Path $OutFile -Parent) | Out-Null
$results | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $OutFile -Encoding utf8
Write-Output "wrote $OutFile"
