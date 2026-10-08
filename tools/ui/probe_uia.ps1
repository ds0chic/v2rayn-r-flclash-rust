<#
.SYNOPSIS
  Probe the Windows UI Automation tree of the packaged app (read-only).

.DESCRIPTION
  Extracts the packaged zip into an isolated temp dir, launches the shipped exe
  with a FRESH isolated data dir, waits for its top-level window to appear in
  the UIA tree (by PID), dumps the accessible tree (Name / ControlType /
  AutomationId / ClassName / BoundingRectangle / patterns) to JSON, then stops
  ONLY the PID tree it started.

  No core is started, no port is bound, 127.0.0.1:10808 is never referenced,
  and the host proxy / Run-key / routes / TUN are untouched.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/ui/probe_uia.ps1 `
    -Zip dist/v2rayN-R-1.0.0+1-windows-x64.zip `
    -OutDir docs/evidence/stable-port/SP-30/runs/<candidate>/uia
#>
[CmdletBinding()]
param(
  [string]$Zip = '',
  [string]$OutDir = '',
  [int]$WaitSec = 30,
  [int]$MaxDepth = 6,
  [switch]$EnableSemantics
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($Zip -eq '') {
  $Zip = (Get-ChildItem -Path (Join-Path $RepoRoot 'dist') -Filter 'v2rayN-R-*-windows-x64.zip' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Zip -or -not (Test-Path -LiteralPath $Zip)) { throw "zip not found: $Zip" }
$Zip = (Resolve-Path -LiteralPath $Zip).Path
if ($OutDir -eq '') { $OutDir = Join-Path $RepoRoot 'docs\evidence\stable-port\SP-30\runs\uia-probe' }
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Sp30Focus {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
'@

function Dump-Node {
  param([System.Windows.Automation.AutomationElement]$El, [int]$Depth, [int]$Max)
  $rect = $El.Current.BoundingRectangle
  $patterns = @()
  try {
    if ($El.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$null)) { $patterns += 'Invoke' }
  } catch {}
  try {
    if ($El.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$null)) { $patterns += 'Toggle' }
  } catch {}
  try {
    if ($El.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$null)) { $patterns += 'Value' }
  } catch {}
  try {
    if ($El.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$null)) { $patterns += 'SelectionItem' }
  } catch {}
  try {
    if ($El.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$null)) { $patterns += 'ExpandCollapse' }
  } catch {}
  $node = [ordered]@{
    name         = $El.Current.Name
    controlType  = $El.Current.ControlType.ProgrammaticName
    automationId = $El.Current.AutomationId
    className    = $El.Current.ClassName
    enabled      = $El.Current.IsEnabled
    offscreen    = $El.Current.IsOffscreen
    rect         = @([int]$rect.X, [int]$rect.Y, [int]$rect.Width, [int]$rect.Height)
    patterns     = $patterns
  }
  if ($Depth -ge $Max) { return $node }
  $kids = @()
  $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
  $child = $walker.GetFirstChild($El)
  while ($child -ne $null) {
    $kids += ,(Dump-Node -El $child -Depth ($Depth + 1) -Max $Max)
    $child = $walker.GetNextSibling($child)
  }
  if ($kids.Count -gt 0) { $node['children'] = $kids }
  return $node
}

$work = Join-Path $env:TEMP ('sp30_uia_' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $work
$pkg = (Get-ChildItem -Path $work -Directory | Select-Object -First 1).FullName
$exe = Join-Path $pkg 'v2rayn_desktop.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "packaged exe missing: $exe" }
$data = Join-Path $work 'data'
New-Item -ItemType Directory -Path $data -Force | Out-Null

$env:V2RAYN_R_DATA_DIR = $data
if ($EnableSemantics) { $env:V2RAYN_R_ENABLE_SEMANTICS = '1' }
$proc = Start-Process -FilePath $exe -PassThru
$root = [System.Windows.Automation.AutomationElement]::RootElement
$pidCond = New-Object System.Windows.Automation.PropertyCondition(
  [System.Windows.Automation.AutomationElement]::ProcessIdProperty, [int]$proc.Id)
$win = $null
$deadline = (Get-Date).AddSeconds($WaitSec)
while ((Get-Date) -lt $deadline) {
  $win = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $pidCond)
  if ($win -ne $null) { break }
  Start-Sleep -Milliseconds 400
}
$result = [ordered]@{
  mode             = 'uia-probe'
  zip              = $Zip
  pid              = $proc.Id
  window_found     = ($win -ne $null)
  window_title     = if ($win) { $win.Current.Name } else { '' }
  window_class     = if ($win) { $win.Current.ClassName } else { '' }
  tree             = if ($win) { Dump-Node -El $win -Depth 0 -Max $MaxDepth } else { $null }
  side_effects     = 'none (isolated data dir; no core; no port bind; 10808 untouched; no proxy/registry/TUN/Run-key change)'
}

# Second pass: give the Flutter accessibility bridge time to materialize the
# semantics tree, then count/dump descendants via the raw view as well.
if ($win -ne $null) {
  try {
    $h = [IntPtr]$win.Current.NativeWindowHandle
    [Sp30Focus]::ShowWindow($h, 9) | Out-Null
    [Sp30Focus]::SetForegroundWindow($h) | Out-Null
  } catch {}
  Start-Sleep -Seconds 3
  $trueCond = [System.Windows.Automation.Condition]::TrueCondition
  $desc = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, $trueCond)
  # Also query the FLUTTERVIEW pane directly (its provider is what Flutter's
  # UIA bridge implements) to force activation.
  $pane = $win.FindFirst(
    [System.Windows.Automation.TreeScope]::Descendants,
    (New-Object System.Windows.Automation.PropertyCondition(
      [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'FLUTTERVIEW')))
  $paneDesc = $null
  if ($pane -ne $null) {
    $paneHwnd = [IntPtr]$pane.Current.NativeWindowHandle
    if ($paneHwnd -ne [IntPtr]::Zero) {
      $paneEl = [System.Windows.Automation.AutomationElement]::FromHandle($paneHwnd)
      if ($paneEl -ne $null) {
        $paneDesc = $paneEl.FindAll([System.Windows.Automation.TreeScope]::Descendants, $trueCond)
      }
    }
  }
  $rawNodes = @()
  $rawWalker = [System.Windows.Automation.TreeWalker]::RawViewWalker
  function Dump-Raw([System.Windows.Automation.AutomationElement]$El, [int]$Depth, [int]$Max) {
    $rect = $El.Current.BoundingRectangle
    $node = [ordered]@{
      name         = $El.Current.Name
      controlType  = $El.Current.ControlType.ProgrammaticName
      automationId = $El.Current.AutomationId
      className    = $El.Current.ClassName
      rect         = @([int]$rect.X, [int]$rect.Y, [int]$rect.Width, [int]$rect.Height)
    }
    if ($Depth -ge $Max) { return $node }
    $kids = @()
    $c = $rawWalker.GetFirstChild($El)
    while ($c -ne $null) { $kids += ,(Dump-Raw -El $c -Depth ($Depth + 1) -Max $Max); $c = $rawWalker.GetNextSibling($c) }
    if ($kids.Count -gt 0) { $node['children'] = $kids }
    return $node
  }
  $result['descendant_count'] = $desc.Count
  $result['pane_descendant_count'] = if ($paneDesc) { $paneDesc.Count } else { -1 }
  $result['raw_tree'] = Dump-Raw -El $win -Depth 0 -Max $MaxDepth
}

$out = Join-Path $OutDir 'uia-tree.json'
$result | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $out -Encoding UTF8
$alive = -not $proc.HasExited
if ($alive) { & taskkill.exe /PID $proc.Id /T /F 2>&1 | Out-Null }
$env:V2RAYN_R_DATA_DIR = $null
if ($EnableSemantics) { $env:V2RAYN_R_ENABLE_SEMANTICS = $null }
Write-Output ("UIA_PROBE window_found=" + $result.window_found + " out=" + $out)
