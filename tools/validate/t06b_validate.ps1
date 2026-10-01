# T06b: validate codegen output with the real Xray and sing-box cores.
#
# For every case in target/t06b/matrix/manifest.json this runs the core's own
# config validator:
#   Xray     : xray run -test -config <cfg>
#   sing-box : sing-box check -c <cfg>
# and writes target/t06b/matrix/results.json plus a per-case log under
# target/t06b/matrix/logs/.
#
# This script never starts a listening core, never touches the system proxy and
# never references port 10808.

[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path,
    [string]$MatrixDir = 'target/t06b/matrix'
)

$ErrorActionPreference = 'Stop'
$XrayExe = Join-Path $RepoRoot 'tools/cores/xray/v26.3.27/xray.exe'
$SboxExe = Join-Path $RepoRoot 'tools/cores/singbox/v1.14.2/sing-box.exe'
$matrixPath = Join-Path $RepoRoot $MatrixDir
$manifestPath = Join-Path $matrixPath 'manifest.json'
$logDir = Join-Path $matrixPath 'logs'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null

if (-not (Test-Path -LiteralPath $manifestPath)) {
    throw "manifest not found: $manifestPath (run 'cargo run --example gen_matrix')"
}

$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$results = @()
$pass = 0
$fail = 0

foreach ($entry in $manifest) {
    if (-not $entry.generated) {
        $results += [pscustomobject]@{
            core = $entry.core; case = $entry.case; file = $null;
            status = 'GEN_ERROR'; exit = -1; error = $entry.error
        }
        $fail++
        continue
    }
    $file = $entry.file
    $log = Join-Path $logDir ("{0}--{1}.log" -f $entry.core, $entry.case)
    if ($entry.core -eq 'xray') {
        $output = & $XrayExe run -test -config $file 2>&1
        $exit = $LASTEXITCODE
    }
    else {
        $output = & $SboxExe check -c $file 2>&1
        $exit = $LASTEXITCODE
    }
    $output | Out-File -LiteralPath $log -Encoding utf8
    $status = if ($exit -eq 0) { 'PASS' } else { 'FAIL' }
    if ($exit -eq 0) { $pass++ } else { $fail++ }
    $text = ($output | Out-String).Trim()
    # Keep only the decisive tail line(s) for the report.
    $decisive = $text
    if ($text.Length -gt 600) { $decisive = $text.Substring($text.Length - 600) }
    $results += [pscustomobject]@{
        core = $entry.core; case = $entry.case; file = $file;
        status = $status; exit = $exit; error = $decisive
    }
    Write-Host ("[{0}] {1} {2} (exit {3})" -f $status, $entry.core, $entry.case, $exit)
}

$resultsPath = Join-Path $matrixPath 'results.json'
$results | ConvertTo-Json -Depth 6 | Out-File -LiteralPath $resultsPath -Encoding utf8
Write-Host ""
Write-Host ("T06b matrix: {0} pass, {1} fail  -> {2}" -f $pass, $fail, $resultsPath)
if ($fail -gt 0) { exit 1 } else { exit 0 }
