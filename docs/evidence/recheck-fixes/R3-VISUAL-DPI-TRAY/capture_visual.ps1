# R3-VISUAL-DPI-TRAY real-window capture at the host DPI.
#
# Launches the packaged release from the isolated data dir (SysProxyType=
# Unchanged, LocalPort=11808), captures the main window and the full primary
# screen (so the taskbar notification/tray area is included), records the host
# and per-window DPI, then stops only our own PID/descendants.
#
# Run once per theme:
#   pwsh -File capture_visual.ps1 -Exe <release exe> -RunDir <release dir> `
#        -DataDir <isolated data> -OutDir <evidence out> -Theme light
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$RunDir,
  [Parameter(Mandatory = $true)][string]$DataDir,
  [Parameter(Mandatory = $true)][string]$OutDir,
  [Parameter(Mandatory = $true)][string]$Theme
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class VisWin {
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
  [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static string Title(IntPtr h) { var sb = new System.Text.StringBuilder(512); GetWindowText(h, sb, sb.Capacity); return sb.ToString(); }
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
$keys = 'V2RAYN_R_OPEN_SETTINGS', 'V2RAYN_R_OPEN_ROUTING', 'V2RAYN_R_THEME', 'V2RAYN_R_AUTOSTART', 'V2RAYN_R_AUTO_SMOKE', 'V2RAYN_R_DATA_DIR'
foreach ($k in $keys) { Remove-Item -Path ("Env:" + $k) -ErrorAction SilentlyContinue }
$env:V2RAYN_R_DATA_DIR = $DataDir
$env:V2RAYN_R_AUTOSTART = '0'
$env:V2RAYN_R_AUTO_SMOKE = '0'
$env:V2RAYN_R_THEME = $Theme

$entry = [ordered]@{ theme = $Theme; host_dpi = [VisWin]::GetDpiForSystem(); pid = $null; window = $null; main_shot = $null; screen_shot = $null; error = $null; stopped = @(); alive_after = $null }
$proc = Start-Process -FilePath $Exe -WorkingDirectory $RunDir -PassThru
$appPid = $proc.Id
$entry.pid = $appPid
$descBefore = @(Get-DescendantPids -RootPid $appPid)
try {
  $hwnd = [IntPtr]::Zero
  for ($i = 0; $i -lt 60 -and $hwnd -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Milliseconds 400
    $hwnd = [VisWin]::Find([uint32]$appPid)
    if ($proc.HasExited) { throw "exited early code=$($proc.ExitCode)" }
  }
  if ($hwnd -eq [IntPtr]::Zero) { throw "window not found" }
  [VisWin]::SetWindowPos($hwnd, [IntPtr]::Zero, 40, 40, 1200, 800, 0x0040) | Out-Null
  Start-Sleep -Milliseconds 6000
  $hwnd = [VisWin]::Find([uint32]$appPid)
  $r = New-Object VisWin+RECT; [VisWin]::GetWindowRect($hwnd, [ref]$r) | Out-Null
  $entry.window = [ordered]@{ title = [VisWin]::Title($hwnd); rect = "$($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"; dpi = [VisWin]::GetDpiForWindow($hwnd) }

  [VisWin]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 700
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $main = Join-Path $OutDir ("main-" + $Theme + ".png")
  $bmp.Save($main, [System.Drawing.Imaging.ImageFormat]::Png); $g.Dispose(); $bmp.Dispose()
  $entry.main_shot = $main

  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $sbmp = New-Object System.Drawing.Bitmap($b.Width, $b.Height)
  $sg = [System.Drawing.Graphics]::FromImage($sbmp)
  $sg.CopyFromScreen($b.X, $b.Y, 0, 0, (New-Object System.Drawing.Size($b.Width, $b.Height)))
  $screen = Join-Path $OutDir ("desktop-" + $Theme + ".png")
  $sbmp.Save($screen, [System.Drawing.Imaging.ImageFormat]::Png); $sg.Dispose(); $sbmp.Dispose()
  $entry.screen_shot = $screen
  $entry.screen_bounds = "$($b.Width)x$($b.Height)"
}
catch { $entry.error = "$_" }
finally {
  $descNow = @(Get-DescendantPids -RootPid $appPid)
  $owned = @($appPid) + $descBefore + @($descNow) | Sort-Object -Unique; [array]::Reverse($owned)
  $stopped = @(); foreach ($cp in $owned) { if (Get-Process -Id $cp -ErrorAction SilentlyContinue) { Stop-Process -Id $cp -Force -ErrorAction SilentlyContinue; $stopped += $cp } }
  $entry.stopped = $stopped; $entry.alive_after = [bool](Get-Process -Id $appPid -ErrorAction SilentlyContinue)
  foreach ($k in $keys) { Remove-Item -Path ("Env:" + $k) -ErrorAction SilentlyContinue }
}
$entry | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDir ("probe-" + $Theme + ".json")) -Encoding UTF8
$entry | ConvertTo-Json -Depth 8
