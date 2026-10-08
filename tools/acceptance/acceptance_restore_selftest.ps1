<#
.SYNOPSIS
  Self-test for the acceptance restore tool (fake backend only, no host writes).

.DESCRIPTION
  Verifies: snapshot create/restore/repeat-restore (idempotent), corrupted and
  missing snapshot rejection, 20808 fail-closed refusal (baseline and current),
  and that the tool never exposes a route/TUN rollback category. It uses the
  fake in-memory backend exclusively and additionally asserts the real WinINET
  proxy values are unchanged before/after (read-only comparison), proving no
  host proxy write happened.
#>
[CmdletBinding()]
param([string]$OutDir = '')

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'AcceptanceRestore.ps1')

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($OutDir -eq '') { $OutDir = Join-Path $RepoRoot 'docs\evidence\stable-port\SP-30\acceptance-restore' }
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$cases = [System.Collections.Generic.List[object]]::new()
function Case([string]$Name, [bool]$Pass, [string]$Detail) {
  $cases.Add([ordered]@{ case = $Name; pass = $Pass; detail = $Detail }) | Out-Null
}

function Read-RealProxyFingerprint {
  $k = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
  try {
    $p = Get-ItemProperty -Path $k -ErrorAction Stop
    return ("{0}|{1}|{2}|{3}" -f [int]$p.ProxyEnable, (Get-ShortHash ([string]$p.ProxyServer)),
      (Get-ShortHash ([string]$p.ProxyOverride)), (Get-ShortHash ([string]$p.AutoConfigURL)))
  } catch { return 'unavailable' }
}

$before = Read-RealProxyFingerprint

$store = Join-Path $env:TEMP ('accept_restore_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $store | Out-Null

# --- 1. create + restore + repeat (idempotent) ---
$baseline = [ordered]@{
  winInetProxy = [ordered]@{ enable = 1; server = '127.0.0.1:11808'; override = '<local>'; autoConfigUrl = '' }
  runKey       = [ordered]@{ 'v2rayn-r-test' = 'C:\synthetic\app.exe' }
}
$fake = [ordered]@{
  winInetProxy = [ordered]@{ enable = 0; server = ''; override = ''; autoConfigUrl = '' }
  runKey       = [ordered]@{}
}
# snapshot from the baseline, then mutate the fake current state
$null = New-AcceptanceSnapshot -Backend fake -FakeState $baseline -StoreDir $store
$fake.winInetProxy.enable = 0
$fake.winInetProxy.server = ''
$restore1 = Restore-AcceptanceSnapshot -Backend fake -FakeState $fake -StoreDir $store
$restoredOk = ($restore1.ok -and $fake.winInetProxy.enable -eq 1 -and $fake.winInetProxy.server -eq '127.0.0.1:11808')
Case 'create+restore' $restoredOk ("ok=" + $restore1.ok + " serverRestored=" + ($fake.winInetProxy.server -eq '127.0.0.1:11808'))
$restore2 = Restore-AcceptanceSnapshot -Backend fake -FakeState $fake -StoreDir $store
$idem = ($restore2.ok -and (($restore2.applied | Where-Object { $_.category -eq 'winInetProxy' }).action -eq 'noop'))
Case 'repeat-restore-idempotent' $idem ("secondRunAppliedAction=" + (($restore2.applied | Where-Object { $_.category -eq 'winInetProxy' }).action))

# --- 2. corrupted snapshot rejected ---
Add-Content -LiteralPath (Join-Path $store 'snapshot.json') -Value 'tamper' -Encoding UTF8
$corrupt = Test-AcceptanceSnapshot -StoreDir $store
$corruptRestore = Restore-AcceptanceSnapshot -Backend fake -FakeState $fake -StoreDir $store
Case 'corrupted-rejected' ((-not $corrupt.ok) -and $corruptRestore.refused) ("valid=" + $corrupt.ok + " reason=" + $corrupt.reason + " restoreRefused=" + $corruptRestore.refused)

# --- 3. missing snapshot rejected ---
$empty = Join-Path $env:TEMP ('accept_restore_empty_' + [guid]::NewGuid().ToString('N'))
$missing = Test-AcceptanceSnapshot -StoreDir $empty
$missingRestore = Restore-AcceptanceSnapshot -Backend fake -FakeState $fake -StoreDir $empty
Case 'missing-rejected' ((-not $missing.ok) -and $missingRestore.refused) ("reason=" + $missing.reason)

# --- 4. baseline references 20808 -> fail-closed ---
$store2 = Join-Path $env:TEMP ('accept_restore_20808_' + [guid]::NewGuid().ToString('N'))
$baseline20808 = [ordered]@{
  winInetProxy = [ordered]@{ enable = 1; server = '127.0.0.1:20808'; override = ''; autoConfigUrl = '' }
  runKey       = [ordered]@{}
}
$null = New-AcceptanceSnapshot -Backend fake -FakeState $baseline20808 -StoreDir $store2
$protected = Test-AcceptanceSnapshot -StoreDir $store2
$refuse20808 = Restore-AcceptanceSnapshot -Backend fake -FakeState $fake -StoreDir $store2
Case 'baseline-20808-fail-closed' ($protected.protected20808 -and $refuse20808.refused) ("protected=" + $protected.protected20808 + " refused=" + $refuse20808.refused)

# --- 5. current state references 20808 -> refuse ---
$store3 = Join-Path $env:TEMP ('accept_restore_cur20808_' + [guid]::NewGuid().ToString('N'))
$null = New-AcceptanceSnapshot -Backend fake -FakeState $baseline -StoreDir $store3
$fakeCur20808 = [ordered]@{
  winInetProxy = [ordered]@{ enable = 1; server = '127.0.0.1:20808'; override = ''; autoConfigUrl = '' }
  runKey       = [ordered]@{}
}
$refuseCur = Restore-AcceptanceSnapshot -Backend fake -FakeState $fakeCur20808 -StoreDir $store3
Case 'current-20808-refused' $refuseCur.refused ("refused=" + $refuseCur.refused + " reason=" + $refuseCur.reason)

# --- 6. no route/TUN rollback category exists ---
$valid = Test-AcceptanceSnapshot -StoreDir $store3
$noRoutes = (($valid.categories -notcontains 'routes') -and ($valid.categories -notcontains 'tun'))
Case 'no-route-tun-rollback' $noRoutes ("categories=" + ($valid.categories -join ','))

$after = Read-RealProxyFingerprint
Case 'no-host-proxy-write' ($before -eq $after) 'real WinINET proxy fingerprint unchanged (read-only comparison)'

$passCount = ($cases | Where-Object { $_.pass }).Count
$summary = [ordered]@{
  mode = 'acceptance-restore-selftest'
  backend = 'fake-only'
  pass = $passCount
  total = $cases.Count
  cases = $cases
  side_effects = 'none (fake in-memory state only; real proxy compared read-only)'
}
$out = Join-Path $OutDir 'selftest.json'
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $out -Encoding UTF8
Write-Output ("ACCEPT_RESTORE_SELFTEST pass=" + $passCount + "/" + $cases.Count + " out=" + $out)
if ($passCount -ne $cases.Count) { exit 1 }
