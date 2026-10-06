<#
.SYNOPSIS
  SP-30 候选包身份核对（只读：消费候选包，不改生产代码/不触网/无 OS 副作用）。

.DESCRIPTION
  对一个未武装候选包做身份核对并输出判定 JSON：
    1. zip/sha：计算 SHA256，与 -ShaFile（默认 RC 运行须传 dist/SHA256SUMS）
       中的同名条目比对；缺文件/缺条目/hash 不一致即 fail。
    2. -Setup <setup.exe>：同上比对 setup 条目（可选；传了才验）。
    3. build-info：从 zip 内 build-info.json 读取 smoke_armed/git_dirty/
       git_commit/target_triple，与 SP-34 基线期望比对（armed=false，
       dirty=false，commit == ExpectedCommit）；缺文件/缺字段即 fail。
    4. -RequireCleanTree：对仓库跑 `git status --porcelain`，非空即 fail
      （dirty tree rejection 的 live 侧；build-info git_dirty=false 是包侧）。
    5. -VerifyFixture：对 fixtures/acceptance/sp30/synthetic-sub.txt 做形状
       校验（scheme 白名单、vmess 可解码、地址 192.0.2.0/24、端口 >=11808）。
  默认 -DryRun：不读任何包，只打印将要执行的检查计划，exit 0。
  任何模式都不启动 exe、不绑定端口、不改注册表/代理。

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_package_identity.ps1 -DryRun
.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_package_identity.ps1 -Zip dist/v2rayN-R-*-windows-x64.zip -ShaFile dist/SHA256SUMS -ExpectedCommit 3635392ed519a26b4da1862d99019f07e6a8de5c -RequireCleanTree
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$ShaFile = '',
  [string]$Setup = '',
  [string]$ExpectedCommit = '3635392ed519a26b4da1862d99019f07e6a8de5c',
  [switch]$VerifyFixture,
  [switch]$RequireCleanTree,
  [string]$RepoRootOverride = '',
  [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$RepoRoot = if ($RepoRootOverride -ne '') { (Resolve-Path -LiteralPath $RepoRootOverride).Path } else { (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path }

function Out-Report([hashtable]$r) {
  ($r | ConvertTo-Json -Depth 5) | Write-Output
}

if ($DryRun -and $Zip -eq '' -and -not $VerifyFixture) {
  $plan = [ordered]@{
    mode = 'dry-run'; status = 'identified'
    plan = @(
      'sha256(zip) 并与 -ShaFile（RC 传 dist/SHA256SUMS）同名条目比对；缺文件/缺条目/不一致即 fail'
      '-Setup <setup.exe>：同文件内同名条目比对（可选；传了才验）'
      '读取 zip 内 build-info.json：断言 smoke_armed=false, git_dirty=false；缺文件/缺字段即 fail'
      '断言 build-info.git_commit == ExpectedCommit（RC 重建时传新基线 commit）'
      '断言 flat layout 四 exe 齐全；断言无 xray/sing-box 被捆绑'
      '-RequireCleanTree：git status --porcelain 非空即 fail（dirty tree live 侧）'
      '-VerifyFixture：合成订阅形状校验（只读）'
      '-RepoRootOverride：仅自测时把 clean-tree 与夹具根指向临时目录；RC 运行不传'
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

function Get-ShaEntry([string]$ShaFilePath, [string]$BaseName) {
  foreach ($line in (Get-Content -LiteralPath $ShaFilePath)) {
    $t = $line.Trim()
    if ($t -eq '' -or $t.StartsWith('#')) { continue }
    $m = [regex]::Match($t, '^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$')
    if (-not $m.Success) { continue }
    $name = [System.IO.Path]::GetFileName($m.Groups[2].Value.Trim())
    if ($name -eq $BaseName) { return $m.Groups[1].Value.ToLower() }
  }
  return $null
}

if ($RequireCleanTree) {
  try {
    $porcelain = (& git -C $RepoRoot status --porcelain 2>&1 | Out-String).Trim()
    if ($porcelain -ne '') { $report.failures += 'working tree dirty (live git status --porcelain non-empty)' }
    else { $report.notes += 'working tree clean (live check)' }
  } catch { $report.failures += "git clean-tree check unavailable: $($_.ToString())" }
}

if ($Zip -ne '') {
  $z = (Resolve-Path -LiteralPath $Zip).Path
  $sha = (Get-FileHash -LiteralPath $z -Algorithm SHA256).Hash.ToLower()
  $report['zip'] = $z; $report['zip_sha256'] = $sha
  if ($ShaFile -ne '') {
    if (-not (Test-Path -LiteralPath $ShaFile)) { $report.failures += "sha256sums file not found: $ShaFile" }
    else {
      $sp = (Resolve-Path -LiteralPath $ShaFile).Path
      $report['sha_file'] = $sp
      $want = Get-ShaEntry $sp ([System.IO.Path]::GetFileName($z))
      if ($null -eq $want) { $report.failures += "sha256sums entry missing for $([System.IO.Path]::GetFileName($z))" }
      elseif ($want -ne $sha) { $report.failures += "zip sha256 mismatch: actual $sha != sha256sums $want" }
      else { $report.notes += 'zip sha256 matches sha256sums entry' }
    }
  } else {
    $report.notes += 'no -ShaFile given: sha pinning skipped (RC run must pass dist/SHA256SUMS)'
  }
  if ($Setup -ne '') {
    if (-not (Test-Path -LiteralPath $Setup)) { $report.failures += "setup file not found: $Setup" }
    else {
      $sx = (Resolve-Path -LiteralPath $Setup).Path
      $shax = (Get-FileHash -LiteralPath $sx -Algorithm SHA256).Hash.ToLower()
      $report['setup'] = $sx; $report['setup_sha256'] = $shax
      if ($ShaFile -eq '' -or -not (Test-Path -LiteralPath $ShaFile)) { $report.notes += 'no usable -ShaFile: setup sha pinning skipped' }
      else {
        $sp2 = (Resolve-Path -LiteralPath $ShaFile).Path
        $wantSx = Get-ShaEntry $sp2 ([System.IO.Path]::GetFileName($sx))
        if ($null -eq $wantSx) { $report.failures += "sha256sums entry missing for $([System.IO.Path]::GetFileName($sx))" }
        elseif ($wantSx -ne $shax) { $report.failures += "setup sha256 mismatch: actual $shax != sha256sums $wantSx" }
        else { $report.notes += 'setup sha256 matches sha256sums entry' }
      }
    }
  }
  Add-Type -AssemblyName System.IO.Compression
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
      try { $raw = $sr.ReadToEnd() } finally { $sr.Close() }
      try { $info = $raw | ConvertFrom-Json }
      catch { $report.failures += "build-info.json unparsable: $($_.ToString())"; $info = $null }
      if ($null -ne $info) {
      $report['build_info'] = @{ smoke_armed = $info.smoke_armed; git_dirty = $info.git_dirty; git_commit = $info.git_commit }
      foreach ($f in @('smoke_armed', 'git_dirty', 'git_commit')) {
        if ($null -eq $info.PSObject.Properties[$f]) { $report.failures += "build-info missing field: $f" }
      }
      if ($info.smoke_armed -ne $false) { $report.failures += 'smoke_armed != false' }
      if ($info.git_dirty -ne $false) { $report.failures += 'git_dirty != false' }
      if ($info.git_commit -ne $ExpectedCommit) { $report.failures += "git_commit $($info.git_commit) != expected $ExpectedCommit" }
      }
    }
  } finally { $fs.Close() }
} else {
  $report.notes += 'no -Zip given: package checks skipped (fixture-only run)'
}

if ($report.failures.Count -gt 0) { $report.status = 'fail' }
Out-Report $report
if ($report.status -eq 'fail') { exit 1 } else { exit 0 }
