param(
  [Parameter(Mandatory = $true)][int]$ProcessId,
  [Parameter(Mandatory = $true)][string]$OutDir
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class Nat {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, IntPtr extra);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  public static void ForceForeground(IntPtr h) {
    IntPtr fg = GetForegroundWindow();
    uint a; uint b;
    GetWindowThreadProcessId(fg, out a);
    b = GetCurrentThreadId();
    AttachThreadInput(b, a, true);
    SetWindowPos(h, (IntPtr)(-1), 0, 0, 0, 0, 0x0043);
    ShowWindow(h, 9);
    SetForegroundWindow(h);
    AttachThreadInput(b, a, false);
  }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  public static IntPtr Find(uint target) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == target && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
'@

function Get-Hwnd {
  param([uint32]$TargetPid)
  for ($i = 0; $i -lt 30; $i++) {
    $h = [Nat]::Find($TargetPid)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 500
  }
  throw "window not found for pid $TargetPid"
}

function Capture {
  param([IntPtr]$Hwnd, [string]$Path)
  $r = New-Object Nat+RECT
  [Nat]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left
  $h = $r.Bottom - $r.Top
  if ($w -le 0 -or $h -le 0) { throw "bad rect ${w}x${h}" }
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output "saved $Path (${w}x${h})"
}

$hwnd = Get-Hwnd -TargetPid ([uint32]$ProcessId)
[Nat]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0043) | Out-Null
Start-Sleep -Milliseconds 1500

$table = Join-Path $OutDir 't01_table.png'
Capture -Hwnd $hwnd -Path $table

# focus table center and select all
$r = New-Object Nat+RECT
[Nat]::GetWindowRect($hwnd, [ref]$r) | Out-Null
$cx = [int](($r.Left + $r.Right) / 2)
$cy = [int](($r.Top + $r.Bottom) / 2)
[Nat]::SetCursorPos($cx, $cy) | Out-Null
[Nat]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)  # left down
[Nat]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)  # left up
Start-Sleep -Milliseconds 600
[Nat]::ForceForeground($hwnd)
Start-Sleep -Milliseconds 400
[Nat]::keybd_event(0x11, 0x1D, 0, [IntPtr]::Zero)          # Ctrl down
[Nat]::keybd_event(0x41, 0x1E, 0, [IntPtr]::Zero)          # A down
Start-Sleep -Milliseconds 120
[Nat]::keybd_event(0x41, 0x1E, 0x0002, [IntPtr]::Zero)     # A up
[Nat]::keybd_event(0x11, 0x1D, 0x0002, [IntPtr]::Zero)     # Ctrl up
Start-Sleep -Milliseconds 800
$sel = Join-Path $OutDir 't01_table_select_all.png'
Capture -Hwnd $hwnd -Path $sel

# scroll down with the wheel
[Nat]::SetCursorPos($cx, $cy) | Out-Null
for ($i = 0; $i -lt 12; $i++) {
  [Nat]::mouse_event(0x0800, 0, 0, -120, [IntPtr]::Zero)  # wheel down
  Start-Sleep -Milliseconds 80
}
Start-Sleep -Milliseconds 800
$scrolled = Join-Path $OutDir 't01_table_scrolled.png'
Capture -Hwnd $hwnd -Path $scrolled

[Nat]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, 0x0043) | Out-Null
Write-Output 'capture complete'
