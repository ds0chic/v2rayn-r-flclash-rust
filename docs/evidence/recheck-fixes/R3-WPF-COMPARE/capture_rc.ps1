# R3-WPF-COMPARE RC (Flutter v2rayn_desktop.exe) isolated capture.
# Launches the packaged RC from a temp copy with V2RAYN_R_DATA_DIR pointing to a
# temp data dir (pre-seeded guiNConfig.json, LocalPort=11808, SysProxyType=
# Unchanged). Only our own PID/descendants are stopped. Three launches:
#   main, V2RAYN_R_OPEN_SETTINGS=1, V2RAYN_R_OPEN_ROUTING=1.
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$RunDir,
  [Parameter(Mandatory = $true)][string]$DataDir,
  [Parameter(Mandatory = $true)][string]$OutDir
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class RcWin {
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
function Save-WindowShot { param([IntPtr]$Hwnd, [string]$Path)
  [RcWin]::SetForegroundWindow($Hwnd) | Out-Null; Start-Sleep -Milliseconds 700
  $r = New-Object RcWin+RECT; [RcWin]::GetWindowRect($Hwnd, [ref]$r) | Out-Null
  $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top; if ($w -le 0 -or $h -le 0) { return $false }
  $bmp = New-Object System.Drawing.Bitmap($w, $h); $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png); $g.Dispose(); $bmp.Dispose(); return $true
}

$keys = 'V2RAYN_R_OPEN_SETTINGS', 'V2RAYN_R_OPEN_ROUTING', 'V2RAYN_R_THEME', 'V2RAYN_R_AUTOSTART', 'V2RAYN_R_AUTO_SMOKE', 'V2RAYN_R_DATA_DIR'
$runs = @(
  [ordered]@{ name = 'rc-main'; extra = @{} },
  [ordered]@{ name = 'rc-option'; extra = @{ V2RAYN_R_OPEN_SETTINGS = '1' } },
  [ordered]@{ name = 'rc-routing'; extra = @{ V2RAYN_R_OPEN_ROUTING = '1' } }
)
$report = [System.Collections.Generic.List[object]]::new()
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

foreach ($run in $runs) {
  foreach ($k in $keys) { Remove-Item -Path ("Env:" + $k) -ErrorAction SilentlyContinue }
  $env:V2RAYN_R_DATA_DIR = $DataDir
  $env:V2RAYN_R_AUTOSTART = '0'
  $env:V2RAYN_R_AUTO_SMOKE = '0'
  $env:V2RAYN_R_THEME = 'light'
  foreach ($kv in $run.extra.GetEnumerator()) { Set-Item -Path ("Env:" + $kv.Key) -Value $kv.Value }
  if ($run.name -eq 'rc-option') { $env:V2RAYN_R_OPEN_SETTINGS = '1' }
  if ($run.name -eq 'rc-routing') { $env:V2RAYN_R_OPEN_ROUTING = '1' }

  $proc = Start-Process -FilePath $Exe -WorkingDirectory $RunDir -PassThru
  $appPid = $proc.Id
  $descBefore = @(Get-DescendantPids -RootPid $appPid)
  $entry = [ordered]@{ name = $run.name; pid = $appPid; extra = $run.extra; shot = $null; window = $null; error = $null; stopped = @(); alive_after = $null }
  try {
    $hwnd = [IntPtr]::Zero
    for ($i = 0; $i -lt 60 -and $hwnd -eq [IntPtr]::Zero; $i++) {
      Start-Sleep -Milliseconds 400
      $hwnd = [RcWin]::Find([uint32]$appPid)
      if ($proc.HasExited) { throw "exited early code=$($proc.ExitCode)" }
    }
    if ($hwnd -eq [IntPtr]::Zero) { throw "window not found" }
    [RcWin]::SetWindowPos($hwnd, [IntPtr]::Zero, 40, 40, 1200, 800, 0x0040) | Out-Null
    $settle = 5000
    Start-Sleep -Milliseconds $settle
    $hwnd = [RcWin]::Find([uint32]$appPid)
    $r = New-Object RcWin+RECT; [RcWin]::GetWindowRect($hwnd, [ref]$r) | Out-Null
    $entry.window = [ordered]@{ title = [RcWin]::Title($hwnd); rect = "$($r.Left),$($r.Top),$($r.Right),$($r.Bottom)"; dpi = [RcWin]::GetDpiForWindow($hwnd) }
    $shot = Join-Path $OutDir ($run.name + '.png')
    if (Save-WindowShot -Hwnd $hwnd -Path $shot) { $entry.shot = $shot }
  }
  catch { $entry.error = "$_" }
  finally {
    $descNow = @(Get-DescendantPids -RootPid $appPid)
    $owned = @($appPid) + $descBefore + @($descNow) | Sort-Object -Unique; [array]::Reverse($owned)
    $stopped = @(); foreach ($cp in $owned) { if (Get-Process -Id $cp -ErrorAction SilentlyContinue) { Stop-Process -Id $cp -Force -ErrorAction SilentlyContinue; $stopped += $cp } }
    $entry.stopped = $stopped; $entry.alive_after = [bool](Get-Process -Id $appPid -ErrorAction SilentlyContinue)
    foreach ($k in $keys) { Remove-Item -Path ("Env:" + $k) -ErrorAction SilentlyContinue }
  }
  $report.Add($entry)
}
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDir 'rc-windows-probe.json') -Encoding UTF8
$report | ConvertTo-Json -Depth 8
