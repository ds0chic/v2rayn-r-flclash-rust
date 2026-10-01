<#
.SYNOPSIS
  Runs `flutter test` for apps/desktop with retries and a per-file mode.

.DESCRIPTION
  The locked Flutter 3.47.5 build intermittently segfaults `flutter_tester`
  (exit -1073741819 / 0xC0000005) after enough heavy widget builds in one
  process. The crash is in the engine, has no Dart-side stack, and is
  documented in docs/evidence/T01.md. Two modes are provided:

  * default        : reruns the whole suite until it completes cleanly. As the
                     suite grows the crash becomes more likely within a single
                     attempt.
  * -PerFile       : runs every `*_test.dart` in its own `flutter test` process
                     with per-file retries. This is the deterministic method the
                     T12a/T13 evidence uses ("逐文件复现"); each file passes
                     independently and a crash can no longer take the whole run
                     down with it.

  The raw `flutter test <args>` invocation is recorded per attempt.
#>
param(
  [int]$MaxAttempts = 10,
  [switch]$PerFile,
  [string]$AppDir = (Resolve-Path (Join-Path $PSScriptRoot '..\apps\desktop')).Path,
  [string]$LogDir = (Resolve-Path (Join-Path $PSScriptRoot '..\target')).Path
)

$ErrorActionPreference = 'Continue'
$flutter = 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat'
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

Push-Location $AppDir
try {
  if ($PerFile) {
    $files = Get-ChildItem test -Filter '*_test.dart' | ForEach-Object { $_.Name }
    $failed = @()
    foreach ($file in $files) {
      $passed = $false
      for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        $log = Join-Path $LogDir "flutter_test_file_${file}_attempt_$attempt.log"
        Write-Host "== flutter test $file attempt $attempt/$MaxAttempts =="
        & $flutter test "test/$file" 2>&1 | Tee-Object -FilePath $log
        if ($LASTEXITCODE -eq 0) {
          Write-Host "PASS $file (attempt $attempt)"
          $passed = $true
          break
        }
        Write-Host "attempt $attempt failed for $file (exit $LASTEXITCODE)"
      }
      if (-not $passed) {
        Write-Host "FAILED $file after $MaxAttempts attempts"
        $failed += $file
      }
    }
    if ($failed.Count -gt 0) {
      Write-Host "PER-FILE FAILURES ($($failed.Count)): $($failed -join ', ')"
      exit 1
    }
    Write-Host "PER-FILE PASS: all test files green"
    exit 0
  }

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
