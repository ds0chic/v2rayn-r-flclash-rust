<#
.SYNOPSIS
  Release-GUI T18 benchmark runner (armed evidence build only).

.DESCRIPTION
  Extracts the armed evidence zip, launches the packaged exe with the T18
  benchmark environment (V2RAYN_R_T18_BENCH=1 + scenario/rows/steps/dir), waits
  for the marker JSON the app writes, records the wall-clock time, stops only
  the PID tree it started, and stores a summary under the evidence dir.

  The app writes every number itself (first-frame timestamp, scroll frame
  stats); this script only measures process-launch to marker as an upper bound.
  It never touches 127.0.0.1:10808, the system proxy or TUN.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/perf/run_t18_release.ps1 -Zip dist/evidence-armed/v2rayN-R-1.0.0+1-windows-x64.zip -Scenario startup
#>
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Zip,
  [string]$Scenario = 'startup',
  [int]$Rows = 10000,
  [int]$Steps = 600,
  [string]$EvidenceDir = '',
  [string]$Tag = '',
  [int]$TimeoutSec = 180
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($EvidenceDir -eq '') {
  $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\stable-port\SP-31\gui'
}
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
$Zip = (Resolve-Path -LiteralPath $Zip).Path

$work = Join-Path $env:TEMP ('sp31_gui_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $work
$pkg = (Get-ChildItem -Path $work -Directory | Select-Object -First 1).FullName
$exe = Join-Path $pkg 'v2rayn_desktop.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "packaged exe missing: $exe" }
$data = Join-Path $work 'data'
$outDir = Join-Path $work 'out'
New-Item -ItemType Directory -Path $data -Force | Out-Null
New-Item -ItemType Directory -Path $outDir -Force | Out-Null

$env:V2RAYN_R_DATA_DIR = $data
$env:V2RAYN_R_T18_BENCH = '1'
$env:V2RAYN_R_T18_SCENARIO = $Scenario
$env:V2RAYN_R_T18_ROWS = "$Rows"
$env:V2RAYN_R_T18_STEPS = "$Steps"
$env:V2RAYN_R_T18_DIR = $outDir
if ($Tag -ne '') { $env:V2RAYN_R_T18_TAG = $Tag }

$marker = if ($Scenario -eq 'startup') { 'startup' } else { "scroll_$Rows" }
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $exe -PassThru
$deadline = (Get-Date).AddSeconds($TimeoutSec)
$file = $null
while ((Get-Date) -lt $deadline) {
  $file = Get-ChildItem -Path $outDir -Filter "$marker*.json" -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if ($file) { break }
  if ($proc.HasExited) {
    Start-Sleep -Milliseconds 400
    $file = Get-ChildItem -Path $outDir -Filter "$marker*.json" -ErrorAction SilentlyContinue |
      Select-Object -First 1
    break
  }
  Start-Sleep -Milliseconds 200
}
$elapsed = $sw.Elapsed
if (-not $proc.HasExited) {
  & taskkill.exe /PID $proc.Id /T /F 2>&1 | Out-Null
}
if (-not $file) {
  throw "no marker '$marker' within $TimeoutSec s (out=$outDir)"
}
$json = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
$summary = [ordered]@{
  scenario          = $Scenario
  rows              = $Rows
  steps             = $Steps
  tag               = $Tag
  wall_ms_to_marker = [math]::Round($elapsed.TotalMilliseconds, 1)
  marker_file       = $file.FullName
  marker            = $json
}
$name = if ($Scenario -eq 'scroll') {
  "gui_$Scenario`_$Rows" + $(if ($Tag -ne '') { "_$Tag" } else { '' }) + '.json'
} else {
  "gui_$Scenario" + $(if ($Tag -ne '') { "_$Tag" } else { '' }) + '.json'
}
$out = Join-Path $EvidenceDir $name
$summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $out -Encoding UTF8
Write-Output "SP31_GUI_OK scenario=$Scenario wall_ms_to_marker=$($summary.wall_ms_to_marker) out=$out"
