<#
.SYNOPSIS
  Acceptance baseline snapshot + one-click restore (test/acceptance tool layer).

.DESCRIPTION
  Captures the pre-test state of the categories a manual acceptance run may
  change, stores it under the current user's profile only (ACL-restricted), and
  can roll those categories back with a single, idempotent, fail-closed action.

  Design rules:
  - Capture is read-only. Restore only rolls back categories this tool recorded
    and only when the current value differs from the baseline.
  - 127.0.0.1:20808 is a hard禁区: any category whose baseline or current value
    references port 20808 is marked protected and restore REFUSES (fail-closed),
    pointing at isolated-VM checkpoint restore instead.
  - Route/TUN state is never rolled back here (no blind route/registry deletes);
    those require a disposable VM checkpoint.
  - Logs and evidence never print proxy addresses or secrets; only category
    names, actions and short hashes.

  Entry points: New-AcceptanceSnapshot, Test-AcceptanceSnapshot,
  Get-AcceptanceRestorePlan, Restore-AcceptanceSnapshot. GUI launcher:
  tools/acceptance/AcceptanceRestoreGui.ps1.
#>

$script:AcceptanceSchema = 1
$script:ForbiddenPortToken = '20808'
$script:AcceptanceStoreDefault = Join-Path $env:LOCALAPPDATA 'v2rayn-r\acceptance'

function Get-AcceptanceStore {
  param([string]$StoreDir = '')
  if ($StoreDir -ne '') { return $StoreDir }
  return $script:AcceptanceStoreDefault
}

function Get-ShortHash([string]$Text) {
  if ([string]::IsNullOrEmpty($Text)) { return '' }
  $sha = [System.Security.Cryptography.SHA256]::Create()
  $bytes = $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($Text))
  return (($bytes | ForEach-Object { $_.ToString('x2') }) -join '').Substring(0, 12)
}

# UTF-8 without BOM on both sides so the integrity hash matches byte-for-byte
# (Windows PowerShell 5.1 Set-Content -Encoding UTF8 adds a BOM).
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
function Write-Utf8([string]$Path, [string]$Text) {
  [System.IO.File]::WriteAllText($Path, $Text, $script:Utf8NoBom)
}
function Read-Utf8([string]$Path) {
  return [System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)
}

function Test-Forbidden20808($Value) {
  if ($null -eq $Value) { return $false }
  return ([string]$Value).Contains($script:ForbiddenPortToken)
}

# ---- state backends -------------------------------------------------------
# fake: in-memory provider used by the self-test (no OS access at all).
# real: registry read for capture; writes only in restore for allowed
#       categories. Route/TUN are never read-modified.
function Get-AcceptanceState {
  param([ValidateSet('fake', 'real')][string]$Backend = 'real', $FakeState = $null)
  if ($Backend -eq 'fake') {
    if ($FakeState -eq $null) { throw 'fake backend requires -FakeState' }
    return $FakeState
  }
  $proxyKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
  $proxy = [ordered]@{ enable = 0; server = ''; override = ''; autoConfigUrl = '' }
  try {
    $p = Get-ItemProperty -Path $proxyKey -ErrorAction Stop
    if ($p.PSObject.Properties['ProxyEnable']) { $proxy.enable = [int]$p.ProxyEnable }
    if ($p.PSObject.Properties['ProxyServer']) { $proxy.server = [string]$p.ProxyServer }
    if ($p.PSObject.Properties['ProxyOverride']) { $proxy.override = [string]$p.ProxyOverride }
    if ($p.PSObject.Properties['AutoConfigURL']) { $proxy.autoConfigUrl = [string]$p.AutoConfigURL }
  } catch { }
  $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
  $run = [ordered]@{}
  try {
    $r = Get-ItemProperty -Path $runKey -ErrorAction Stop
    foreach ($prop in $r.PSObject.Properties) {
      if ($prop.Name -notlike 'PS*') { $run[$prop.Name] = [string]$prop.Value }
    }
  } catch { }
  return [ordered]@{ winInetProxy = $proxy; runKey = $run }
}

function Write-AcceptanceStateCategory {
  param([ValidateSet('fake', 'real')][string]$Backend, $FakeState, [string]$Category, $Value)
  if ($Backend -eq 'fake') {
    if ($Category -eq 'winInetProxy') { $FakeState.winInetProxy = $Value }
    if ($Category -eq 'runKey') { $FakeState.runKey = $Value }
    return
  }
  if ($Category -eq 'winInetProxy') {
    $proxyKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
    New-Item -Path $proxyKey -Force | Out-Null
    Set-ItemProperty -Path $proxyKey -Name ProxyEnable -Value ([int]$Value.enable)
    Set-ItemProperty -Path $proxyKey -Name ProxyServer -Value ([string]$Value.server)
    Set-ItemProperty -Path $proxyKey -Name ProxyOverride -Value ([string]$Value.override)
    if ([string]$Value.autoConfigUrl -eq '') {
      Remove-ItemProperty -Path $proxyKey -Name AutoConfigURL -ErrorAction SilentlyContinue
    } else {
      Set-ItemProperty -Path $proxyKey -Name AutoConfigURL -Value ([string]$Value.autoConfigUrl)
    }
  }
  # runKey restore is intentionally limited to values that were recorded as
  # present in the baseline; the tool never deletes unrelated Run entries.
  if ($Category -eq 'runKey') {
    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    foreach ($name in $Value.Keys) {
      Set-ItemProperty -Path $runKey -Name $name -Value ([string]$Value[$name])
    }
  }
}

# ---- snapshot -------------------------------------------------------------
function New-AcceptanceSnapshot {
  [CmdletBinding()]
  param(
    [ValidateSet('fake', 'real')][string]$Backend = 'real',
    $FakeState = $null,
    [string]$StoreDir = ''
  )
  $store = Get-AcceptanceStore -StoreDir $StoreDir
  New-Item -ItemType Directory -Path $store -Force | Out-Null
  $state = Get-AcceptanceState -Backend $Backend -FakeState $FakeState
  $protected = (Test-Forbidden20808 $state.winInetProxy.server) -or
               (Test-Forbidden20808 $state.winInetProxy.override) -or
               (Test-Forbidden20808 $state.winInetProxy.autoConfigUrl)
  $snapshot = [ordered]@{
    schema        = $script:AcceptanceSchema
    createdAtUtc  = (Get-Date).ToUniversalTime().ToString('o')
    protected20808 = $protected
    categories    = [ordered]@{
      winInetProxy = $state.winInetProxy
      runKey       = $state.runKey
    }
  }
  $json = $snapshot | ConvertTo-Json -Depth 8
  $snapshotPath = Join-Path $store 'snapshot.json'
  $metaPath = Join-Path $store 'snapshot.meta.json'
  Write-Utf8 $snapshotPath $json
  $meta = [ordered]@{
    schema        = $script:AcceptanceSchema
    createdAtUtc  = $snapshot.createdAtUtc
    sha256        = (Get-ShortHash $json)
    protected20808 = $protected
    categories    = @('winInetProxy', 'runKey')
    note          = 'baseline values stored locally only; never copied into evidence'
  }
  Write-Utf8 $metaPath ($meta | ConvertTo-Json -Depth 6)
  # restrict access to the current user
  try {
    & icacls.exe $store /inheritance:r /grant:r "$($env:USERNAME):(OI)(CI)F" 2>&1 | Out-Null
  } catch { }
  Write-Output ("ACCEPTANCE_SNAPSHOT ok=true store=$store protected20808=" + $protected +
    " categories=winInetProxy,runKey")
}

function Test-AcceptanceSnapshot {
  [CmdletBinding()]
  param([string]$StoreDir = '')
  $store = Get-AcceptanceStore -StoreDir $StoreDir
  $snapshotPath = Join-Path $store 'snapshot.json'
  $metaPath = Join-Path $store 'snapshot.meta.json'
  if (-not (Test-Path -LiteralPath $snapshotPath) -or -not (Test-Path -LiteralPath $metaPath)) {
    return [pscustomobject]@{ ok = $false; reason = 'snapshot-missing'; protected20808 = $false }
  }
  try {
    $json = Read-Utf8 $snapshotPath
    $meta = Read-Utf8 $metaPath | ConvertFrom-Json
    $snap = $json | ConvertFrom-Json
    if ($snap.schema -ne $script:AcceptanceSchema) {
      return [pscustomobject]@{ ok = $false; reason = 'schema-mismatch'; protected20808 = $false }
    }
    if ((Get-ShortHash $json) -ne [string]$meta.sha256) {
      return [pscustomobject]@{ ok = $false; reason = 'integrity-mismatch'; protected20808 = $false }
    }
    return [pscustomobject]@{
      ok = $true; reason = ''; protected20808 = [bool]$snap.protected20808
      categories = @('winInetProxy', 'runKey'); snapshot = $snap
    }
  } catch {
    return [pscustomobject]@{ ok = $false; reason = ('parse-failed: ' + $_.Exception.Message); protected20808 = $false }
  }
}

function Get-AcceptanceRestorePlan {
  [CmdletBinding()]
  param(
    [ValidateSet('fake', 'real')][string]$Backend = 'real',
    $FakeState = $null,
    [string]$StoreDir = ''
  )
  $valid = Test-AcceptanceSnapshot -StoreDir $StoreDir
  if (-not $valid.ok) {
    return [pscustomobject]@{ ok = $false; refused = $true; reason = $valid.reason; items = @() }
  }
  if ($valid.protected20808) {
    return [pscustomobject]@{
      ok = $false; refused = $true
      reason = 'baseline references 127.0.0.1:20808 - restore refused (fail-closed); use an isolated VM checkpoint'
      items = @()
    }
  }
  $current = Get-AcceptanceState -Backend $Backend -FakeState $FakeState
  $snap = $valid.snapshot
  $items = @()
  # winInetProxy: restore only if current differs and current does not reference 20808
  $curProxy = $current.winInetProxy
  $snapProxy = $snap.categories.winInetProxy
  if ((Test-Forbidden20808 $curProxy.server) -or (Test-Forbidden20808 $curProxy.override) -or
      (Test-Forbidden20808 $curProxy.autoConfigUrl)) {
    $items += [pscustomobject]@{ category = 'winInetProxy'; action = 'refused'; reason = 'current state references 20808' }
  } elseif (($curProxy.enable -ne $snapProxy.enable) -or ($curProxy.server -ne $snapProxy.server) -or
            ($curProxy.override -ne $snapProxy.override) -or ($curProxy.autoConfigUrl -ne $snapProxy.autoConfigUrl)) {
    $items += [pscustomobject]@{ category = 'winInetProxy'; action = 'restore'; reason = 'differs from baseline' }
  } else {
    $items += [pscustomobject]@{ category = 'winInetProxy'; action = 'noop'; reason = 'already at baseline' }
  }
  # runKey: never delete; restore recorded values only
  $items += [pscustomobject]@{ category = 'runKey'; action = 'noop'; reason = 'recorded-only; no deletions' }
  $refused = (@($items | Where-Object { $_.action -eq 'refused' }).Count -gt 0)
  return [pscustomobject]@{ ok = (-not $refused); refused = $refused; reason = ''; items = $items }
}

function Restore-AcceptanceSnapshot {
  [CmdletBinding()]
  param(
    [ValidateSet('fake', 'real')][string]$Backend = 'real',
    $FakeState = $null,
    [string]$StoreDir = '',
    [switch]$DryRun
  )
  $plan = Get-AcceptanceRestorePlan -Backend $Backend -FakeState $FakeState -StoreDir $StoreDir
  if ($plan.refused) {
    return [pscustomobject]@{ ok = $false; refused = $true; reason = $plan.reason; applied = @(); dryRun = [bool]$DryRun }
  }
  $applied = @()
  $valid = Test-AcceptanceSnapshot -StoreDir $StoreDir
  foreach ($item in $plan.items) {
    if ($item.action -ne 'restore') { $applied += $item; continue }
    if ($item.category -eq 'winInetProxy') {
      if (-not $DryRun) {
        Write-AcceptanceStateCategory -Backend $Backend -FakeState $FakeState -Category 'winInetProxy' `
          -Value $valid.snapshot.categories.winInetProxy
      }
      $applied += [pscustomobject]@{ category = 'winInetProxy'; action = 'restore'; reason = 'applied' }
    }
  }
  $log = [ordered]@{
    schema = $script:AcceptanceSchema
    atUtc = (Get-Date).ToUniversalTime().ToString('o')
    dryRun = [bool]$DryRun
    applied = ($applied | ForEach-Object { [ordered]@{ category = $_.category; action = $_.action } })
  }
  if (-not $DryRun) {
    $store = Get-AcceptanceStore -StoreDir $StoreDir
    Write-Utf8 (Join-Path $store 'restore-log.json') ($log | ConvertTo-Json -Depth 6)
  }
  return [pscustomobject]@{ ok = $true; refused = $false; reason = ''; applied = $applied; dryRun = [bool]$DryRun }
}
