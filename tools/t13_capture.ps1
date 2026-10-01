param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$OutFile,
  [string]$DataDir = '',
  [string]$EnvFile = '',
  [int]$WaitMs = 6000
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class Nat2 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
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
  for ($i = 0; $i -lt 40; $i++) {
    $h = [Nat2]::Find($TargetPid)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 500
  }
  throw "window not found for pid $TargetPid"
}

# Build environment for the child process.
$env2 = @{}
if ($DataDir -ne '') { $env2['V2RAYN_R_DATA_DIR'] = $DataDir }
if ($EnvFile -ne '') {
  foreach ($pair in ($EnvFile -split ';')) {
    if ($pair -match '=') {
      $kv = $pair -split '=', 2
      $env2[$kv[0]] = $kv[1]
    }
  }
}
$procArgs = @{ FilePath = $Exe }
if ($env2.Count -gt 0) {
  $old = @{}
  foreach ($k in $env2.Keys) { $old[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $env2[$k]) }
}
$proc = Start-Process -FilePath $Exe -PassThru
if ($env2.Count -gt 0) {
  foreach ($k in $env2.Keys) { [Environment]::SetEnvironmentVariable($k, $old[$k]) }
}

try {
  Start-Sleep -Milliseconds $WaitMs
  $hwnd = Get-Hwnd -TargetPid ([uint32]$proc.Id)
  [Nat2]::SetWindowPos($hwnd, [IntPtr](-1), 100, 100, 0, 0, 0x0043) | Out-Null
  Start-Sleep -Milliseconds 1200
  $r = New-Object Nat2+RECT
  [Nat2]::GetWindowRect($hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left
  $h = $r.Bottom - $r.Top
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($OutFile, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output "saved $OutFile (${w}x${h}) pid=$($proc.Id)"
}
finally {
  if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
