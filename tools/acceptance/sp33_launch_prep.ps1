<#
.SYNOPSIS
  SP-33 Windows x64 instance launch prep (read-only; -DryRun prints plan only).

.DESCRIPTION
  Runnable prep items before the real build/smoke; never a chain substitute:
    1. -VerifyFixture: shape check on fixtures/acceptance/sp33/synthetic-sub.txt
       (scheme whitelist, vmess decodable, 192.0.2.0/24 doc range, port >= 11808,
       exactly 4 lines).
    2. -Zip <pkg>: candidate package identity (sha256, flat four-exe layout,
       no bundled proxy cores, build-info.json smoke_armed=false /
       git_dirty=false / commit==expected). Without -Zip the check is recorded
       as skipped (historic ZIPs must not be reused).
    3. -ProbePorts: TCP connect probe on 11808..11815 - refused means free,
       connected means busy (stop and pick others). Never touches 10808,
       never listens/binds.
  No mode launches an exe or writes registry/proxy/TUN/route/DNS/Run-key,
  and no user secrets are read.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp33_launch_prep.ps1 -DryRun
.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp33_launch_prep.ps1 -VerifyFixture -ProbePorts
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$ExpectedCommit = '393fafd687fb32dc4270f5332dac86e1efc57961',
  [switch]$VerifyFixture,
  [switch]$ProbePorts,
  [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if ($DryRun -and $Zip -eq '' -and -not $VerifyFixture -and -not $ProbePorts) {
  ([ordered]@{
    mode = 'dry-run'; status = 'identified'
    plan = @(
      '-VerifyFixture: synthetic subscription shape check (read-only)'
      '-Zip <new pkg>: package identity (sha / four exes / no bundled cores / unarmed asserts)'
      '-ProbePorts: 11808..11815 free-port probe (read-only connect; 10808 never touched)'
      'Real items follow the checklist: isolated datadir -> normal-entry launch -> import/apply/backup/reopen/restore/cleanup'
    )
    side_effects = 'none (no exe launch, no port bind, no proxy/registry/TUN/Run-key change)'
  } | ConvertTo-Json -Depth 4) | Write-Output
  exit 0
}

$report = [ordered]@{ mode = 'launch-prep'; baseline = '393fafd'; status = 'pass'; failures = @(); notes = @() }

if ($VerifyFixture) {
  $fx = Join-Path $RepoRoot 'fixtures\acceptance\sp33\synthetic-sub.txt'
  $lines = Get-Content -LiteralPath $fx | Where-Object { $_.Trim() -ne '' }
  $report['fixture_lines'] = $lines.Count
  foreach ($ln in $lines) {
    $t = $ln.Trim()
    if ($t -match '^vmess://(.+)$') {
      try {
        $raw = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[1]))
        $j = $raw | ConvertFrom-Json
        if ($j.add -notmatch '^192\.0\.2\.\d+$') { $report.failures += "address out of doc-range: $($j.add)" }
        if ($j.ps -notmatch '^sp33-synth-') { $report.failures += "remark not sp33-synth-*: $($j.ps)" }
        if ([int]$j.port -lt 11808) { $report.failures += "port below 11808: $($j.port)" }
      } catch { $report.failures += "vmess decode failed: $($_.ToString())" }
    } elseif ($t -match '^ss://[^@]+@([^:]+):(\d+)#(.+)$') {
      $ssHost = $Matches[1]; $ssPort = $Matches[2]
      if ($ssHost -notmatch '^192\.0\.2\.\d+$') { $report.failures += "address out of doc-range: $ssHost" }
      if ([int]$ssPort -lt 11808) { $report.failures += "port below 11808: $ssPort" }
    } else { $report.failures += 'unknown scheme line' }
  }
  if ($lines.Count -ne 4) { $report.failures += "expected 4 lines, got $($lines.Count)" }
}

if ($Zip -ne '') {
  $z = (Resolve-Path -LiteralPath $Zip).Path
  $report['zip'] = $z
  $report['zip_sha256'] = (Get-FileHash -LiteralPath $z -Algorithm SHA256).Hash.ToLower()
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $fs = [System.IO.File]::OpenRead($z)
  try {
    $zipArc = New-Object System.IO.Compression.ZipArchive($fs, [System.IO.Compression.ZipArchiveMode]::Read)
    $names = @($zipArc.Entries | ForEach-Object { $_.FullName })
    $report['entry_count'] = $names.Count
    foreach ($exe in @('v2rayn_desktop.exe', 'net_host.exe', 'privileged_helper.exe', 'v2rayN-upgrade.exe')) {
      if (@($names | Where-Object { $_ -like "*$exe" }).Count -eq 0) { $report.failures += "missing flat-layout exe: $exe" }
    }
    if (@($names | Where-Object { $_ -match '(?i)(^|/)(xray|sing-box)(\.exe)?$' }).Count -gt 0) { $report.failures += 'proxy core bundled (policy: not_bundled)' }
    $bi = @($zipArc.Entries | Where-Object { $_.FullName -like '*build-info.json' }) | Select-Object -First 1
    if ($null -eq $bi) { $report.failures += 'build-info.json not in zip' }
    else {
      $sr = New-Object System.IO.StreamReader($bi.Open())
      try { $info = $sr.ReadToEnd() | ConvertFrom-Json } finally { $sr.Close() }
      $report['build_info'] = @{ smoke_armed = $info.smoke_armed; git_dirty = $info.git_dirty; git_commit = $info.git_commit }
      if ($info.smoke_armed -ne $false) { $report.failures += 'smoke_armed != false' }
      if ($info.git_dirty -ne $false) { $report.failures += 'git_dirty != false' }
      if ($info.git_commit -ne $ExpectedCommit) { $report.failures += "git_commit $($info.git_commit) != expected $ExpectedCommit" }
    }
  } finally { $fs.Close() }
} else {
  $report.notes += 'no -Zip given: package identity skipped (rebuild a clean candidate from baseline first, then fill P0; historic ZIPs must not be reused)'
}

if ($ProbePorts) {
  $busy = @()
  foreach ($p in 11808..11815) {
    $c = New-Object Net.Sockets.TcpClient
    try {
      $iar = $c.BeginConnect('127.0.0.1', $p, $null, $null)
      if ($iar.AsyncWaitHandle.WaitOne(300) -and $c.Connected) { $busy += $p }
    } catch { } finally { $c.Close() }
  }
  $report['ports_probed'] = '11808..11815'
  $report['ports_busy'] = $busy
  $report['port_10808'] = 'untouched (AGENTS.md forbids occupying/modifying it)'
  if ($busy.Count -gt 0) { $report.failures += "ports busy, pick others >=11808: $($busy -join ',')" }
}

if ($report.failures.Count -gt 0) { $report.status = 'fail' }
($report | ConvertTo-Json -Depth 5) | Write-Output
if ($report.status -eq 'fail') { exit 1 } else { exit 0 }
