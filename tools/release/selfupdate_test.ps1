<#
.SYNOPSIS
  Capture T21 external self-update evidence (success + rollback/digest cases).

.DESCRIPTION
  Runs the `upgrade_runner` integration tests (which drive the real pipeline
  against a loopback GitHub mock: check -> download -> verify -> external swap
  -> restart, and the digest-mismatch abort) with `--nocapture`, then snapshots
  the runner's machine-readable result documents and a timing summary into
  docs/evidence/T21-install-update.runs/selfupdate-<stamp>.json.

  Uses only OS-assigned loopback ports and short-lived child processes; never
  touches 10808, the system proxy or TUN.
#>
[CmdletBinding()]
param(
  [string]$EvidenceDir = ''
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($EvidenceDir -eq '') { $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\T21-install-update.runs' }
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$jsonPath = Join-Path $EvidenceDir "selfupdate-$stamp.json"
$logPath = Join-Path $EvidenceDir "selfupdate-$stamp.log"

$cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargo) { $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
if (-not (Test-Path -LiteralPath $cargo)) { throw 'cargo not found' }

$env:Path = (Split-Path $cargo) + ';' + $env:Path
$started = (Get-Date).ToUniversalTime()
& $cargo test -p upgrade_runner --locked --test self_update -- --nocapture 2>&1 |
  Tee-Object -FilePath $logPath
$exit = $LASTEXITCODE
$finished = (Get-Date).ToUniversalTime()

$result = [ordered]@{
  schema = 1
  task = 'T21-install-update'
  kind = 'selfupdate'
  timestamp_utc = $stamp
  exit = $exit
  passed = ($exit -eq 0)
  started_utc = $started.ToString('o')
  finished_utc = $finished.ToString('o')
  duration_ms = [int](($finished - $started).TotalMilliseconds)
  cases = @(
    [ordered]@{
      name = 'self_update_replaces_and_backs_up_then_restarts'
      steps = @('check_core(app_update_spec) -> stage/verify -> plan JSON',
                'spawn upgrade_runner --pid <fake app>',
                'apply_atomic swap (backup v2rayN-R.previous)',
                'restart stub writes marker',
                'assert NEW-VERSION.txt, version.txt=1.0.1, backup=1.0.0, result.ok')
      verified = ($exit -eq 0)
    },
    [ordered]@{
      name = 'digest_mismatch_aborts_without_touching_install'
      steps = @('metadata declares sha256 of a different valid zip',
                'served bytes hash differently -> DigestMismatch (E_CONFLICT)',
                'old install byte-for-byte unchanged; no backup dir; no staging leak')
      verified = ($exit -eq 0)
    },
    [ordered]@{
      name = 'commit_rename_failure_leaves_install_usable'
      steps = @('upgrade_runner --fail-inject commit',
                'apply_atomic injects a final-rename failure',
                'runner exits non-zero; old install intact; no keep dir left')
      verified = ($exit -eq 0)
    },
    [ordered]@{
      name = 'crash_between_renames_is_recovered'
      steps = @('simulate half-done swap: current missing, keep dir present',
                'updater::install::restore_previous renames keep -> current',
                'old version byte-for-byte restored')
      verified = ($exit -eq 0)
    }
  )
  log = $logPath
  limitations = @(
    'restart target is a marker-writing stub, not the GUI (keeps the test headless)',
    'mock asserts on-binary replacement + restart, not a TLS handshake with real GitHub'
  )
}
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
Write-Host ""
Write-Host "SELFUPDATE JSON $jsonPath"
Write-Host "SELFUPDATE LOG  $logPath"
if ($exit -ne 0) { exit $exit }
exit 0
