<#
.SYNOPSIS
  R4-29 release-packaging assertions (synthetic stage, no real build).

.DESCRIPTION
  Uses build_windows.ps1's -ReleaseDirOverride / -TargetReleaseOverride /
  -OutRootOverride to point packaging at a synthetic stage, then asserts the
  release package's build-info / version / naming are mutually consistent:

    1. the stage/zip name is derived from the pubspec version
       (`v2rayN-R-<pubspec>-windows-x64`);
    2. build-info.json exists both under the output root and inside the stage,
       and both agree with the pubspec version;
    3. the self-update runner ships under the exact name the updater launches
       (`updater::DEFAULT_RUNNER_NAME == "v2rayN-upgrade.exe"`).

  Never builds cargo/flutter; never touches 127.0.0.1:10808 or the host proxy.
#>
[CmdletBinding()]
param(
  [string]$WorkRoot = ''
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if ($WorkRoot -eq '') {
  $WorkRoot = Join-Path $env:TEMP ("v2rayn-r429-" + [Guid]::NewGuid().ToString('N').Substring(0, 8))
}
$ReleaseDir = Join-Path $WorkRoot 'release'
$TargetRelease = Join-Path $WorkRoot 'target-release'
$OutRoot = Join-Path $WorkRoot 'dist'
New-Item -ItemType Directory -Path $ReleaseDir, $TargetRelease, $OutRoot -Force | Out-Null

# Synthetic inputs satisfying build_windows.ps1's required-artifact checks.
Set-Content -LiteralPath (Join-Path $ReleaseDir 'v2rayn_desktop.exe') -Value 'stub' -Encoding ASCII
Set-Content -LiteralPath (Join-Path $ReleaseDir 'bridge_api.dll') -Value 'stub' -Encoding ASCII
Set-Content -LiteralPath (Join-Path $TargetRelease 'net_host.exe') -Value 'stub' -Encoding ASCII
Set-Content -LiteralPath (Join-Path $TargetRelease 'privileged_helper.exe') -Value 'stub' -Encoding ASCII
Set-Content -LiteralPath (Join-Path $TargetRelease 'upgrade_runner.exe') -Value 'runner-stub' -Encoding ASCII

$buildScript = Join-Path $PSScriptRoot 'build_windows.ps1'
& $buildScript -SkipBuild -SkipFlutter `
  -ReleaseDirOverride $ReleaseDir `
  -TargetReleaseOverride $TargetRelease `
  -OutRootOverride $OutRoot | Out-Null

$failures = New-Object System.Collections.Generic.List[string]

$pubspecLine = Select-String -Path (Join-Path $RepoRoot 'apps\desktop\pubspec.yaml') -Pattern '^version:\s*(\S+)' |
  Select-Object -First 1
if (-not $pubspecLine) { throw 'pubspec version not found' }
$pubspec = $pubspecLine.Matches[0].Groups[1].Value

$stage = Get-ChildItem -LiteralPath $OutRoot -Directory |
  Where-Object { $_.Name -like 'v2rayN-R-*-windows-x64' } | Select-Object -First 1
if (-not $stage) { throw 'no dist stage directory produced' }

# 1. stage name derives from the pubspec version.
$expectedStageName = "v2rayN-R-$pubspec-windows-x64"
if ($stage.Name -ne $expectedStageName) {
  $failures.Add("stage name '$($stage.Name)' != expected '$expectedStageName'")
} else {
  Write-Host "PASS stage name $($stage.Name)"
}
if (-not (Test-Path -LiteralPath (Join-Path $OutRoot "$expectedStageName.zip"))) {
  $failures.Add("zip not named from version: $expectedStageName.zip")
}

# 2. build-info present and version-consistent in both locations.
$rootInfoPath = Join-Path $OutRoot 'build-info.json'
$stageInfoPath = Join-Path $stage.FullName 'build-info.json'
foreach ($p in @($rootInfoPath, $stageInfoPath)) {
  if (-not (Test-Path -LiteralPath $p)) { $failures.Add("missing build-info.json: $p") }
}
if ((Test-Path -LiteralPath $rootInfoPath) -and (Test-Path -LiteralPath $stageInfoPath)) {
  $rootInfo = Get-Content -LiteralPath $rootInfoPath -Raw | ConvertFrom-Json
  $stageInfo = Get-Content -LiteralPath $stageInfoPath -Raw | ConvertFrom-Json
  if ($rootInfo.pubspec_version -ne $pubspec) {
    $failures.Add("root build-info pubspec_version '$($rootInfo.pubspec_version)' != '$pubspec'")
  }
  if ($stageInfo.pubspec_version -ne $pubspec) {
    $failures.Add("stage build-info pubspec_version '$($stageInfo.pubspec_version)' != '$pubspec'")
  }
  if ([string]::IsNullOrWhiteSpace($stageInfo.version)) {
    $failures.Add('build-info.version is empty')
  }
  if ($rootInfo.version -ne $stageInfo.version) {
    $failures.Add("build-info version mismatch root='$($rootInfo.version)' stage='$($stageInfo.version)'")
  }
  if ($stageInfo.product -ne 'v2rayN-R') {
    $failures.Add("build-info.product '$($stageInfo.product)' != 'v2rayN-R'")
  }
  if ($stageInfo.target_platform -ne 'windows-x64') {
    $failures.Add("build-info.target_platform '$($stageInfo.target_platform)' != 'windows-x64'")
  }
  Write-Host "PASS build-info version=$($stageInfo.version) pubspec=$($stageInfo.pubspec_version)"
}

# 3. runner name matches the frozen updater constant; both names ship flat.
$updaterSrc = Get-Content -LiteralPath (Join-Path $RepoRoot 'crates\updater\src\app_upgrade.rs') -Raw
if ($updaterSrc -notmatch 'DEFAULT_RUNNER_NAME:\s*&str\s*=\s*"v2rayN-upgrade\.exe"') {
  $failures.Add('DEFAULT_RUNNER_NAME is not "v2rayN-upgrade.exe"')
}
foreach ($exe in @('v2rayN-upgrade.exe', 'v2rayn_desktop.exe')) {
  if (-not (Test-Path -LiteralPath (Join-Path $stage.FullName $exe))) {
    $failures.Add("stage missing flat exe: $exe")
  }
}
Write-Host "PASS flat exes: v2rayn_desktop.exe, v2rayN-upgrade.exe"

if ($failures.Count -gt 0) {
  $failures | ForEach-Object { Write-Host "FAIL $_" }
  Write-Host "R4_29_RELEASE_ASSERTS ok=false failures=$($failures.Count) stage=$($stage.FullName)"
  exit 1
}
Write-Host "R4_29_RELEASE_ASSERTS ok=true stage=$($stage.FullName)"
exit 0
