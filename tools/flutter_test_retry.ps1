<#
.SYNOPSIS
  Runs `flutter test` for apps/desktop with retries.

.DESCRIPTION
  The locked Flutter 3.47.5 build intermittently segfaults `flutter_tester`
  (exit -1073741819 / 0xC0000005) after one or more heavy widget builds. The
  crash is in the engine, has no Dart-side stack, and is documented in
  docs/evidence/T01.md. This wrapper reruns the suite until it completes
  cleanly; the raw `flutter test` invocation is still recorded per attempt.
#>
param(
  [int]$MaxAttempts = 10,
  [string]$AppDir = (Resolve-Path (Join-Path $PSScriptRoot '..\apps\desktop')).Path,
  [string]$LogDir = (Resolve-Path (Join-Path $PSScriptRoot '..\target')).Path
)

$ErrorActionPreference = 'Continue'
$flutter = 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat'
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

Push-Location $AppDir
try {
  for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
    $log = Join-Path $LogDir "flutter_test_attempt_$attempt.log"
    Write-Host "== flutter test attempt $attempt/$MaxAttempts =="
    & $flutter test 2>&1 | Tee-Object -FilePath $log
    if ($LASTEXITCODE -eq 0) {
      Write-Host "PASS on attempt $attempt (log: $log)"
      exit 0
    }
    Write-Host "attempt $attempt failed (exit $LASTEXITCODE)"
  }
  Write-Host "FAILED after $MaxAttempts attempts"
  exit 1
}
finally {
  Pop-Location
}
