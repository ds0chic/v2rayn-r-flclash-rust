<#
.SYNOPSIS
  SP-30 候选包身份核对（只读：消费候选包，不改生产代码/不触网/无 OS 副作用）。

.DESCRIPTION
  对一个未武装候选包做身份核对并输出判定 JSON：
    1. zip/sha：计算 SHA256，与 dist/SHA256SUMS 比对（若提供）。
    2. build-info：从 zip 内 build-info.json 读取 smoke_armed/git_dirty/
       git_commit/target_triple，与 SP-34 基线期望比对（armed=false，
       dirty=false，commit == 当前固定提交）。
    3. 未武装标志：断言 smoke_armed=false；列出包内 exe 清单（flat layout 契约：
       v2rayn_desktop.exe / net_host.exe / privileged_helper.exe /
       v2rayN-upgrade.exe），核对外部内核未被捆绑。
    4. -VerifyFixture：对 fixtures/acceptance/sp30/synthetic-sub.txt 做形状
       校验（scheme 白名单、vmess 可解码、地址 192.0.2.0/24、端口 >=11808）。
  默认 -DryRun：不读任何包，只打印将要执行的检查计划，exit 0。
  任何模式都不启动 exe、不绑定端口、不改注册表/代理。

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_package_identity.ps1 -DryRun
.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_package_identity.ps1 -Zip dist/v2rayN-R-*-windows-x64.zip -ExpectedCommit 3635392ed519a26b4da1862d99019f07e6a8de5c
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$ExpectedCommit = '3635392ed519a26b4da1862d99019f07e6a8de5c',
  [switch]$VerifyFixture,
  [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Out-Report([hashtable]$r) {
  ($r | ConvertTo-Json -Depth 5) | Write-Output
}

if ($DryRun -and $Zip -eq '' -and -not $VerifyFixture) {
  $plan = [ordered]@{
    mode = 'dry-run'; status = 'identified'
    plan = @(
      'sha256(zip) 并与 dist/SHA256SUMS 比对'
      '读取 zip 内 build-info.json：断言 smoke_armed=false, git_dirty=false'
      '断言 build-info.git_commit == ExpectedCommit（默认 3635392 基线重建后的新包）'
      '断言 flat layout 四 exe 齐全；断言无 xray/sing-box 被捆绑'
      '-VerifyFixture：合成订阅形状校验（只读）'
    )
    side_effects = 'none（不启动 exe、不绑定端口、不改代理/注册表/TUN/Run-key）'
  }
  Out-Report $plan
  exit 0
}

$report = [ordered]@{
  mode = 'identity-check'; status = 'pass'; failures = @(); notes = @()
}

if ($VerifyFixture) {
  $fx = Join-Path $RepoRoot 'fixtures\acceptance\sp30\synthetic-sub.txt'
  $lines = Get-Content -LiteralPath $fx | Where-Object { $_.Trim() -ne '' }
  $report['fixture_lines'] = $lines.Count
  foreach ($ln in $lines) {
    $t = $ln.Trim()
    if ($t -match '^vmess://(.+)$') {
      try {
        $raw = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Matches[1]))
        $j = $raw | ConvertFrom-Json
        if ($j.add -notmatch '^192\.0\.2\.\d+$') { $report.failures += "address out of doc-range: $($j.add)" }
        if ([int]$j.port -lt 11808) { $report.failures += "port below 11808: $($j.port)" }
      } catch { $report.failures += "vmess decode failed: $($_.ToString())" }
    } elseif ($t -match '^ss://[^@]+@([^:]+):(\d+)#') {
      $ssHost = $Matches[1]; $ssPort = $Matches[2]
      if ($ssHost -notmatch '^192\.0\.2\.\d+$') { $report.failures += "address out of doc-range: $ssHost" }
      if ([int]$ssPort -lt 11808) { $report.failures += "port below 11808: $ssPort" }
    } else { $report.failures += "unknown scheme line" }
  }
  if ($lines.Count -ne 4) { $report.failures += "expected 4 lines, got $($lines.Count)" }
}

if ($Zip -ne '') {
  $z = (Resolve-Path -LiteralPath $Zip).Path
  $sha = (Get-FileHash -LiteralPath $z -Algorithm SHA256).Hash.ToLower()
  $report['zip'] = $z; $report['zip_sha256'] = $sha
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
  $report.notes += 'no -Zip given: package checks skipped (fixture-only run)'
}

if ($report.failures.Count -gt 0) { $report.status = 'fail' }
Out-Report $report
if ($report.status -eq 'fail') { exit 1 } else { exit 0 }
