<#
.SYNOPSIS
  Build and package the v2rayN-R Windows x64 release candidate (portable zip).

.DESCRIPTION
  1. cargo build --workspace --release --locked
  2. flutter build windows --release
  3. Assemble dist/v2rayN-R-<version>-windows-x64/ from the Flutter Release
     directory plus our own net_host.exe / privileged_helper.exe.
  4. Emit dist/build-info.json, dist/v2rayN-R-<version>-windows-x64.zip and
     dist/SHA256SUMS.

  External proxy cores (xray / sing-box) are NOT bundled; they are downloaded
  at runtime and pinned by tools/cores/cores.lock.json.

  Re-runnable: the destination directory and zip are removed and rebuilt.

  -SmokeArmed builds the evidence-only variant used by the T20/F-02/F-03
  packaged smoke: `flutter build windows --release --dart-define=
  V2RAYN_R_SMOKE_ARMED=true`. The default (no switch) is the official release
  package and never sets the arming define. Armed artifacts go to
  dist/evidence-armed/ and are never mixed with the official dist/ package.

.NOTES
  Do not touch 127.0.0.1:10808 or the host system proxy. This script only
  builds and copies files.
#>
[CmdletBinding()]
param(
  [string]$Version = '',
  [switch]$SkipBuild,
  [switch]$SkipFlutter,
  [switch]$SmokeArmed,
  [switch]$KeepStage
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$AppDir = Join-Path $RepoRoot 'apps\desktop'
$ReleaseDir = Join-Path $AppDir 'build\windows\x64\runner\Release'
$TargetRelease = Join-Path $RepoRoot 'target\release'
$DistDir = Join-Path $RepoRoot 'dist'
# Armed evidence builds are isolated so they can never overwrite the official
# release package, build-info.json or SHA256SUMS under dist/.
$OutRoot = if ($SmokeArmed) { Join-Path $DistDir 'evidence-armed' } else { $DistDir }

function Invoke-Step {
  param([string]$Label, [scriptblock]$Body)
  Write-Host "==> $Label"
  & $Body
  if ($LASTEXITCODE -ne 0) {
    throw "$Label failed with exit code $LASTEXITCODE"
  }
}

function Get-PubspecVersion {
  $line = Select-String -Path (Join-Path $AppDir 'pubspec.yaml') -Pattern '^version:\s*(\S+)' |
    Select-Object -First 1
  if (-not $line) { throw 'version not found in apps/desktop/pubspec.yaml' }
  return $line.Matches[0].Groups[1].Value
}

function Get-RustWorkspaceVersion {
  $line = Select-String -Path (Join-Path $RepoRoot 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1
  if (-not $line) { throw 'workspace version not found in Cargo.toml' }
  return $line.Matches[0].Groups[1].Value
}

function Get-ToolVersion {
  param([string]$Exe, [string[]]$ExeArgs, [string]$Pattern = '')
  try {
    $out = (& $Exe @ExeArgs 2>&1 | Out-String)
    $lines = $out -split "`r?`n" | Where-Object { $_.Trim() -ne '' }
    if ($Pattern -ne '') {
      $match = $lines | Where-Object { $_ -match $Pattern } | Select-Object -First 1
      if ($match) { return $match.Trim() }
    }
    return ($lines | Select-Object -First 1).Trim()
  } catch {
    return "unavailable"
  }
}

if ($Version -eq '') { $Version = Get-PubspecVersion }
$RustVersion = Get-RustWorkspaceVersion

$buildName = $Version
$buildNumber = ''
if ($Version -match '^(.+)\+(.+)$') {
  $buildName = $Matches[1]
  $buildNumber = $Matches[2]
}

$pkgName = "v2rayN-R-$Version-windows-x64"
$stageDir = Join-Path $OutRoot $pkgName
$zipPath = Join-Path $OutRoot "$pkgName.zip"

Push-Location $RepoRoot
$sw = [Diagnostics.Stopwatch]::StartNew()
try {
  if (-not $SkipBuild) {
    Invoke-Step 'cargo build --workspace --release --locked' {
      & cargo build --workspace --release --locked
    }
  }
  if (-not $SkipFlutter) {
    $flutterArgs = @('build', 'windows', '--release')
    if ($SmokeArmed) { $flutterArgs += '--dart-define=V2RAYN_R_SMOKE_ARMED=true' }
    Invoke-Step "flutter $($flutterArgs -join ' ')" {
      Push-Location $AppDir
      try { & flutter @flutterArgs } finally { Pop-Location }
    }
  }

  foreach ($required in @(
      (Join-Path $ReleaseDir 'v2rayn_desktop.exe'),
      (Join-Path $ReleaseDir 'bridge_api.dll'),
      (Join-Path $TargetRelease 'net_host.exe'),
      (Join-Path $TargetRelease 'privileged_helper.exe')
    )) {
    if (-not (Test-Path -LiteralPath $required)) {
      throw "missing build artifact: $required"
    }
  }

  if (Test-Path -LiteralPath $stageDir) { Remove-Item -LiteralPath $stageDir -Recurse -Force }
  New-Item -ItemType Directory -Path $stageDir -Force | Out-Null

  Get-ChildItem -LiteralPath $ReleaseDir -Recurse -File | ForEach-Object {
    $rel = $_.FullName.Substring($ReleaseDir.Length).TrimStart('\')
    if ($rel -eq 'ui_state.json') { return }  # runtime UI-state draft, never ship
    $dest = Join-Path $stageDir $rel
    $destDir = Split-Path -Parent $dest
    if (-not (Test-Path -LiteralPath $destDir)) { New-Item -ItemType Directory -Path $destDir -Force | Out-Null }
    Copy-Item -LiteralPath $_.FullName -Destination $dest -Force
  }

  Copy-Item -LiteralPath (Join-Path $TargetRelease 'net_host.exe') -Destination $stageDir -Force
  Copy-Item -LiteralPath (Join-Path $TargetRelease 'privileged_helper.exe') -Destination $stageDir -Force

  foreach ($doc in @('LICENSE', 'NOTICE.md', 'README.md')) {
    $src = Join-Path $RepoRoot $doc
    if (Test-Path -LiteralPath $src) { Copy-Item -LiteralPath $src -Destination $stageDir -Force }
  }

  $gitCommit = (git rev-parse HEAD 2>$null)
  $gitShort = (git rev-parse --short HEAD 2>$null)
  $gitBranch = (git rev-parse --abbrev-ref HEAD 2>$null)
  $gitDirty = ((git status --porcelain 2>$null) | Measure-Object -Line).Lines -gt 0

  $buildTime = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  $buildInfo = [ordered]@{
    schema         = 1
    product        = 'v2rayN-R'
    kind           = 'derived-refactor'
    version        = $buildName
    build_number   = $buildNumber
    pubspec_version = $Version
    rust_version   = $RustVersion
    git_commit     = ($gitCommit | Out-String).Trim()
    git_commit_short = ($gitShort | Out-String).Trim()
    git_branch     = ($gitBranch | Out-String).Trim()
    git_dirty      = $gitDirty
    target_platform = 'windows-x64'
    target_triple  = 'x86_64-pc-windows-msvc'
    build_time_utc = $buildTime
    smoke_armed    = [bool]$SmokeArmed
    toolchain      = [ordered]@{
      flutter   = (Get-Content (Join-Path $AppDir '.dart_tool\version') -ErrorAction SilentlyContinue)
      dart      = Get-ToolVersion 'dart' @('--version') -Pattern 'Dart'
      rustc     = Get-ToolVersion 'rustc' @('--version') -Pattern '^rustc'
      cargo     = Get-ToolVersion 'cargo' @('--version') -Pattern '^cargo'
      frb       = '2.13.0'
    }
    cores_policy   = 'not_bundled; downloaded at runtime, pinned by tools/cores/cores.lock.json'
  }
  $buildInfoJson = $buildInfo | ConvertTo-Json -Depth 5
  if (-not (Test-Path -LiteralPath $OutRoot)) { New-Item -ItemType Directory -Path $OutRoot -Force | Out-Null }
  Set-Content -LiteralPath (Join-Path $OutRoot 'build-info.json') -Value $buildInfoJson -Encoding UTF8
  Set-Content -LiteralPath (Join-Path $stageDir 'build-info.json') -Value $buildInfoJson -Encoding UTF8

  $coreNote = @"
v2rayN-R does not bundle third-party proxy cores (Xray-core, sing-box).
They are downloaded at runtime by the in-app updater and pinned by
tools/cores/cores.lock.json in the source repository. Point the app at an
existing core directory with V2RAYN_R_CORES_ROOT / V2RAYN_R_XRAY_BIN when
testing offline. Xray-core and sing-box remain under their own licenses.
"@
  Set-Content -LiteralPath (Join-Path $stageDir 'CORE-NOTES.txt') -Value $coreNote -Encoding UTF8

  if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
  Compress-Archive -LiteralPath $stageDir -DestinationPath $zipPath -CompressionLevel Optimal -Force

  $hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
  Set-Content -LiteralPath (Join-Path $OutRoot 'SHA256SUMS') -Value "$hash  $(Split-Path -Leaf $zipPath)" -Encoding ASCII

  $sw.Stop()
  Write-Host ""
  Write-Host "PACKAGE  $pkgName"
  Write-Host "STAGE    $stageDir"
  Write-Host "ZIP      $zipPath"
  Write-Host "SHA256   $hash"
  Write-Host "ARMED    $([bool]$SmokeArmed)"
  Write-Host "ELAPSED  $([math]::Round($sw.Elapsed.TotalSeconds,1))s"
  Write-Host "T20_BUILD_OK version=$Version armed=$([bool]$SmokeArmed) zip=$zipPath sha256=$hash"
}
finally {
  Pop-Location
}
