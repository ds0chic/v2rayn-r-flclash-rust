<#
.SYNOPSIS
  SP-29 reproducible gate: locked-toolchain checks with no silent incompletes.

.DESCRIPTION
  Runs the AGENTS publication checks through one entry point and writes a
  machine-readable result JSON (pass/fail/skip/incomplete per file plus
  toolchain lock). Rules:

  * Flutter tests run per file in isolated processes (recursive inventory of
    apps/desktop/test/**/*_test.dart, so nested dirs such as test/repair are
    never silently skipped).
  * A native tester crash (exit 79, 0xC0000005, "did not complete",
    "No tests were found") classifies as `incomplete` and is retried up to
    -FileMaxAttempts until a stable conclusion (completed pass/fail).
    Incomplete is never reported as pass or fail, and a missing result entry
    fails the gate instead of staying silent.
  * Toolchain versions (flutter/dart/cargo/rustc paths + versions, git HEAD,
    FRB versions) are recorded in the output JSON.
  * Exit code: 0 = gate green (0 fail, 0 incomplete, stages green);
    1 = assertion/static failure; 2 = incomplete remains after retries.

  The gate only spawns and waits for its own child test processes; it never
  kills processes by name and never binds test ports itself (Flutter unit
  tests and Rust loopback tests use in-process transports only).

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/gates/run_all.ps1
.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/gates/run_all.ps1 `
    -FlutterFilter 'sp_29' -SkipStatic -SkipCargo
#>
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
  [string]$EvidenceDir = '',
  [int]$FileMaxAttempts = 3,
  [switch]$SkipStatic,
  [switch]$SkipCargo,
  [switch]$SkipFlutter,
  [string]$FlutterFilter = '',
  [string]$CompareWith = ''
)

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$env:LC_ALL = 'C.UTF-8'

$flutterBin = 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat'
$dartBin = 'C:\Users\Colby\toolchains\flutter\bin\dart.bat'
$cargoBin = 'C:\Users\Colby\.cargo\bin\cargo.exe'
$appDir = Join-Path $RepoRoot 'apps\desktop'
$testRoot = Join-Path $appDir 'test'

if ([string]::IsNullOrEmpty($EvidenceDir)) {
  $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\stable-port\SP-29'
}
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$runDir = Join-Path $EvidenceDir "runs\$stamp"
New-Item -ItemType Directory -Force -Path $runDir | Out-Null
$resultPath = Join-Path $runDir 'gate-result.json'

function Invoke-Step {
  param([string]$Name, [string]$LogFile, [scriptblock]$Body)
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  & $Body > $LogFile 2>&1
  $code = $LASTEXITCODE
  $sw.Stop()
  return @{ name = $Name; exit_code = $code; seconds = [math]::Round($sw.Elapsed.TotalSeconds, 3); log = $LogFile }
}

function Get-AttemptClass {
  param([int]$ExitCode, [string]$Output)
  $markers = @('did not complete', 'No tests were found', 'EXCEPTION_ACCESS_VIOLATION', 'Segmentation fault', 'Connection closed before test suite loaded', 'Failed to load "')
  foreach ($m in $markers) {
    if ($Output.Contains($m)) { return 'incomplete' }
  }
  if ($ExitCode -eq 79 -or $ExitCode -eq -1073741819) { return 'incomplete' }
  if ($ExitCode -eq 0) {
    $done = [regex]::Match($Output, '\+(\d+)( ~\d+)?: All tests passed!')
    if ($done.Success -and $done.Groups[1].Value -eq '0') { return 'skip' }
    return 'pass'
  }
  return 'fail'
}

# --- toolchain lock ---
$toolchain = @{}
try { $toolchain['flutter_path'] = (Get-Command $flutterBin -ErrorAction Stop).Source } catch { $toolchain['flutter_path'] = $flutterBin }
try { $toolchain['flutter_version'] = ((& $flutterBin --version 2>&1 | Out-String) -split "`r?`n" | Select-Object -First 4) -join "`n" } catch { $toolchain['flutter_version'] = 'unknown' }
try { $toolchain['dart_version'] = ((& $dartBin --version 2>&1 | Out-String).Trim()) } catch { $toolchain['dart_version'] = 'unknown' }
try { $toolchain['cargo_path'] = (Get-Command $cargoBin -ErrorAction Stop).Source } catch { $toolchain['cargo_path'] = $cargoBin }
try { $toolchain['cargo_version'] = ((& $cargoBin --version 2>&1 | Out-String).Trim()) } catch { $toolchain['cargo_version'] = 'unknown' }
try { $toolchain['rustc_version'] = ((& rustc --version 2>&1 | Out-String).Trim()) } catch { $toolchain['rustc_version'] = 'unknown' }
Push-Location $RepoRoot
try { $toolchain['git_head'] = ((git rev-parse HEAD 2>&1 | Out-String).Trim()) } catch { $toolchain['git_head'] = 'unknown' }
Pop-Location
try {
  $pub = Get-Content (Join-Path $appDir 'pubspec.yaml') -Raw
  $m = [regex]::Match($pub, 'flutter_rust_bridge:\s*([^\s]+)')
  $toolchain['frb_dart'] = if ($m.Success) { $m.Groups[1].Value } else { 'unknown' }
} catch { $toolchain['frb_dart'] = 'unknown' }
try {
  $toml = Get-Content (Join-Path $RepoRoot 'Cargo.toml') -Raw
  $m = [regex]::Match($toml, 'flutter_rust_bridge\s*=\s*"=?([^"]+)"')
  $toolchain['frb_rust'] = if ($m.Success) { $m.Groups[1].Value } else { 'unknown' }
} catch { $toolchain['frb_rust'] = 'unknown' }
try { $toolchain['frb_codegen'] = ((flutter_rust_bridge_codegen --version 2>&1 | Out-String).Trim()) } catch { $toolchain['frb_codegen'] = 'not-on-path' }

$stages = @()
$gateFail = $false

# --- static stages ---
if (-not $SkipStatic) {
  Push-Location $RepoRoot
  try {
    $stages += Invoke-Step 'cargo-fmt' (Join-Path $runDir 'cargo-fmt.log') { & $cargoBin fmt --all -- --check }
    $stages += Invoke-Step 'cargo-clippy' (Join-Path $runDir 'cargo-clippy.log') { & $cargoBin clippy --workspace --all-targets --locked -- -D warnings }
  } finally { Pop-Location }
  Push-Location $appDir
  try {
    $stages += Invoke-Step 'dart-format' (Join-Path $runDir 'dart-format.log') { & $dartBin format --output=none --set-exit-if-changed lib test }
    $stages += Invoke-Step 'flutter-analyze' (Join-Path $runDir 'flutter-analyze.log') { & $flutterBin analyze --no-pub }
  } finally { Pop-Location }
  foreach ($s in $stages) { if ($s.exit_code -ne 0) { $gateFail = $true } }
}

# --- cargo tests ---
$cargoSummary = @{ passed = 0; failed = 0; ignored = 0; suites = 0 }
if (-not $SkipCargo) {
  Push-Location $RepoRoot
  try {
    $s = Invoke-Step 'cargo-test' (Join-Path $runDir 'cargo-test.log') { & $cargoBin test --workspace --locked }
    $stages += $s
    if ($s.exit_code -ne 0) { $gateFail = $true }
    foreach ($line in (Get-Content $s.log)) {
      $clean = $line -replace '\x1b\[[0-9;]*m', ''
      $m = [regex]::Match($clean, 'test result:\s*(\w+)\.\s*(\d+) passed;\s*(\d+) failed;\s*(\d+) ignored')
      if ($m.Success) {
        $cargoSummary.suites++
        $cargoSummary.passed += [int]$m.Groups[2].Value
        $cargoSummary.failed += [int]$m.Groups[3].Value
        $cargoSummary.ignored += [int]$m.Groups[4].Value
        if ($m.Groups[1].Value -ne 'ok') { $gateFail = $true }
      }
    }
  } finally { Pop-Location }
}

# --- flutter per-file stage ---
$fileResults = @()
$inventory = @()
if (-not $SkipFlutter) {
  $all = Get-ChildItem -Path $testRoot -Recurse -Filter '*_test.dart' | Sort-Object FullName
  foreach ($f in $all) {
    $rel = ('test/' + $f.FullName.Substring($testRoot.Length + 1)) -replace '\\', '/'
    $inventory += $rel
  }
  if ($FlutterFilter -ne '') {
    $tokens = @($FlutterFilter -split '[,;]' | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' })
    $inventory = @($inventory | Where-Object { $p = $_; @($tokens | Where-Object { $p -like "*$_*" }).Count -gt 0 })
  }
  Push-Location $appDir
  try {
    $idx = 0
    foreach ($rel in $inventory) {
      $idx++
      $safe = ($rel -replace '[/\\]', '_')
      $attempts = @()
      for ($a = 1; $a -le $FileMaxAttempts; $a++) {
        $log = Join-Path $runDir "flutter_${safe}_attempt_${a}.log"
        Write-Host "[$idx/$($inventory.Count)] flutter test $rel (attempt $a/$FileMaxAttempts)"
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        # SilentlyContinue only suppresses the NativeCommandError wrapper
        # record; real stderr text is still merged into the log via 2>&1.
        $prevPref = $ErrorActionPreference
        $ErrorActionPreference = 'SilentlyContinue'
        $out = (& $flutterBin test --no-pub $rel --reporter expanded 2>&1 | Out-String)
        $code = $LASTEXITCODE
        $ErrorActionPreference = $prevPref
        $sw.Stop()
        Set-Content -LiteralPath $log -Value $out -Encoding utf8
        $class = Get-AttemptClass -ExitCode $code -Output $out
        $attempts += @{ attempt = $a; exit_code = $code; class = $class; seconds = [math]::Round($sw.Elapsed.TotalSeconds, 3); log = $log }
        if ($class -eq 'pass' -or $class -eq 'skip' -or $class -eq 'fail') { break }
      }
      # Stable-conclusion rule: retries stop at first completed pass/skip;
      # if attempts are exhausted, any completed assertion failure wins (fail),
      # otherwise the file stays explicitly incomplete.
      $fails = @($attempts | Where-Object { $_.class -eq 'fail' })
      $passes = @($attempts | Where-Object { $_.class -eq 'pass' -or $_.class -eq 'skip' })
      if ($passes.Count -gt 0) { $final = $passes[0].class; $reason = "completed on attempt $($passes[0].attempt)" }
      elseif ($fails.Count -gt 0) { $final = 'fail'; $reason = 'assertion failure (stable across retries)' }
      else { $final = 'incomplete'; $reason = "still incomplete after $($attempts.Count) attempts" }
      $fileResults += @{ path = $rel; status = $final; reason = $reason; attempts = $attempts }
    }
  } finally { Pop-Location }
  # No silent incompletes: result count must equal inventory count.
  if ($fileResults.Count -ne $inventory.Count) { $gateFail = $true }
}

$pass = @($fileResults | Where-Object { $_.status -eq 'pass' }).Count
$fail = @($fileResults | Where-Object { $_.status -eq 'fail' }).Count
$skip = @($fileResults | Where-Object { $_.status -eq 'skip' }).Count
$incomplete = @($fileResults | Where-Object { $_.status -eq 'incomplete' }).Count

$repro = $null
if ($CompareWith -ne '') {
  try {
    $prior = Get-Content $CompareWith -Raw | ConvertFrom-Json
    $priorMap = @{}
    foreach ($r in $prior.files) { $priorMap[$r.path] = $r.status }
    $mism = @()
    foreach ($r in $fileResults) {
      if (-not $priorMap.ContainsKey($r.path)) { $mism += "new file: $($r.path)" }
      elseif ($priorMap[$r.path] -ne $r.status) { $mism += "$($r.path): $($priorMap[$r.path]) -> $($r.status)" }
    }
    foreach ($p in $prior.files) {
      if (@($fileResults | Where-Object { $_.path -eq $p.path }).Count -eq 0) { $mism += "missing file: $($p.path)" }
    }
    $repro = @{ compared_with = $CompareWith; inventory_match = ($inventory.Count -eq $prior.inventory.count); status_match = ($mism.Count -eq 0); mismatches = $mism }
  } catch {
    $repro = @{ compared_with = $CompareWith; error = $_.ToString() }
  }
}

$result = [ordered]@{
  schema_version = 1
  gate = 'SP-29'
  started_at = (Get-Date).ToString('o')
  toolchain = $toolchain
  inventory = @{ count = $inventory.Count; files = $inventory }
  stages = $stages
  cargo_summary = $cargoSummary
  files = $fileResults
  summary = @{ total = $fileResults.Count; pass = $pass; fail = $fail; skip = $skip; incomplete = $incomplete }
  reproducibility = $repro
  verdict = 'pass'
}

$exitCode = 0
if ($fail -gt 0 -or $gateFail) { $result.verdict = 'fail'; $exitCode = 1 }
if ($incomplete -gt 0 -and $exitCode -eq 0) { $result.verdict = 'incomplete'; $exitCode = 2 }
if ($incomplete -gt 0 -and $exitCode -eq 1) { $result.verdict = 'fail+incomplete' }

($result | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $resultPath -Encoding utf8
Write-Host "SP-29 gate: pass=$pass fail=$fail skip=$skip incomplete=$incomplete verdict=$($result.verdict)"
Write-Host "result: $resultPath"
exit $exitCode
