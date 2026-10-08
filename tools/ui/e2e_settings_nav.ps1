<#
.SYNOPSIS
  Real end-to-end UI acceptance for the desktop automation layer (SP-30).

.DESCRIPTION
  Launches the ARMED evidence package with an isolated data dir and the
  semantics-dump bridge on, then drives the UI with REAL mouse/keyboard input
  (SendInput via tools/ui/UiAutomation.ps1) located through stable semantics
  selectors (identifiers/labels), never through app/business APIs:

    1. find the main window (UIA window-level by PID),
    2. real-click the 设置 menu (semantics label),
    3. wait for the settings window, real-click the 核心类型设置 tab
       (identifier settings.tab.coretype), then back to the core tab,
    4. real-click the User-Agent field (identifier settings.core.user_agent),
       type synthetic text, and confirm the value via a fresh semantics snapshot,
    5. real-click 取消 to discard (nothing is saved), screenshot, and stop only
       the PID tree it started.

  Safety: no core, no port bind, 127.0.0.1:10808 never referenced, host proxy /
  Run-key / routes / TUN untouched, synthetic data only.
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$EvidenceDir = '',
  [int]$WindowWaitSec = 40
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
. (Join-Path $PSScriptRoot 'UiAutomation.ps1')

# Chinese UI labels are built from code points so this script stays ASCII-only:
# Windows PowerShell 5.1 reads BOM-less scripts as ANSI and would mangle
# non-ASCII literals. 设置 = U+8BBE U+7F6E, 取消 = U+53D6 U+6D88.
$labelSettings = -join ([char]0x8BBE, [char]0x7F6E)
$labelCancel = -join ([char]0x53D6, [char]0x6D88)
# 参数设置 = U+53C2 U+6570 U+8BBE U+7F6E (the 设置 submenu entry that opens the
# settings window; the top-level menu only opens the dropdown).
$labelOptionSetting = -join ([char]0x53C2, [char]0x6570, [char]0x8BBE, [char]0x7F6E)

if ($Zip -eq '') {
  $Zip = (Get-ChildItem -Path (Join-Path $RepoRoot 'dist\evidence-armed') -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Zip -or -not (Test-Path -LiteralPath $Zip)) { throw "armed zip not found: $Zip" }
$Zip = (Resolve-Path -LiteralPath $Zip).Path
if ($EvidenceDir -eq '') { $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\stable-port\SP-30\runs\ui-e2e' }
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null

$steps = [System.Collections.Generic.List[object]]::new()
function Step([string]$Name, $Result) {
  $steps.Add([ordered]@{ step = $Name; result = $Result }) | Out-Null
}

$work = Join-Path $env:TEMP ('sp30_e2e_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $work
$pkg = (Get-ChildItem -Path $work -Directory | Select-Object -First 1).FullName
$exe = Join-Path $pkg 'v2rayn_desktop.exe'
$data = Join-Path $work 'data'
New-Item -ItemType Directory -Path $data -Force | Out-Null
$dump = Join-Path $work 'semantics.json'
$dumpSettings = "$dump.settings"

$env:V2RAYN_R_DATA_DIR = $data
$env:V2RAYN_R_ENABLE_SEMANTICS = '1'
$env:V2RAYN_R_SEMANTICS_DUMP = $dump
$proc = Start-Process -FilePath $exe -PassThru
$result = [ordered]@{
  mode = 'sp30-ui-e2e'
  zip = $Zip
  pid = $proc.Id
  ok = $false
  steps = $steps
  screenshots = @()
  error = $null
  side_effects = 'none (isolated data dir; no core; no port bind; 10808 untouched; no proxy/registry/TUN/Run-key change)'
}

function Find-SemanticsNode([string]$Path, [string]$Identifier, [string]$Label) {
  $sem = Get-UiSemantics -Path $Path -TimeoutSec 10
  if (-not $sem.ok) { return $null }
  foreach ($n in $sem.nodes) {
    $idOk = ($Identifier -eq '' -or $n.identifier -ieq $Identifier)
    $labelOk = ($Label -eq '' -or $n.label -ieq $Label)
    if ($idOk -and $labelOk) { return $n }
  }
  return $null
}

try {
  $main = Wait-UiWindow -ProcessId $proc.Id -TimeoutSec $WindowWaitSec
  Step 'find-main-window' $main
  if (-not $main.ok) { throw "main window not found: $($main.error)" }

  $sem = Get-UiSemantics -Path $dump -TimeoutSec 20
  Step 'read-main-semantics' ([ordered]@{ ok = $sem.ok; nodeCount = $sem.count; error = $sem.error })
  if (-not $sem.ok) { throw "main semantics dump unavailable: $($sem.error)" }

  $menu = Get-UiElement -Window $main -SemanticsPath $dump -Label $labelSettings -TimeoutSec 10
  Step 'locate-settings-menu' $menu
  if (-not $menu.ok) { throw "settings menu not found: $($menu.error)" }
  $click = Invoke-UiRealClick -Element $menu
  Step 'click-settings-menu' $click
  if (-not $click.ok) { throw "settings menu click failed: $($click.error)" }

  # The top-level menu opens a dropdown; click the 参数设置 entry.
  Start-Sleep -Milliseconds 800
  $optionItem = Get-UiElement -Window $main -SemanticsPath $dump -Label $labelOptionSetting -TimeoutSec 10
  Step 'locate-option-setting-item' $optionItem
  if (-not $optionItem.ok) { throw "option-setting item not found: $($optionItem.error)" }
  $clickOption = Invoke-UiRealClick -Element $optionItem
  Step 'click-option-setting-item' $clickOption
  if (-not $clickOption.ok) { throw "option-setting click failed: $($clickOption.error)" }

  $settingsWin = Wait-UiWindow -ProcessId $proc.Id -Title $labelSettings -TimeoutSec 20
  Step 'find-settings-window' $settingsWin
  if (-not $settingsWin.ok) { throw "settings window not found: $($settingsWin.error)" }

  $semS = Get-UiSemantics -Path $dumpSettings -TimeoutSec 20
  Step 'read-settings-semantics' ([ordered]@{ ok = $semS.ok; nodeCount = $semS.count; error = $semS.error })
  if (-not $semS.ok) { throw "settings semantics dump unavailable: $($semS.error)" }

  # Switch to the core-type tab (no side effects), then back to the core tab.
  $tab = Get-UiElement -Window $settingsWin -SemanticsPath $dumpSettings -Identifier 'settings.tab.coretype' -TimeoutSec 10
  Step 'locate-coretype-tab' $tab
  if ($tab.ok) { Step 'click-coretype-tab' (Invoke-UiRealClick -Element $tab) }
  Start-Sleep -Milliseconds 700
  $tabCore = Get-UiElement -Window $settingsWin -SemanticsPath $dumpSettings -Identifier 'settings.tab.core' -TimeoutSec 10
  Step 'locate-core-tab' $tabCore
  if ($tabCore.ok) { Step 'click-core-tab' (Invoke-UiRealClick -Element $tabCore) }
  Start-Sleep -Milliseconds 700

  # Type synthetic text into the User-Agent field and confirm via semantics.
  $ua = Get-UiElement -Window $settingsWin -SemanticsPath $dumpSettings -Identifier 'settings.core.user_agent' -TimeoutSec 10
  Step 'locate-user-agent-field' $ua
  if (-not $ua.ok) { throw "user-agent field not found: $($ua.error)" }
  Step 'click-user-agent-field' (Invoke-UiRealClick -Element $ua)
  Start-Sleep -Milliseconds 400
  $typed = 'uia-synthetic-ua'
  Step 'type-user-agent' (Send-UiText -Window $settingsWin -Text $typed)
  Start-Sleep -Milliseconds 1200

  $uaNode = Find-SemanticsNode -Path $dumpSettings -Identifier 'settings.core.user_agent'
  $confirmed = $false
  $seen = ''
  if ($uaNode -ne $null) {
    $seen = [string]$uaNode.value
    $confirmed = ($seen -like "*$typed*")
  }
  Step 'confirm-user-agent-value' ([ordered]@{
      ok = $confirmed; valueRedacted = ("len=" + $typed.Length)
    })

  $shot = Join-Path $EvidenceDir 'e2e-settings-typed.png'
  $shotRes = Save-UiShot -Window $settingsWin -Path $shot
  Step 'screenshot' $shotRes
  if ($shotRes.ok) { $result.screenshots += $shot }

  # Discard the draft: real-click 取消.
  $cancel = Get-UiElement -Window $settingsWin -SemanticsPath $dumpSettings -Label $labelCancel -TimeoutSec 10
  Step 'locate-cancel' $cancel
  if ($cancel.ok) { Step 'click-cancel' (Invoke-UiRealClick -Element $cancel) }
  Start-Sleep -Milliseconds 700

  $result.ok = $confirmed
  if (-not $confirmed) { $result.error = "typed value not confirmed in semantics (valueRedacted=len$($typed.Length))" }
} catch {
  $result.error = "$_"
} finally {
  if (-not $proc.HasExited) { & taskkill.exe /PID $proc.Id /T /F 2>&1 | Out-Null }
  Remove-Item Env:V2RAYN_R_SEMANTICS_DUMP -ErrorAction SilentlyContinue
  Remove-Item Env:V2RAYN_R_ENABLE_SEMANTICS -ErrorAction SilentlyContinue
  Remove-Item Env:V2RAYN_R_DATA_DIR -ErrorAction SilentlyContinue
}

$out = Join-Path $EvidenceDir 'e2e-settings-nav.json'
$result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $out -Encoding UTF8
Write-Output ("SP30_UI_E2E ok=" + $result.ok + " error=" + $result.error + " out=" + $out)
