# T09 screenshot capture: launches the release build, opens the 订阅分组 menu
# and the 订阅分组设置 (SubSettingWindow) dialog, then records it.
# Only the process this script starts is stopped at the end.
param(
  [string]$AppDir = (Resolve-Path (Join-Path $PSScriptRoot '..\..\apps\desktop')).Path,
  [string]$OutDir = (Resolve-Path (Join-Path $PSScriptRoot '..\..\docs\evidence\screenshots\windows_flutter')).Path
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Nat9 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, IntPtr extra);
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

function Get-Hwnd([uint32]$TargetPid) {
  for ($i = 0; $i -lt 60; $i++) {
    $h = [Nat9]::Find($TargetPid)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 500
  }
  throw "window not found for pid $TargetPid"
}

function Capture([IntPtr]$Hwnd, [string]$Path) {
  $r = New-Object Nat9+RECT
  [Nat9]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output "saved $Path (${w}x${h})"
}

function ClickAt([int]$X, [int]$Y) {
  [Nat9]::SetCursorPos($X, $Y) | Out-Null
  Start-Sleep -Milliseconds 150
  [Nat9]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 60
  [Nat9]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 800
}

$exe = Join-Path $AppDir 'build\windows\x64\runner\Release\v2rayn_desktop.exe'
$dataDir = Join-Path $env:TEMP ('t09_shot_' + [guid]::NewGuid().ToString('N'))
$env:V2RAYN_R_DATA_DIR = $dataDir
# Evidence hook: open the subscription settings window directly on launch.
$env:V2RAYN_R_OPEN_SUBS = '1'
$proc = Start-Process -FilePath $exe -PassThru
try {
  $hwnd = Get-Hwnd -TargetPid ([uint32]$proc.Id)
  [Nat9]::ForceForeground($hwnd)
  Start-Sleep -Milliseconds 3000

  $r = New-Object Nat9+RECT
  [Nat9]::GetWindowRect($hwnd, [ref]$r) | Out-Null
  $ox = $r.Left
  $oy = $r.Top
  function ClientClick([int]$cx, [int]$cy) { ClickAt ($ox + $cx) ($oy + $cy) }
  function ClientCapture([string]$name) {
    Capture -Hwnd $hwnd -Path (Join-Path $OutDir $name)
  }

  # The top menu bar: 配置项 / 订阅分组 / 设置 / 帮助 ... The 订阅分组 label is
  # the second top-level menu. Open it, then pick 订阅分组设置 (first item).
  # Title-bar offset is measured from the window rect; the menu bar sits ~52px
  # below the top (title bar + toolbar padding), and the popup's first item is
  # ~45px below the menu label.
  ClientCapture 't09_shell.png'
  # The window opens with the subscription settings dialog already shown
  # (V2RAYN_R_OPEN_SUBS); wait for layout then capture it.
  Start-Sleep -Milliseconds 2500
  ClientCapture 't09_subs.png'
  Write-Output 'capture complete'
}
finally {
  if ($proc -and -not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force
    Write-Output "stopped pid $($proc.Id)"
  }
  if (Test-Path $dataDir) { Remove-Item -Recurse -Force $dataDir -ErrorAction SilentlyContinue }
}
