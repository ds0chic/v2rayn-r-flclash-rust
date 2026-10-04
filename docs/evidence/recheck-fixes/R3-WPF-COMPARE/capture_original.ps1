# R3-WPF-COMPARE original v2rayN 7.25.4 (WPF) isolated capture.
# Launches the frozen-source build from a temp run dir whose exe directory IS
# the data dir (StartupPath = AppDomain.BaseDirectory). The run dir is
# pre-seeded with guiNConfig.json SysProxyType=Unchanged so the original never
# touches the host system proxy on startup/exit. Only the PID this script
# starts (and its recorded descendants) is ever stopped; the process is
# force-killed so AppExitAsync / UpdateSysProxy never runs.
#
# Usage:
#   pwsh -File capture_original.ps1 -Exe <path> -RunDir <path> -OutDir <path> [-ProbeOnly]
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$RunDir,
  [Parameter(Mandatory = $true)][string]$OutDir,
  [switch]$ProbeOnly
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class WpfWin {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static string Title(IntPtr h) {
    var sb = new System.Text.StringBuilder(512); GetWindowText(h, sb, sb.Capacity); return sb.ToString();
  }
  public static IntPtr FindFirst(uint target) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  public static IntPtr FindByTitleContains(uint target, string needle) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target && IsWindowVisible(h)) {
        var t = Title(h);
        if (t.Length > 0 && t.Contains(needle)) { found = h; return false; }
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  public static IntPtr[] List(uint target) {
    var list = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target && IsWindowVisible(h)) list.Add(h);
      return true;
    }, IntPtr.Zero);
    return list.ToArray();
  }
}
'@

function Get-DescendantPids {
  param([int]$RootPid)
  $result = [System.Collections.Generic.List[int]]::new()
  $queue = [System.Collections.Generic.Queue[int]]::new()
  $queue.Enqueue($RootPid)
  while ($queue.Count -gt 0) {
    $parent = $queue.Dequeue()
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId=$parent" -ErrorAction SilentlyContinue
    foreach ($c in $children) { $result.Add([int]$c.ProcessId) | Out-Null; $queue.Enqueue([int]$c.ProcessId) }
  }
  return $result
}

function Save-WindowShot {
  param([IntPtr]$Hwnd, [string]$Path)
  [WpfWin]::SetForegroundWindow($Hwnd) | Out-Null
  Start-Sleep -Milliseconds 600
  $r = New-Object WpfWin+RECT
  [WpfWin]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  if ($w -le 0 -or $h -le 0) { return $false }
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  return $true
}

function Dump-UiaTree {
  param([IntPtr]$Hwnd, [string]$Path)
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($Hwnd)
  $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
  $lines = [System.Collections.Generic.List[string]]::new()
  function Walk($el, [int]$depth) {
    if ($null -eq $el) { return }
    $pad = '  ' * $depth
    try {
      $ct = $el.Current.ControlType.ProgrammaticName
      $name = $el.Current.Name
      $aid = $el.Current.AutomationId
      $cls = $el.Current.ClassName
      $lines.Add(('{0}{1} | name="{2}" | aid="{3}" | cls="{4}"' -f $pad, $ct, $name, $aid, $cls))
    } catch {}
    if ($depth -gt 8) { return }
    $child = $walker.GetFirstChild($el)
    while ($null -ne $child) {
      Walk $child ($depth + 1)
      $child = $walker.GetNextSibling($child)
    }
  }
  Walk $root 0
  Set-Content -LiteralPath $Path -Value $lines -Encoding UTF8
  return $lines.Count
}

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$proc = Start-Process -FilePath $Exe -WorkingDirectory $RunDir -PassThru
$appPid = $proc.Id
$descBefore = @(Get-DescendantPids -RootPid $appPid)
$result = [ordered]@{ pid = $appPid; exe = $Exe; run_dir = $RunDir; shots = @(); ui_tree = $null; windows = @(); error = $null }
try {
  $main = [IntPtr]::Zero
  for ($i = 0; $i -lt 50 -and $main -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Milliseconds 400
    $main = [WpfWin]::FindFirst([uint32]$appPid)
    if ($proc.HasExited) { throw "process exited early code=$($proc.ExitCode)" }
  }
  if ($main -eq [IntPtr]::Zero) { throw "main window not found" }
  Start-Sleep -Seconds 3
  $main = [WpfWin]::FindFirst([uint32]$appPid)
  $r = New-Object WpfWin+RECT; [WpfWin]::GetWindowRect($main, [ref]$r) | Out-Null
  $result.windows += [ordered]@{ title = [WpfWin]::Title($main); hwnd = $main.ToInt64(); rect = "$($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"; dpi = [WpfWin]::GetDpiForWindow($main) }
  $mainShot = Join-Path $OutDir 'original-main.png'
  if (Save-WindowShot -Hwnd $main -Path $mainShot) { $result.shots += $mainShot }
  $treePath = Join-Path $OutDir 'original-main-uia.txt'
  $result.ui_tree = Dump-UiaTree -Hwnd $main -Path $treePath
}
catch { $result.error = "$_" }
finally {
  $descNow = @(Get-DescendantPids -RootPid $appPid)
  $owned = @($appPid) + $descBefore + @($descNow) | Sort-Object -Unique
  [array]::Reverse($owned)
  $stopped = @()
  foreach ($cp in $owned) { if (Get-Process -Id $cp -ErrorAction SilentlyContinue) { Stop-Process -Id $cp -Force -ErrorAction SilentlyContinue; $stopped += $cp } }
  $result.cleanup = [ordered]@{ stopped = $stopped; alive_after = [bool](Get-Process -Id $appPid -ErrorAction SilentlyContinue) }
}
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDir 'original-probe.json') -Encoding UTF8
$result | ConvertTo-Json -Depth 8
