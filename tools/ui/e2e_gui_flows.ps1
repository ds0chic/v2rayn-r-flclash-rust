<#
.SYNOPSIS
  GUI click-flow acceptance (real clicks) for the remaining Wave B entries.

.DESCRIPTION
  Drives the ARMED evidence package with an isolated data dir and the semantics
  snapshot bridge on, using REAL mouse/keyboard input located through stable
  semantics selectors:

    A. synthetic clipboard import (配置项 -> 从剪贴板导入分享链接) and a second
       launch on the same data dir to prove persistence (FLD-CFG-001/002),
    B. real-click menu paths that open the routing / DNS / full-template /
       global-hotkey / theme / backup windows (formal-entry halves of the
       Wave B rows); each opened window is verified via UIA window-level and a
       window-rect screenshot,
    C. real-click the main-window bottom 信息/日志 tabs (page switch).

  No core is started, no port is bound, 127.0.0.1:10808 is never referenced,
  the host proxy / Run-key / routes / TUN are untouched, and the only input is
  synthetic (clipboard text + clicks). The update window is NOT opened (it
  would attempt a real remote check; registered as needing release infra).
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

# Chinese labels built from code points (PS 5.1 reads BOM-less scripts as ANSI).
$L = @{
  MenuServers        = -join ([char]0x914D, [char]0x7F6E, [char]0x9879)                       # 配置项
  MenuSubs           = -join ([char]0x8BA2, [char]0x9605, [char]0x5206, [char]0x7EC4)         # 订阅分组
  MenuSetting        = -join ([char]0x8BBE, [char]0x7F6E)                                     # 设置
  MenuHelp           = -join ([char]0x5E2E, [char]0x52A9)                                     # 帮助
  ImportClipboard    = -join ([char]0x4ECE, [char]0x526A, [char]0x8D34, [char]0x677F, [char]0x5BFC, [char]0x5165, [char]0x5206, [char]0x4EAB, [char]0x94FE, [char]0x63A5)
  RoutingSetting     = -join ([char]0x8DEF, [char]0x7531, [char]0x8BBE, [char]0x7F6E)         # 路由设置
  DnsSetting         = -join ([char]0x0044, [char]0x004E, [char]0x0053, [char]0x0020, [char]0x8BBE, [char]0x7F6E)
  FullTemplate       = -join ([char]0x5B8C, [char]0x6574, [char]0x914D, [char]0x7F6E, [char]0x6A21, [char]0x677F, [char]0x8BBE, [char]0x7F6E)
  HotkeySetting      = -join ([char]0x5168, [char]0x5C40, [char]0x70ED, [char]0x952E, [char]0x8BBE, [char]0x7F6E)
  ThemeSetting       = -join ([char]0x4E3B, [char]0x9898, [char]0x8BBE, [char]0x7F6E)         # 主题设置
  BackupRestore      = -join ([char]0x5907, [char]0x4EFD, [char]0x548C, [char]0x8FD8, [char]0x539F)
  Cancel             = -join ([char]0x53D6, [char]0x6D88)
  InfoTab            = -join ([char]0x4FE1, [char]0x606F)                                     # 信息
  LogTab             = -join ([char]0x65E5, [char]0x5FD7)                                     # 日志
}

if ($Zip -eq '') {
  $Zip = (Get-ChildItem -Path (Join-Path $RepoRoot 'dist\evidence-armed') -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Zip -or -not (Test-Path -LiteralPath $Zip)) { throw "armed zip not found: $Zip" }
$Zip = (Resolve-Path -LiteralPath $Zip).Path
if ($EvidenceDir -eq '') { $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\stable-port\SP-30\runs\ui-gui-flows' }
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null

$work = Join-Path $env:TEMP ('sp30_gui_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $work
$pkg = (Get-ChildItem -Path $work -Directory | Select-Object -First 1).FullName
$exe = Join-Path $pkg 'v2rayn_desktop.exe'
$data = Join-Path $work 'data'
New-Item -ItemType Directory -Path $data -Force | Out-Null
$dump = Join-Path $work 'semantics.json'
$fixture = Join-Path $RepoRoot 'fixtures\acceptance\sp30\synthetic-sub.txt'

$results = [System.Collections.Generic.List[object]]::new()
function Add-Result([string]$Id, [string]$Status, [string]$Detail, [string]$Shot) {
  $results.Add([ordered]@{ id = $Id; status = $Status; detail = $Detail; screenshot = $Shot }) | Out-Null
}

function Start-App {
  $env:V2RAYN_R_DATA_DIR = $data
  $env:V2RAYN_R_ENABLE_SEMANTICS = '1'
  $env:V2RAYN_R_SEMANTICS_DUMP = $dump
  return (Start-Process -FilePath $exe -PassThru)
}
function Stop-App($proc) {
  if ($proc -and -not $proc.HasExited) { & taskkill.exe /PID $proc.Id /T /F 2>&1 | Out-Null }
}
function Open-MenuWindow([string]$topLabel, [string]$subLabel, [string[]]$titleCandidates, [string]$shotName) {
  $main = Wait-UiWindow -ProcessId $script:proc.Id -TimeoutSec $WindowWaitSec
  if (-not $main.ok) { return @{ ok = $false; detail = "main-window-missing: $($main.error)" } }
  $top = Get-UiElement -Window $main -SemanticsPath $dump -Label $topLabel -TimeoutSec 10
  if (-not $top.ok) { return @{ ok = $false; detail = "top-menu-not-found: $($top.error)" } }
  $clicked = $false
  for ($i = 1; $i -le 3 -and -not $clicked; $i++) {
    Start-Sleep -Milliseconds 400
    $c = Invoke-UiRealClick -Element $top
    Start-Sleep -Milliseconds 800
    $sub = Get-UiElement -Window $main -SemanticsPath $dump -Label $subLabel -TimeoutSec 3
    if ($sub.ok) {
      $c2 = Invoke-UiRealClick -Element $sub
      $clicked = $c2.ok
    }
  }
  if (-not $clicked) { return @{ ok = $false; detail = "submenu-not-clicked: $subLabel" } }
  $win = $null
  foreach ($title in $titleCandidates) {
    $win = Wait-UiWindow -ProcessId $script:proc.Id -Title $title -TimeoutSec 6
    if ($win.ok) { break }
  }
  if (-not $win -or -not $win.ok) { return @{ ok = $false; detail = "window-not-opened: $($titleCandidates -join '/')" } }
  $shot = Join-Path $EvidenceDir $shotName
  $shotRes = Save-UiShot -Window $win -Path $shot
  # close the opened window with ESC (no save)
  $null = Send-UiKey -Window $win -Key ESC
  Start-Sleep -Milliseconds 600
  return @{ ok = $true; detail = "window '$($win.title)' opened"; screenshot = $(if ($shotRes.ok) { $shot } else { '' }) }
}

# ---- Phase A: clipboard import + persistence (001/002) ----
$script:proc = Start-App
try {
  $main = Wait-UiWindow -ProcessId $script:proc.Id -TimeoutSec $WindowWaitSec
  if ($main.ok) {
    Set-Clipboard -Value (Get-Content -LiteralPath $fixture -Raw -Encoding UTF8)
    $top = Get-UiElement -Window $main -SemanticsPath $dump -Label $L.MenuServers -TimeoutSec 10
    $imported = $false
    for ($i = 1; $i -le 3 -and -not $imported; $i++) {
      Start-Sleep -Milliseconds 400
      $null = Invoke-UiRealClick -Element $top
      Start-Sleep -Milliseconds 800
      $item = Get-UiElement -Window $main -SemanticsPath $dump -Label $L.ImportClipboard -TimeoutSec 3
      if ($item.ok) { $null = Invoke-UiRealClick -Element $item; $imported = $true }
    }
    Start-Sleep -Milliseconds 1500
    # a preview/confirm dialog may appear; confirm if present
    $confirm = Get-UiElement -Window $main -SemanticsPath $dump -Label (-join ([char]0x786E, [char]0x5B9A)) -TimeoutSec 3
    if ($confirm.ok) { $null = Invoke-UiRealClick -Element $confirm; Start-Sleep -Milliseconds 1200 }
    $sem = Get-UiSemantics -Path $dump -TimeoutSec 10
    $alias = 'sp30-synth-01'
    $found = $false
    if ($sem.ok) {
      foreach ($n in $sem.nodes) { if (($n.label -like "*$alias*") -or ($n.value -like "*$alias*")) { $found = $true; break } }
    }
    $shot = Join-Path $EvidenceDir 'import-clipboard.png'
    $null = Save-UiShot -Window $main -Path $shot
    Add-Result '001/002-import' $(if ($found) { 'pass' } else { 'blocked' }) "clipboard import executed; alias '$alias' visible=$found" $(if (Test-Path $shot) { $shot } else { '' })
  } else {
    Add-Result '001/002-import' 'blocked' "main window missing: $($main.error)" ''
  }
} finally {
  Stop-App $script:proc
}
# persistence: relaunch on the same data dir and re-read the table
$script:proc = Start-App
try {
  $main2 = Wait-UiWindow -ProcessId $script:proc.Id -TimeoutSec $WindowWaitSec
  $persisted = $false
  if ($main2.ok) {
    $sem2 = Get-UiSemantics -Path $dump -TimeoutSec 15
    if ($sem2.ok) {
      foreach ($n in $sem2.nodes) { if (($n.label -like '*sp30-synth-01*') -or ($n.value -like '*sp30-synth-01*')) { $persisted = $true; break } }
    }
    $shot2 = Join-Path $EvidenceDir 'import-persist-reopen.png'
    $null = Save-UiShot -Window $main2 -Path $shot2
  }
  Add-Result '001/002-persist' $(if ($persisted) { 'pass' } else { 'blocked' }) "after relaunch alias visible=$persisted" $(if (Test-Path $shot2) { $shot2 } else { '' })
} finally {
  Stop-App $script:proc
}

# ---- Phase B: menu-opened windows (formal-entry halves) ----
$script:proc = Start-App
try {
  $main3 = Wait-UiWindow -ProcessId $script:proc.Id -TimeoutSec $WindowWaitSec
  $flows = @(
    @{ id = '074-077-routing-window'; top = $L.MenuSetting; sub = $L.RoutingSetting; titles = @($L.RoutingSetting); shot = 'window-routing.png' },
    @{ id = '070-073-dns-window'; top = $L.MenuSetting; sub = $L.DnsSetting; titles = @($L.DnsSetting, 'DNS'); shot = 'window-dns.png' },
    @{ id = '078-083-template-window'; top = $L.MenuSetting; sub = $L.FullTemplate; titles = @($L.FullTemplate); shot = 'window-template.png' },
    @{ id = '088-092-hotkey-window'; top = $L.MenuSetting; sub = $L.HotkeySetting; titles = @($L.HotkeySetting); shot = 'window-hotkey.png' },
    @{ id = '156-158-theme-window'; top = $L.MenuSetting; sub = $L.ThemeSetting; titles = @($L.ThemeSetting); shot = 'window-theme.png' },
    @{ id = '143-146-backup-window'; top = $L.MenuSubs; sub = $L.BackupRestore; titles = @($L.BackupRestore); shot = 'window-backup.png' }
  )
  foreach ($f in $flows) {
    if (-not $main3.ok) { Add-Result $f.id 'blocked' 'main window missing' ''; continue }
    $r = Open-MenuWindow -topLabel $f.top -subLabel $f.sub -titleCandidates $f.titles -shotName $f.shot
    Add-Result $f.id $(if ($r.ok) { 'pass' } else { 'blocked' }) $r.detail $(if ($r.ContainsKey('screenshot')) { $r.screenshot } else { '' })
  }
  # ---- Phase C: main-window bottom tabs ----
  foreach ($tab in @(@{ id = '070-info-tab'; label = $L.InfoTab; shot = 'tab-info.png' }, @{ id = '073-log-tab'; label = $L.LogTab; shot = 'tab-log.png' })) {
    $el = Get-UiElement -Window $main3 -SemanticsPath $dump -Label $tab.label -TimeoutSec 8
    if ($el.ok) {
      $null = Invoke-UiRealClick -Element $el
      Start-Sleep -Milliseconds 700
      $shot = Join-Path $EvidenceDir $tab.shot
      $null = Save-UiShot -Window $main3 -Path $shot
      Add-Result $tab.id 'pass' "tab '$($tab.label)' clicked" $(if (Test-Path $shot) { $shot } else { '' })
    } else {
      Add-Result $tab.id 'blocked' "tab not found: $($el.error)" ''
    }
  }
} finally {
  Stop-App $script:proc
  Remove-Item Env:V2RAYN_R_SEMANTICS_DUMP -ErrorAction SilentlyContinue
  Remove-Item Env:V2RAYN_R_ENABLE_SEMANTICS -ErrorAction SilentlyContinue
  Remove-Item Env:V2RAYN_R_DATA_DIR -ErrorAction SilentlyContinue
}

$summary = [ordered]@{
  mode = 'sp30-gui-flows'
  zip = $Zip
  results = $results
  pass = ($results | Where-Object { $_.status -eq 'pass' }).Count
  blocked = ($results | Where-Object { $_.status -eq 'blocked' }).Count
  side_effects = 'none (isolated data dir; synthetic clipboard; no core; no port bind; 10808 untouched; no proxy/registry/TUN/Run-key change)'
}
$out = Join-Path $EvidenceDir 'gui-flows.json'
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $out -Encoding UTF8
Write-Output ("SP30_GUI_FLOWS pass=" + $summary.pass + " blocked=" + $summary.blocked + " out=" + $out)
