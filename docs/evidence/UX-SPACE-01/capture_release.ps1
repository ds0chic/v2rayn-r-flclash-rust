# UX-SPACE-01 release-window capture.
# Launches the built release exe with an isolated data dir, resizes the OS
# window, screenshots it, then stops ONLY that PID. No mouse/keyboard input.
# Enforces a 120s wall-clock budget per launch.
param(
  [string]$Exe = "C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn\apps\desktop\build\windows\x64\runner\Release\v2rayn_desktop.exe",
  [string]$OutDir = "C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn\docs\evidence\UX-SPACE-01\real-window",
  [int]$Width = 1200,
  [int]$Height = 800,
  [string]$Layout = "vertical",
  [string]$Theme = "light",
  [string]$Name = "08-release-1200x800-vertical-light"
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
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

$root = "C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn"
$dataDir = "$root/target/ux-space01-release-data"
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$env:V2RAYN_R_DATA_DIR = $dataDir
$env:V2RAYN_R_LAYOUT = $Layout
$env:V2RAYN_R_THEME = $Theme
$env:V2RAYN_R_AUTOSTART = '0'
$env:V2RAYN_R_AUTO_SMOKE = '0'

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe) -PassThru
try {
  $hwnd = [IntPtr]::Zero
  for ($i = 0; $i -lt 40 -and $hwnd -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Milliseconds 400
    $hwnd = [W]::Find([uint32]$proc.Id)
    if ($sw.Elapsed.TotalSeconds -gt 120) { throw "window timeout >120s" }
  }
  if ($hwnd -eq [IntPtr]::Zero) { throw "window not found for pid $($proc.Id)" }
  [W]::SetWindowPos($hwnd, [IntPtr]::Zero, 40, 40, $Width, $Height, 0x0040) | Out-Null
  Start-Sleep -Milliseconds 2500
  $r = New-Object W+RECT
  [W]::GetWindowRect($hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $png = Join-Path $OutDir "$Name.png"
  $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  $dpi = [W]::GetDpiForWindow($hwnd)
  $sw.Stop()
  $record = [ordered]@{
    name = $Name; pid = $proc.Id; width = $w; height = $h
    layout = $Layout; theme = $Theme; dpi = $dpi
    durationMs = [int]$sw.Elapsed.TotalMilliseconds; png = $png
  } | ConvertTo-Json
  Add-Content -LiteralPath (Join-Path $OutDir 'release-captures.json') -Value $record
  Write-Output $record
}
finally {
  if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
