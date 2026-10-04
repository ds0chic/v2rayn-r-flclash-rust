# R3-WPF-COMPARE original v2rayN settings/routing window capture.
# Isolation: temp run dir is the data dir; guiNConfig.json is pre-seeded with
# SystemProxyType=Unchanged; the process is force-killed so AppExitAsync never
# runs. Only our own PID/descendants are stopped. Menu items are opened with
# real synthetic mouse clicks on our own isolated window (UIA Invoke is not
# reliable for WPF submenu items).
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$RunDir,
  [Parameter(Mandatory = $true)][string]$OutDir,
  [string]$MenuSetting = '设置'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class WpfWin3 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static string Title(IntPtr h) { var sb = new System.Text.StringBuilder(512); GetWindowText(h, sb, sb.Capacity); return sb.ToString(); }
  public static IntPtr[] List(uint target) {
    var list = new System.Collections.Generic.List<IntPtr>();
    EnumWindows((h, l) => { uint pid; GetWindowThreadProcessId(h, out pid); if (pid == target && IsWindowVisible(h)) list.Add(h); return true; }, IntPtr.Zero);
    return list.ToArray();
  }
  public static void Click(int x, int y) { SetCursorPos(x, y); System.Threading.Thread.Sleep(60); mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(40); mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero); }
}
'@

function Get-DescendantPids { param([int]$RootPid)
  $result = [System.Collections.Generic.List[int]]::new(); $queue = [System.Collections.Generic.Queue[int]]::new(); $queue.Enqueue($RootPid)
  while ($queue.Count -gt 0) { $parent = $queue.Dequeue(); $children = Get-CimInstance Win32_Process -Filter "ParentProcessId=$parent" -ErrorAction SilentlyContinue
    foreach ($c in $children) { $result.Add([int]$c.ProcessId) | Out-Null; $queue.Enqueue([int]$c.ProcessId) } }
  return $result
}
function Save-WindowShot { param([IntPtr]$Hwnd, [string]$Path)
  [WpfWin3]::SetForegroundWindow($Hwnd) | Out-Null; Start-Sleep -Milliseconds 600
  $r = New-Object WpfWin3+RECT; [WpfWin3]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top; if ($w -le 0 -or $h -le 0) { return $false }
  $bmp = New-Object System.Drawing.Bitmap($w, $h); $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png); $g.Dispose(); $bmp.Dispose(); return $true
}
function Click-Element { param($El)
  $r = $El.Current.BoundingRectangle
  if ($r.IsEmpty) { throw "element has empty bounds" }
  $x = [int]($r.Left + $r.Width / 2); $y = [int]($r.Top + $r.Height / 2)
  [WpfWin3]::Click($x, $y)
}
function Find-ByAid { param([string]$Aid)
  $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Aid)
  return [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}
function Open-SettingWindow { param([string]$ItemAid, [int]$ExpectIndex)
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.AndCondition(
    (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $MenuSetting)),
    (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::MenuItem)))
  $parent = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
  if ($null -eq $parent) { throw "settings menu not found" }
  Click-Element $parent; Start-Sleep -Milliseconds 1000
  $item = Find-ByAid $ItemAid
  if ($null -eq $item) { throw "submenu item not found: $ItemAid" }
  Click-Element $item; Start-Sleep -Milliseconds 400
}
function Wait-NewWindow { param([uint32]$ProcId, [IntPtr]$Main, [int]$TimeoutMs = 8000)
  $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
  while ((Get-Date) -lt $deadline) {
    foreach ($h in [WpfWin3]::List($ProcId)) { if ($h -ne $Main -and [WpfWin3]::Title($h).Length -gt 0) { return $h } }
    Start-Sleep -Milliseconds 250
  }
  return [IntPtr]::Zero
}
function Close-Window { param([IntPtr]$Hwnd)
  try { $el = [System.Windows.Automation.AutomationElement]::FromHandle($Hwnd); $wp = $null
    if ($el.TryGetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern, [ref]$wp)) { $wp.Close(); return } } catch {}
  [WpfWin3]::PostMessage($Hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
}

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$proc = Start-Process -FilePath $Exe -WorkingDirectory $RunDir -PassThru
$appPid = $proc.Id
$pidU32 = [uint32]$appPid
$descBefore = @(Get-DescendantPids -RootPid $appPid)
$result = [ordered]@{ pid = $appPid; shots = @(); windows = @(); steps = @(); error = $null }
try {
  $main = $null
  for ($i = 0; $i -lt 50; $i++) { Start-Sleep -Milliseconds 400; $wins = @([WpfWin3]::List($pidU32)); if ($wins.Count -gt 0) { $main = $wins[0]; break }; if ($proc.HasExited) { throw "exited early" } }
  if ($null -eq $main) { throw "main window not found" }
  Start-Sleep -Seconds 3
  [WpfWin3]::SetForegroundWindow($main) | Out-Null

  $p = Join-Path $OutDir 'original-main.png'; if (Save-WindowShot -Hwnd $main -Path $p) { $result.shots += $p }
  $r = New-Object WpfWin3+RECT; [WpfWin3]::GetWindowRect($main, [ref]$r) | Out-Null
  $result.windows += [ordered]@{ name='main'; title=[WpfWin3]::Title($main); rect="$($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"; dpi=[WpfWin3]::GetDpiForWindow($main) }

  Open-SettingWindow -ItemAid 'menuOptionSetting' -ExpectIndex 1
  $w = Wait-NewWindow -ProcId $pidU32 -Main $main
  if ($w -ne [IntPtr]::Zero) {
    Start-Sleep -Milliseconds 1000
    $p = Join-Path $OutDir 'original-option.png'; if (Save-WindowShot -Hwnd $w -Path $p) { $result.shots += $p }
    $r2 = New-Object WpfWin3+RECT; [WpfWin3]::GetWindowRect($w, [ref]$r2) | Out-Null
    $result.windows += [ordered]@{ name='option'; title=[WpfWin3]::Title($w); rect="$($r2.Left),$($r2.Top),$($r2.Right),$($r2.Bottom)"; dpi=[WpfWin3]::GetDpiForWindow($w) }
    $result.steps += "option hwnd=$($w.ToInt64())"
    Close-Window -Hwnd $w; Start-Sleep -Milliseconds 1500
  } else { $result.steps += "option window not found" }

  Open-SettingWindow -ItemAid 'menuRoutingSetting' -ExpectIndex 2
  $w = Wait-NewWindow -ProcId $pidU32 -Main $main
  if ($w -ne [IntPtr]::Zero) {
    Start-Sleep -Milliseconds 1000
    $p = Join-Path $OutDir 'original-routing.png'; if (Save-WindowShot -Hwnd $w -Path $p) { $result.shots += $p }
    $r3 = New-Object WpfWin3+RECT; [WpfWin3]::GetWindowRect($w, [ref]$r3) | Out-Null
    $result.windows += [ordered]@{ name='routing'; title=[WpfWin3]::Title($w); rect="$($r3.Left),$($r3.Top),$($r3.Right),$($r3.Bottom)"; dpi=[WpfWin3]::GetDpiForWindow($w) }
    $result.steps += "routing hwnd=$($w.ToInt64())"
    Close-Window -Hwnd $w; Start-Sleep -Milliseconds 800
  } else { $result.steps += "routing window not found" }
}
catch { $result.error = "$_" }
finally {
  $descNow = @(Get-DescendantPids -RootPid $appPid)
  $owned = @($appPid) + $descBefore + @($descNow) | Sort-Object -Unique; [array]::Reverse($owned)
  $stopped = @(); foreach ($cp in $owned) { if (Get-Process -Id $cp -ErrorAction SilentlyContinue) { Stop-Process -Id $cp -Force -ErrorAction SilentlyContinue; $stopped += $cp } }
  $result.cleanup = [ordered]@{ stopped = $stopped; alive_after = [bool](Get-Process -Id $appPid -ErrorAction SilentlyContinue) }
}
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDir 'original-windows-probe.json') -Encoding UTF8
$result | ConvertTo-Json -Depth 8
