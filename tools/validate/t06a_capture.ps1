# T06a screenshot capture: launches the release build, opens the Add-node
# dialog and records the editor for VMess / VLESS / WireGuard.
# Only the process this script starts is stopped at the end.
param(
  [string]$AppDir = (Resolve-Path (Join-Path $PSScriptRoot '..\..\apps\desktop')).Path,
  [string]$OutDir = (Resolve-Path (Join-Path $PSScriptRoot '..\..\docs\evidence\screenshots\windows_flutter')).Path
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
    $h = [Nat]::Find($TargetPid)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 500
  }
  throw "window not found for pid $TargetPid"
}

function Capture([IntPtr]$Hwnd, [string]$Path) {
  $r = New-Object Nat+RECT
  [Nat]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output "saved $Path (${w}x${h})"
}

function ClickAt([int]$X, [int]$Y) {
  [Nat]::SetCursorPos($X, $Y) | Out-Null
  Start-Sleep -Milliseconds 150
  [Nat]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 60
  [Nat]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 800
}

$exe = Join-Path $AppDir 'build\windows\x64\runner\Release\v2rayn_desktop.exe'
$env:V2RAYN_R_DATA_DIR = Join-Path $env:TEMP ('t06a_shot_' + [guid]::NewGuid().ToString('N'))
$proc = Start-Process -FilePath $exe -PassThru
try {
  $hwnd = Get-Hwnd -TargetPid ([uint32]$proc.Id)
  [Nat]::ForceForeground($hwnd)
  Start-Sleep -Milliseconds 3000

  # Coordinates are relative to the client area (measured at 1200x800).
  $r = New-Object Nat+RECT
  [Nat]::GetWindowRect($hwnd, [ref]$r) | Out-Null
  $ox = $r.Left
  $oy = $r.Top

  function ClientClick([int]$cx, [int]$cy) { ClickAt ($ox + $cx) ($oy + $cy) }
  function ClientCapture([string]$name) {
    Capture -Hwnd $hwnd -Path (Join-Path $OutDir $name)
  }

  # 1) Open the "添加" protocol popup (toolbar ~x=391,y=98).
  ClientCapture 't06a_addserver_menu.png'
  ClientClick 391 98
  Start-Sleep -Milliseconds 500
  ClientCapture 't06a_addserver_menupopup.png'

  # 2) Pick "vmess" in the popup (first entry under the toolbar).
  ClientClick 391 132
  Start-Sleep -Milliseconds 900
  ClientCapture 't06a_editor_vmess.png'

  # 3) Fill an ASCII remark (keyboard-layout independent) and capture input.
  ClientClick 600 268
  Start-Sleep -Milliseconds 300
  [System.Windows.Forms.SendKeys]::SendWait('T06a-node')
  Start-Sleep -Milliseconds 500
  ClientCapture 't06a_editor_remark.png'

  # 4) Open the protocol-type dropdown and switch to VLESS (second item).
  ClientClick 600 172
  Start-Sleep -Milliseconds 500
  ClientCapture 't06a_editor_protocol_menu.png'
  ClientClick 600 226
  Start-Sleep -Milliseconds 900
  ClientCapture 't06a_editor_vless.png'

  # 5) Scroll to the TLS/Reality section of the VLESS editor.
  $sx = $ox + 600
  $sy = $oy + 450
  [Nat]::SetCursorPos($sx, $sy) | Out-Null
  for ($i = 0; $i -lt 6; $i++) {
    [Nat]::mouse_event(0x0800, 0, 0, -120, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
  }
  Start-Sleep -Milliseconds 500
  ClientCapture 't06a_editor_vless_tls.png'

  Write-Output 'capture complete'
}
finally {
  if ($proc -and -not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force
    Write-Output "stopped pid $($proc.Id)"
  }
}
