# R3-VISUAL-DPI-TRAY: open the notification-area overflow and right-click the
# live app tray icon (the gray ring, first icon in the flyout) to capture the
# context menu. No menu item is clicked, so no proxy/core action runs. Only our
# own process/descendants are stopped.
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$RunDir,
  [Parameter(Mandatory = $true)][string]$DataDir,
  [Parameter(Mandatory = $true)][string]$OutDir,
  [int]$IconX = 1593,
  [int]$IconY = 838
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class TrayMenuWin {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  public static void Move(int x, int y) { SetCursorPos(x, y); }
  public static void LeftClick(int x, int y) { SetCursorPos(x, y); System.Threading.Thread.Sleep(150); mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero); mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero); }
  public static void RightClick(int x, int y) { SetCursorPos(x, y); System.Threading.Thread.Sleep(150); mouse_event(0x0008, 0, 0, 0, UIntPtr.Zero); mouse_event(0x0010, 0, 0, 0, UIntPtr.Zero); }
  public static IntPtr Find(uint target) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { uint pid; GetWindowThreadProcessId(h, out pid); if (pid == target && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
}
'@

function Get-DescendantPids { param([int]$RootPid)
  $result = [System.Collections.Generic.List[int]]::new(); $queue = [System.Collections.Generic.Queue[int]]::new(); $queue.Enqueue($RootPid)
  while ($queue.Count -gt 0) { $parent = $queue.Dequeue(); $children = Get-CimInstance Win32_Process -Filter "ParentProcessId=$parent" -ErrorAction SilentlyContinue
    foreach ($c in $children) { $result.Add([int]$c.ProcessId) | Out-Null; $queue.Enqueue([int]$c.ProcessId) } }
  return $result
}

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$keys = 'V2RAYN_R_THEME', 'V2RAYN_R_AUTOSTART', 'V2RAYN_R_AUTO_SMOKE', 'V2RAYN_R_DATA_DIR'
foreach ($k in $keys) { Remove-Item -Path ("Env:" + $k) -ErrorAction SilentlyContinue }
$env:V2RAYN_R_DATA_DIR = $DataDir
$env:V2RAYN_R_AUTOSTART = '0'
$env:V2RAYN_R_AUTO_SMOKE = '0'
$env:V2RAYN_R_THEME = 'light'

$entry = [ordered]@{ pid = $null; error = $null; menu_shot = $null; tooltip_shot = $null; stopped = @(); alive_after = $null }
$proc = Start-Process -FilePath $Exe -WorkingDirectory $RunDir -PassThru
$appPid = $proc.Id
$entry.pid = $appPid
$descBefore = @(Get-DescendantPids -RootPid $appPid)
try {
  for ($i = 0; $i -lt 40 -and ([TrayMenuWin]::Find([uint32]$appPid) -eq [IntPtr]::Zero); $i++) { Start-Sleep -Milliseconds 400 }
  Start-Sleep -Milliseconds 5000
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  # Open the overflow flyout.
  [TrayMenuWin]::LeftClick([int]($b.Width - 248), [int]($b.Height - 27))
  Start-Sleep -Milliseconds 1500
  # Hover our icon for its tooltip.
  [TrayMenuWin]::Move($IconX, $IconY)
  Start-Sleep -Milliseconds 2200
  $sbmp = New-Object System.Drawing.Bitmap($b.Width, $b.Height)
  $sg = [System.Drawing.Graphics]::FromImage($sbmp)
  $sg.CopyFromScreen($b.X, $b.Y, 0, 0, (New-Object System.Drawing.Size($b.Width, $b.Height)))
  $tip = Join-Path $OutDir 'desktop-tray-tooltip-light.png'
  $sbmp.Save($tip, [System.Drawing.Imaging.ImageFormat]::Png); $sg.Dispose(); $sbmp.Dispose()
  $entry.tooltip_shot = $tip
  # Right-click for the app context menu.
  [TrayMenuWin]::RightClick($IconX, $IconY)
  Start-Sleep -Milliseconds 1500
  $sbmp2 = New-Object System.Drawing.Bitmap($b.Width, $b.Height)
  $sg2 = [System.Drawing.Graphics]::FromImage($sbmp2)
  $sg2.CopyFromScreen($b.X, $b.Y, 0, 0, (New-Object System.Drawing.Size($b.Width, $b.Height)))
  $menu = Join-Path $OutDir 'desktop-tray-menu-light.png'
  $sbmp2.Save($menu, [System.Drawing.Imaging.ImageFormat]::Png); $sg2.Dispose(); $sbmp2.Dispose()
  $entry.menu_shot = $menu
  [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
  Start-Sleep -Milliseconds 500
}
catch { $entry.error = "$_" }
finally {
  $descNow = @(Get-DescendantPids -RootPid $appPid)
  $owned = @($appPid) + $descBefore + @($descNow) | Sort-Object -Unique; [array]::Reverse($owned)
  $stopped = @(); foreach ($cp in $owned) { if (Get-Process -Id $cp -ErrorAction SilentlyContinue) { Stop-Process -Id $cp -Force -ErrorAction SilentlyContinue; $stopped += $cp } }
  $entry.stopped = $stopped; $entry.alive_after = [bool](Get-Process -Id $appPid -ErrorAction SilentlyContinue)
  foreach ($k in $keys) { Remove-Item -Path ("Env:" + $k) -ErrorAction SilentlyContinue }
}
$entry | ConvertTo-Json -Depth 8
