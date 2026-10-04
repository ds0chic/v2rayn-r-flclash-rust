<#
.SYNOPSIS
  RR-04 release-packaging assertions (synthetic stage, no real build).

.DESCRIPTION
  Uses build_windows.ps1's -ReleaseDirOverride / -TargetReleaseOverride /
  -OutRootOverride to point packaging at a synthetic stage, then asserts:

    1. the dist stage contains v2rayN-upgrade.exe (the runner the updater
       launches from the flat install root);
    2. v2rayn-r.iss [UninstallDelete] never recursively deletes the whole {app}
       directory (no `filesandordirs` on bare `{app}` or `{app}\*`), while the
       managed `.staging` / `app.previous` leftovers and the empty-dir removal
       are still present.

  Never builds cargo/flutter, never touches 127.0.0.1:10808 or the host proxy.
#>
[CmdletBinding()]
param(
  [string]$WorkRoot = ''
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if ($WorkRoot -eq '') {
  $WorkRoot = Join-Path $env:TEMP ("v2rayn-rr04-" + [Guid]::NewGuid().ToString('N').Substring(0, 8))
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

$stage = Get-ChildItem -LiteralPath $OutRoot -Directory | Where-Object { $_.Name -like 'v2rayN-R-*-windows-x64' } | Select-Object -First 1
if (-not $stage) { throw 'no dist stage directory produced' }

$failures = New-Object System.Collections.Generic.List[string]

# 1. runner ships under the exact name the updater launches.
$runner = Join-Path $stage.FullName 'v2rayN-upgrade.exe'
if (-not (Test-Path -LiteralPath $runner)) {
  $failures.Add("stage missing v2rayN-upgrade.exe: $runner")
} else {
  Write-Host "PASS stage contains v2rayN-upgrade.exe"
}
if (-not (Test-Path -LiteralPath (Join-Path $stage.FullName 'v2rayn_desktop.exe'))) {
  $failures.Add('stage missing v2rayn_desktop.exe')
}

# 2. Inno uninstall never recursively deletes the whole install root.
$issPath = Join-Path $PSScriptRoot 'v2rayn-r.iss'
$iss = Get-Content -LiteralPath $issPath
$section = $null
$uninstallLines = New-Object System.Collections.Generic.List[string]
foreach ($line in $iss) {
  $trimmed = $line.Trim()
  if ($trimmed -match '^\[(.+)\]$') {
    $section = $Matches[1]
    continue
  }
  if ($section -eq 'UninstallDelete' -and $trimmed -ne '' -and -not $trimmed.StartsWith(';')) {
    $uninstallLines.Add($trimmed)
  }
}
if ($uninstallLines.Count -eq 0) {
  $failures.Add('[UninstallDelete] section is empty or missing')
}
$hasEmptyDir = $false
foreach ($line in $uninstallLines) {
  $isRecursiveAppRoot =
    ($line -match 'filesandordirs') -and
    ($line -match 'Name:\s*"\s*\{app\}\s*\*?\s*"')
  if ($isRecursiveAppRoot) {
    $failures.Add("recursive whole-dir uninstall: $line")
  }
  if (($line -match 'dirifempty') -and ($line -match '\{app\}')) {
    $hasEmptyDir = $true
  }
}
if (-not $hasEmptyDir) {
  $failures.Add('[UninstallDelete] missing dirifempty on {app}')
}
foreach ($needle in @('{app}\.staging', '{app}\app.previous')) {
  if (-not ($uninstallLines | Where-Object { $_ -like "*$needle*" })) {
    $failures.Add("[UninstallDelete] missing managed leftover rule for $needle")
  }
}
Write-Host ("PASS [UninstallDelete] rules: " + ($uninstallLines -join ' | '))

if ($failures.Count -gt 0) {
  $failures | ForEach-Object { Write-Host "FAIL $_" }
  Write-Host "RR04_RELEASE_ASSERTS ok=false failures=$($failures.Count) stage=$($stage.FullName)"
  exit 1
}
Write-Host "RR04_RELEASE_ASSERTS ok=true stage=$($stage.FullName)"
exit 0
