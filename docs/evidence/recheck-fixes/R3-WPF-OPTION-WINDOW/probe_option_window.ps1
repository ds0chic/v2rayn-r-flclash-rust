#requires -Version 7
<#
  R3-WPF-OPTION-WINDOW window probe.

  Launches the release package (from a temp copy, so it gets a fresh
  single-instance lock) in an isolated data directory with the
  V2RAYN_R_OPEN_SETTINGS evidence hook, then asserts the option settings UI is
  an independent, owned top-level HWND (its own Flutter engine) rather than an
  in-process dialog. Captures screenshots and a machine-readable probe JSON.

  Safety: only the process tree started here is stopped; no system proxy /
  registry / route / TUN change; no user credentials are read. The app is not
  asked to apply a plan, so no kernel listens on 10808.
#>
param(
  [string]$Exe = 'C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn\apps\desktop\build\windows\x64\runner\Release\v2rayn_desktop.exe',
  [string]$OutDir = $PSScriptRoot,
  [int]$WaitSeconds = 30
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class WinInfo {
  public long Hwnd;
  public long Owner;
  public string Title;
  public bool Visible;
  public bool Enabled;
  public int Left, Top, Right, Bottom;
}

public static class W32 {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hWnd, uint cmd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr hWnd, int index);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, IntPtr pid);
  [DllImport("user32.dll")] static extern bool AttachThreadInput(uint a, uint b, bool attach);
  [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();

  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  // Grant activation to a window even when the calling process is not the
  // current foreground owner, by temporarily attaching input threads.
  public static bool ForceForeground(IntPtr h) {
    IntPtr fg = GetForegroundWindow();
    uint fgThread = fg == IntPtr.Zero ? 0 : GetWindowThreadProcessId(fg, IntPtr.Zero);
    uint thisThread = GetCurrentThreadId();
    bool attached = false;
    if (fgThread != 0 && fgThread != thisThread) {
      attached = AttachThreadInput(thisThread, fgThread, true);
    }
    bool ok = SetForegroundWindow(h);
    if (attached) {
      AttachThreadInput(thisThread, fgThread, false);
    }
    return ok;
  }

  public static List<WinInfo> ForPid(int pid) {
    var list = new List<WinInfo>();
    EnumWindows(delegate(IntPtr h, IntPtr l) {
      uint wp;
      GetWindowThreadProcessId(h, out wp);
      if (wp == (uint)pid) {
        var sb = new StringBuilder(512);
        GetWindowText(h, sb, 512);
        RECT r;
        GetWindowRect(h, out r);
        var wi = new WinInfo();
        wi.Hwnd = h.ToInt64();
        wi.Owner = GetWindow(h, 4).ToInt64();
        wi.Title = sb.ToString();
        wi.Visible = IsWindowVisible(h);
        wi.Enabled = IsWindowEnabled(h);
        wi.Left = r.Left; wi.Top = r.Top; wi.Right = r.Right; wi.Bottom = r.Bottom;
        list.Add(wi);
      }
      return true;
    }, IntPtr.Zero);
    return list;
  }
}
'@

Add-Type -AssemblyName System.Drawing

# U+8BBE U+7F6E (设置) built from codepoints so it never depends on the
# encoding of this script file.
$expectedSettingsTitle = [string][char]0x8BBE + [string][char]0x7F6E

function Convert-Wins($raw) {
  $out = @()
  foreach ($w in $raw) {
    $out += [pscustomobject]@{
      hwnd      = ('0x{0:X}' -f $w.Hwnd)
      hwnd_raw  = $w.Hwnd
      title     = $w.Title
      visible   = $w.Visible
      enabled   = $w.Enabled
      owner     = if ($w.Owner -eq 0) { $null } else { '0x{0:X}' -f $w.Owner }
      owner_raw = $w.Owner
      style     = '0x{0:X8}' -f [W32]::GetWindowLong([IntPtr]$w.Hwnd, -16)
      rect      = @{ left = $w.Left; top = $w.Top; right = $w.Right; bottom = $w.Bottom }
    }
  }
  return $out
}

function Select-RealTopLevel($wins) {
  return @($wins | Where-Object { $_.visible -and $_.title -ne '' -and $_.title -notlike '*IME*' })
}

function Save-WindowShot([object]$win, [string]$path) {
  $r = $win.rect
  $w = $r.right - $r.left
  $h = $r.bottom - $r.top
  if ($w -le 0 -or $h -le 0) { return $false }
  $bmp = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.left, $r.top, 0, 0, $bmp.Size)
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  return $true
}

if (-not (Test-Path -LiteralPath $Exe)) { throw "exe not found: $Exe" }

# Run from an isolated copy: the single-instance mutex is keyed on the exe
# path, so a temp copy starts a fresh instance without touching any other
# running v2rayN-R process.
$bundle = Split-Path -Parent $Exe
$runDir = Join-Path $env:TEMP ("v2rayn-r-option-run-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runDir | Out-Null
Copy-Item -Path (Join-Path $bundle '*') -Destination $runDir -Recurse -Force
$runExe = Join-Path $runDir 'v2rayn_desktop.exe'

$dataDir = Join-Path $env:TEMP ("v2rayn-r-option-probe-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $dataDir | Out-Null

$env:V2RAYN_R_DATA_DIR = $dataDir
$env:V2RAYN_R_OPEN_SETTINGS = '1'
$started = Get-Date
$proc = Start-Process -FilePath $runExe -PassThru

$allWins = @()
$trace = @()
$deadline = (Get-Date).AddSeconds($WaitSeconds)
$main = $null
$settings = $null
while ((Get-Date) -lt $deadline) {
  if ($proc.HasExited) { break }
  Start-Sleep -Milliseconds 400
  $allWins = Convert-Wins ([W32]::ForPid($proc.Id))
  $real = Select-RealTopLevel $allWins
  $main = $real | Where-Object { $_.title -like 'v2rayN*' } | Select-Object -First 1
  $settings = $real | Where-Object { $_.hwnd_raw -ne ($main.hwnd_raw) -and $_.title -eq $expectedSettingsTitle } | Select-Object -First 1
  if ($trace.Count -lt 80) {
    $secs = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
    $trace += "[$secs] " + (($allWins | ForEach-Object { ($(if ($_.visible) { 'V' } else { 'h' })) + ':' + $_.title }) -join ' | ')
  }
  if ($null -ne $main -and $null -ne $settings) { break }
}
Start-Sleep -Milliseconds 800
$allWins = Convert-Wins ([W32]::ForPid($proc.Id))
$real = Select-RealTopLevel $allWins
$main = $real | Where-Object { $_.title -like 'v2rayN*' } | Select-Object -First 1
$settings = $real | Where-Object { $null -ne $main -and $_.hwnd_raw -ne $main.hwnd_raw } | Select-Object -First 1
$fg = [W32]::GetForegroundWindow().ToInt64()

# Focusability: our process is the foreground console, so granting activation
# to the settings HWND is permitted; this proves it is a real focusable
# top-level window. (Whether it happened to hold focus at the exact moment of
# creation is environment dependent and only recorded.)
$settingsFocusable = $false
if ($null -ne $settings) {
  [void][W32]::ForceForeground([IntPtr]$settings.hwnd_raw)
  Start-Sleep -Milliseconds 350
  $settingsFocusable = ([W32]::GetForegroundWindow().ToInt64() -eq $settings.hwnd_raw)
}

$exited = $proc.HasExited
$exitCode = if ($exited) { $proc.ExitCode } else { $null }

$mainShot = $false
$settingsShot = $false
if ($null -ne $main) { $mainShot = Save-WindowShot $main (Join-Path $OutDir 'probe-main.png') }
if ($null -ne $settings) { $settingsShot = Save-WindowShot $settings (Join-Path $OutDir 'probe-option.png') }

$topVisible = @($allWins | Where-Object { $_.visible -and $_.title -notlike '*IME*' })
$checks = [ordered]@{
  main_present           = ($null -ne $main)
  settings_present       = ($null -ne $settings)
  settings_title         = ($null -ne $settings -and $settings.title -eq $expectedSettingsTitle)
  independent_hwnd       = ($null -ne $main -and $null -ne $settings -and $main.hwnd_raw -ne $settings.hwnd_raw)
  settings_owned_by_main = ($null -ne $main -and $null -ne $settings -and $settings.owner_raw -eq $main.hwnd_raw)
  main_disabled_modal    = ($null -ne $main -and -not $main.enabled)
  settings_enabled       = ($null -ne $settings -and $settings.enabled)
  two_visible_top_level  = ($topVisible.Count -eq 2)
  main_screenshot        = $mainShot
  settings_screenshot    = $settingsShot
  # Recorded, not a pass gate: a terminal that owns the foreground can deny a
  # launched process activation, so this is environment dependent. The modal
  # owner relationship (main disabled) is the focus/ownership guarantee.
  settings_foregroundable = $settingsFocusable
}
$hardChecks = @(
  'main_present', 'settings_present', 'settings_title', 'independent_hwnd',
  'settings_owned_by_main', 'main_disabled_modal', 'settings_enabled',
  'two_visible_top_level', 'main_screenshot', 'settings_screenshot'
)
$passed = -not (@($hardChecks | Where-Object { -not $checks[$_] }).Count -gt 0)

$probe = [pscustomobject]@{
  session = [pscustomobject]@{
    pid = $proc.Id; exe = $Exe; run_exe = $runExe; run_dir = $runDir
    data_dir = $dataDir
    started = $started.ToString('o'); exited_early = $exited; exit_code = $exitCode
  }
  windows = $allWins
  trace = $trace
  foreground_raw = $fg
  checks = $checks
  passed = $passed
}
$probe | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutDir 'probe-option-window.json')

# Stop only the process tree started here, by PID, after recording it.
$children = @()
try {
  $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ProcessId)
} catch {}
foreach ($c in $children) { Stop-Process -Id $c -Force -ErrorAction SilentlyContinue }
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 300
$alive = $null -ne (Get-Process -Id $proc.Id -ErrorAction SilentlyContinue)
$probe.session | Add-Member -NotePropertyName stopped -NotePropertyValue $true
$probe.session | Add-Member -NotePropertyName alive_after -NotePropertyValue $alive
$probe | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $OutDir 'probe-option-window.json')
Remove-Item -LiteralPath $runDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Output ($probe | ConvertTo-Json -Depth 8)
if (-not $passed) { exit 2 }
