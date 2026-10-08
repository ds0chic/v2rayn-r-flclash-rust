<#
.SYNOPSIS
  Reusable desktop UI automation layer for agents (dot-sourceable, no side effects on import).

.DESCRIPTION
  Entry point: dot-source this file, then call the exported functions:
    . tools/ui/UiAutomation.ps1

  What this module does:
    - Finds top-level windows by title regex / owning PID / HWND (with timeout + retries).
    - Locates elements with a fixed selector priority:
        (1) UIA AutomationId / Name / ControlType (other apps / future engine),
        (2) SEMANTICS SNAPSHOT file (JSON written by our armed app hook:
            nodes with identifier, label, rect), matched by -Identifier / -Label,
        (3) raw -X / -Y screen coordinates (explicit opt-in only).
    - Performs REAL input through SendInput only (mouse move + down/up with
      correct button and double-click timing; keyboard via virtual-key and
      unicode input). NEVER UIA InvokePattern, NEVER any app API.
    - Captures window-rect PNGs (same approach as
      tools/acceptance/sp30_ui_hooks_capture.ps1: GetWindowRect + CopyFromScreen).

  Contract shared by EVERY public function:
    - Returns a structured object: { ok, error, locatedBy, rect, ... }.
    - Never throws for expected failures (returns ok=false with an error code).
    - Every op takes a -TimeoutSec parameter.
    - Any -Text value is REDACTED in results: only textLength + a SHA256
      prefix (first 16 hex chars, UTF-8) are reported, never the text itself.

  Safety boundaries (hard):
    - Every action re-verifies that the target HWND is still live AND still
      owned by the expected PID; on mismatch the op is REFUSED
      (error 'pid-mismatch-refused'), never retargeted.
    - This module opens no sockets, binds/listens on no port, and never
      references 127.0.0.1:10808, the host proxy/WinINET, the registry,
      routes, or TUN. There is no networking or system-config code in this file.
    - Callers must only operate on processes they started themselves and must
      close only those owned PIDs (this module kills nothing; use
      Stop-Process -Id <ownedPid> from your own script).

  Selector priority (also see tools/ui/README.md):
    1. UIA: pass -AutomationId and/or -Name and/or -ControlType.
    2. Semantics snapshot: pass -SemanticsPath plus -Identifier and/or -Label.
    3. Coordinates: pass -X -Y together with -AllowCoordinates; the result is
       always marked locatedBy='coordinates'.

.NOTES
  PowerShell 5.1 compatible. Re-dot-sourcing is safe (native type + assemblies
  load once; functions are redefined idempotently; nothing runs on import).
#>

# ---------------------------------------------------------------------------
# One-time native interop (user32 SendInput + window scan). No action on import.
# ---------------------------------------------------------------------------

$__UiAutoNativeCode = @'
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;

public static class UiAutoNative
{
    public delegate bool EnumProc(IntPtr h, IntPtr l);

    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int n);
    [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] p, int cb);

    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx; public int dy; public int mouseData; public int dwFlags; public int time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public short wVk; public short wScan; public int dwFlags; public int time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct HARDWAREINPUT { public int uMsg; public short wParamL; public short wParamH; }
    [StructLayout(LayoutKind.Explicit)] public struct INPUTUNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public HARDWAREINPUT hi; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public int type; public INPUTUNION u; }

    const int M_MOVE = 0x0001;
    const int M_LEFTDOWN = 0x0002;
    const int M_LEFTUP = 0x0004;
    const int M_RIGHTDOWN = 0x0008;
    const int M_RIGHTUP = 0x0010;
    const int M_ABSOLUTE = 0x8000;
    const int M_VIRTUALDESK = 0x4000;
    const int K_EXTENDED = 0x0001;
    const int K_UP = 0x0002;
    const int K_UNICODE = 0x0004;

    static void SendOne(INPUT i)
    {
        INPUT[] a = new INPUT[] { i };
        uint r = SendInput(1, a, Marshal.SizeOf(typeof(INPUT)));
        if (r != 1) throw new Exception("SendInput failed");
    }

    static INPUT BlankMouse()
    {
        INPUT i = new INPUT();
        i.type = 0;
        i.u.mi.dx = 0; i.u.mi.dy = 0; i.u.mi.mouseData = 0;
        i.u.mi.dwFlags = 0; i.u.mi.time = 0; i.u.mi.dwExtraInfo = IntPtr.Zero;
        return i;
    }

    // Real pointer move via SendInput on the virtual desktop (multi-monitor safe).
    public static void MoveAbsolute(int x, int y)
    {
        int vx = GetSystemMetrics(76);
        int vy = GetSystemMetrics(77);
        int vw = GetSystemMetrics(78);
        int vh = GetSystemMetrics(79);
        if (vw <= 0) { vx = 0; vw = GetSystemMetrics(0); }
        if (vh <= 0) { vy = 0; vh = GetSystemMetrics(1); }
        int dx = (int)(((double)(x - vx) * 65535.0) / (double)vw);
        int dy = (int)(((double)(y - vy) * 65535.0) / (double)vh);
        INPUT i = BlankMouse();
        i.u.mi.dx = dx; i.u.mi.dy = dy;
        i.u.mi.dwFlags = M_MOVE | M_ABSOLUTE | M_VIRTUALDESK;
        SendOne(i);
    }

    public static void MouseDown(string button)
    {
        INPUT i = BlankMouse();
        i.u.mi.dwFlags = (button == "right") ? M_RIGHTDOWN : M_LEFTDOWN;
        SendOne(i);
    }

    public static void MouseUp(string button)
    {
        INPUT i = BlankMouse();
        i.u.mi.dwFlags = (button == "right") ? M_RIGHTUP : M_LEFTUP;
        SendOne(i);
    }

    public static void KeyVk(int vk, bool down, bool extended)
    {
        INPUT i = new INPUT();
        i.type = 1;
        i.u.ki.wVk = (short)vk; i.u.ki.wScan = 0; i.u.ki.time = 0; i.u.ki.dwExtraInfo = IntPtr.Zero;
        int f = 0;
        if (extended) f |= K_EXTENDED;
        if (!down) f |= K_UP;
        i.u.ki.dwFlags = f;
        SendOne(i);
    }

    public static void TypeUnicode(char c)
    {
        INPUT d = new INPUT();
        d.type = 1;
        d.u.ki.wVk = 0; d.u.ki.wScan = (short)c; d.u.ki.dwFlags = K_UNICODE;
        d.u.ki.time = 0; d.u.ki.dwExtraInfo = IntPtr.Zero;
        SendOne(d);
        INPUT u = d;
        u.u.ki.dwFlags = K_UNICODE | K_UP;
        SendOne(u);
    }

    public static uint GetPid(IntPtr h)
    {
        uint p; GetWindowThreadProcessId(h, out p); return p;
    }

    public static string GetText(IntPtr h)
    {
        int n = GetWindowTextLength(h);
        if (n <= 0) return "";
        StringBuilder s = new StringBuilder(n + 1);
        GetWindowText(h, s, s.Capacity);
        return s.ToString();
    }

    public static string GetClass(IntPtr h)
    {
        StringBuilder s = new StringBuilder(256);
        GetClassName(h, s, s.Capacity);
        return s.ToString();
    }

    // Returns { x, y, width, height } in screen pixels, or null when unreadable.
    public static int[] GetRectArr(IntPtr h)
    {
        RECT r;
        if (!GetWindowRect(h, out r)) return null;
        return new int[] { r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top };
    }

    public static bool EnsureForeground(IntPtr h)
    {
        ShowWindow(h, 9);
        return SetForegroundWindow(h);
    }

    public static IntPtr FindByPid(uint pid)
    {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static IntPtr FindByTitle(string pattern, uint pid)
    {
        Regex rx = new Regex(pattern);
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            if (!IsWindowVisible(h)) return true;
            uint p; GetWindowThreadProcessId(h, out p);
            if (pid != 0 && p != pid) return true;
            string t = GetText(h);
            if (t.Length == 0) return true;
            if (rx.IsMatch(t)) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

if (-not ([System.Management.Automation.PSTypeName]'UiAutoNative').Type) {
  Add-Type -TypeDefinition $__UiAutoNativeCode
}
Remove-Variable -Name __UiAutoNativeCode -Scope Script -ErrorAction SilentlyContinue

try { Add-Type -AssemblyName System.Drawing } catch {}
try { Add-Type -AssemblyName UIAutomationClient } catch {}
try { Add-Type -AssemblyName UIAutomationTypes } catch {}

$script:UiAutomation_UiaAvailable = $false
try {
  $null = [System.Windows.Automation.AutomationElement]::RootElement
  $script:UiAutomation_UiaAvailable = $true
} catch {
  $script:UiAutomation_UiaAvailable = $false
}

# ---------------------------------------------------------------------------
# Internal helpers (not part of the public API surface, but harmless to call)
# ---------------------------------------------------------------------------

function New-UiFail {
  param(
    [string]$Error = 'unknown',
    [string]$LocatedBy = '',
    $Rect = $null,
    [hashtable]$Diagnostics = @{}
  )
  return [pscustomobject]@{
    ok         = $false
    error      = $Error
    locatedBy  = $LocatedBy
    rect       = $Rect
    diagnostics = $Diagnostics
  }
}

function New-UiRect {
  param([int]$X, [int]$Y, [int]$Width, [int]$Height)
  return [ordered]@{ x = $X; y = $Y; width = $Width; height = $Height }
}

function Get-UiRectCenter {
  param($Rect)
  if ($Rect -eq $null) { return $null }
  return [ordered]@{
    x = [int]($Rect.x + [math]::Floor($Rect.width / 2))
    y = [int]($Rect.y + [math]::Floor($Rect.height / 2))
  }
}

# Saturated double->int for UIA rects (offscreen nodes can report NaN/Inf or
# extreme values that would otherwise throw on a direct [int] cast).
function ConvertTo-UiInt {
  param([double]$Value)
  if ([double]::IsNaN($Value) -or [double]::IsInfinity($Value)) { return 0 }
  if ($Value -gt 100000) { return 100000 }
  if ($Value -lt -100000) { return -100000 }
  return [int]$Value
}

function Get-UiSha256Prefix {
  param([string]$Value)
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($Value)
  $sha = [System.Security.Cryptography.SHA256]::Create()
  try {
    $hash = $sha.ComputeHash($bytes)
    return ([System.BitConverter]::ToString($hash)).Replace('-', '').Substring(0, 16).ToLowerInvariant()
  } finally {
    $sha.Dispose()
  }
}

function New-UiWindowObject {
  param([IntPtr]$Hwnd, [string]$LocatedBy, [int]$Attempts, [long]$ElapsedMs)
  $pidOut = [UiAutoNative]::GetPid($Hwnd)
  $rectArr = [UiAutoNative]::GetRectArr($Hwnd)
  $rect = $null
  if ($rectArr -ne $null) {
    $rect = New-UiRect -X $rectArr[0] -Y $rectArr[1] -Width $rectArr[2] -Height $rectArr[3]
  }
  $hwndLong = $Hwnd.ToInt64()
  return [pscustomobject]@{
    ok         = $true
    error      = ''
    locatedBy  = $LocatedBy
    hwnd       = $hwndLong
    hwndHex    = ('0x{0:X}' -f $hwndLong)
    pid        = [int]$pidOut
    title      = [UiAutoNative]::GetText($Hwnd)
    className  = [UiAutoNative]::GetClass($Hwnd)
    rect       = $rect
    attempts   = $Attempts
    elapsedMs  = $ElapsedMs
    diagnostics = @{}
  }
}

# Input-safety gate: resolves -Window or -Hwnd/-ExpectedPid to a live,
# PID-verified target. Polls up to -TimeoutSec so freshly launched windows
# can settle. Returns @{ ok, hwndLong, expectedPid, ... } or a fail object.
function Resolve-UiTargetWindow {
  param($Window = $null, [long]$Hwnd = 0, [int]$ExpectedPid = 0,
        [int]$TimeoutSec = 10, [int]$PollMs = 400, [string]$Op = 'op')
  $t0 = Get-Date
  try {
    $hwndLong = 0
    $pidWant = 0
    if ($Window -ne $null) {
      $hwndLong = [long]$Window.hwnd
      $pidWant = [int]$Window.pid
    } else {
      if ($Hwnd -eq 0 -or $ExpectedPid -le 0) {
        return New-UiFail -Error 'window-context-required' -Diagnostics @{
          op = $Op; hint = 'pass -Window (from Get-UiWindow) or -Hwnd plus -ExpectedPid'
        }
      }
      $hwndLong = $Hwnd
      $pidWant = $ExpectedPid
    }
    if ($TimeoutSec -lt 1) { $TimeoutSec = 1 }
    if ($PollMs -lt 100) { $PollMs = 100 }
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $attempts = 0
    while ($true) {
      $attempts++
      $h = [IntPtr]$hwndLong
      if ([UiAutoNative]::IsWindow($h)) {
        $livePid = [int][UiAutoNative]::GetPid($h)
        if ($livePid -eq $pidWant) {
          return [pscustomobject]@{
            ok = $true; error = ''; hwndLong = $hwndLong; expectedPid = $pidWant
            attempts = $Attempts; elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
          }
        }
        return New-UiFail -Error 'pid-mismatch-refused' -Diagnostics @{
          op = $Op; hwnd = $hwndLong; expectedPid = $pidWant; livePid = $livePid
        }
      }
      if ((Get-Date) -ge $deadline) { break }
      Start-Sleep -Milliseconds $PollMs
    }
    return New-UiFail -Error 'hwnd-not-live' -Diagnostics @{
      op = $Op; hwnd = $hwndLong; expectedPid = $pidWant; attempts = $attempts
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -Diagnostics @{ op = $Op }
  }
}

# Normalizes a semantics-snapshot rect to @{ x, y, width, height } (screen px).
# Accepted shapes: [x, y, w, h] array, or an object with x/y/width/height
# (w/h aliases allowed), or left/top/right/bottom. Anything else -> $null.
function ConvertTo-UiRect {
  param($Raw)
  try {
    if ($Raw -eq $null) { return $null }
    if ($Raw -is [array] -and $Raw.Count -eq 4) {
      return New-UiRect -X ([int]$Raw[0]) -Y ([int]$Raw[1]) -Width ([int]$Raw[2]) -Height ([int]$Raw[3])
    }
    $props = @{}
    foreach ($p in $Raw.PSObject.Properties) { $props[$p.Name.ToLowerInvariant()] = $p.Value }
    if ($props.ContainsKey('x') -and $props.ContainsKey('y')) {
      $w = $null; $h = $null
      foreach ($k in @('width', 'w')) { if ($props.ContainsKey($k)) { $w = [int]$props[$k] } }
      foreach ($k in @('height', 'h')) { if ($props.ContainsKey($k)) { $h = [int]$props[$k] } }
      if ($w -ne $null -and $h -ne $null) {
        return New-UiRect -X ([int]$props['x']) -Y ([int]$props['y']) -Width $w -Height $h
      }
    }
    if ($props.ContainsKey('left') -and $props.ContainsKey('top') -and
        $props.ContainsKey('right') -and $props.ContainsKey('bottom')) {
      $l = [int]$props['left']; $t = [int]$props['top']
      return New-UiRect -X $l -Y $t -Width ([int]$props['right'] - $l) -Height ([int]$props['bottom'] - $t)
    }
    return $null
  } catch {
    return $null
  }
}

# Flattens a nested semantics tree (roots[].tree.children..., as written by
# apps/desktop/lib/perf/semantics_dump_hook.dart) into a plain node array.
function Expand-UiSemSubtree {
  param($Node)
  $flat = @()
  if ($Node -eq $null) { return $flat }
  $flat += ,$Node
  if ($Node.PSObject.Properties['children'] -ne $null -and $Node.children -is [array]) {
    foreach ($c in $Node.children) { $flat += (Expand-UiSemSubtree -Node $c) }
  }
  return $flat
}

function Find-UiAutomationControlType {
  param([string]$Name)
  $short = $Name
  $dot = $Name.LastIndexOf('.')
  if ($dot -ge 0) { $short = $Name.Substring($dot + 1) }
  $ctType = [System.Windows.Automation.ControlType]
  $flags = [System.Reflection.BindingFlags]'Public, Static, IgnoreCase'
  # NOTE: ControlType members (Button, Edit, Document, ...) are static FIELDS.
  $field = $ctType.GetField($short, $flags)
  if ($field -ne $null) { return $field.GetValue($null) }
  $prop = $ctType.GetProperty($short, $flags)
  if ($prop -eq $null) { return $null }
  return $prop.GetValue($null, $null)
}

# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

function Get-UiWindow {
  <#
  .SYNOPSIS
    Finds a visible, titled top-level window by title regex, owning PID, or HWND.
  #>
  [CmdletBinding()]
  param(
    [string]$Title = '',
    [int]$ProcessId = 0,
    [long]$Hwnd = 0,
    [int]$TimeoutSec = 10,
    [int]$PollMs = 400
  )
  $t0 = Get-Date
  try {
    if ($TimeoutSec -lt 1) { $TimeoutSec = 1 }
    if ($PollMs -lt 100) { $PollMs = 100 }
    if ($Title -eq '' -and $ProcessId -le 0 -and $Hwnd -eq 0) {
      return New-UiFail -Error 'specify -Title, -ProcessId, or -Hwnd' -Diagnostics @{ op = 'Get-UiWindow' }
    }
    if ($Hwnd -ne 0) {
      $h = [IntPtr]$Hwnd
      if (-not [UiAutoNative]::IsWindow($h)) {
        return New-UiFail -Error 'hwnd-not-live' -Diagnostics @{ op = 'Get-UiWindow'; hwnd = $Hwnd }
      }
      $livePid = [int][UiAutoNative]::GetPid($h)
      if ($ProcessId -gt 0 -and $livePid -ne $ProcessId) {
        return New-UiFail -Error 'pid-mismatch-refused' -Diagnostics @{
          op = 'Get-UiWindow'; hwnd = $Hwnd; expectedPid = $ProcessId; livePid = $livePid
        }
      }
      $elapsed = [long]((Get-Date) - $t0).TotalMilliseconds
      return New-UiWindowObject -Hwnd $h -LocatedBy 'hwnd' -Attempts 1 -ElapsedMs $elapsed
    }
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $attempts = 0
    while ($true) {
      $attempts++
      $found = [IntPtr]::Zero
      try {
        if ($Title -ne '') {
          $pidFilter = 0
          if ($ProcessId -gt 0) { $pidFilter = $ProcessId }
          $found = [UiAutoNative]::FindByTitle($Title, [uint32]$pidFilter)
        } else {
          $found = [UiAutoNative]::FindByPid([uint32]$ProcessId)
        }
      } catch {
        return New-UiFail -Error ('title-regex-invalid: ' + $_.Exception.Message) -Diagnostics @{
          op = 'Get-UiWindow'; title = $Title
        }
      }
      if ($found -ne [IntPtr]::Zero) {
        $elapsed = [long]((Get-Date) - $t0).TotalMilliseconds
        return New-UiWindowObject -Hwnd $found -LocatedBy 'window-scan' -Attempts $attempts -ElapsedMs $elapsed
      }
      if ((Get-Date) -ge $deadline) { break }
      Start-Sleep -Milliseconds $PollMs
    }
    return New-UiFail -Error 'window-not-found' -Diagnostics @{
      op = 'Get-UiWindow'; title = $Title; processId = $ProcessId
      timeoutSec = $TimeoutSec; attempts = $attempts
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -Diagnostics @{ op = 'Get-UiWindow' }
  }
}

function Get-UiSemantics {
  <#
  .SYNOPSIS
    Reads + parses a semantics snapshot file written by our armed app hook.
    Expected JSON: { "nodes": [ { "identifier": "...", "label": "...",
    "rect": { "x":..,"y":..,"width":..,"height":.. } } ] }
    (a bare array, or .elements / .semantics / .children, is also accepted;
    rect also accepts a [x, y, w, h] array or left/top/right/bottom).
  #>
  [CmdletBinding()]
  param(
    [string]$Path = '',
    [int]$TimeoutSec = 5,
    [int]$PollMs = 400
  )
  $t0 = Get-Date
  try {
    if ($TimeoutSec -lt 1) { $TimeoutSec = 1 }
    if ($PollMs -lt 100) { $PollMs = 100 }
    if ($Path -eq '') {
      return New-UiFail -Error 'path-required' -LocatedBy 'semantics-file' -Diagnostics @{ op = 'Get-UiSemantics' }
    }
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while (-not (Test-Path -LiteralPath $Path)) {
      if ((Get-Date) -ge $deadline) { break }
      Start-Sleep -Milliseconds $PollMs
    }
    if (-not (Test-Path -LiteralPath $Path)) {
      return New-UiFail -Error 'semantics-file-missing' -LocatedBy 'semantics-file' -Diagnostics @{
        op = 'Get-UiSemantics'; path = $Path; timeoutSec = $TimeoutSec
      }
    }
    $raw = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 -ErrorAction Stop
    $json = $raw | ConvertFrom-Json -ErrorAction Stop
    $list = $null
    if ($json -is [array]) {
      $list = $json
    } else {
      foreach ($k in @('nodes', 'elements', 'semantics', 'children')) {
        if ($json.PSObject.Properties[$k] -ne $null -and $json.$k -is [array]) { $list = $json.$k; break }
      }
      if ($list -eq $null -and $json.PSObject.Properties['roots'] -ne $null -and $json.roots -is [array]) {
        # Nested hook shape (semantics_dump_hook.dart): roots[].tree + children.
        $flat = @()
        foreach ($root in $json.roots) {
          if ($root -eq $null) { continue }
          $tree = $null
          if ($root.PSObject.Properties['tree'] -ne $null) { $tree = $root.tree }
          elseif ($root.PSObject.Properties['node'] -ne $null) { $tree = $root.node }
          $flat += (Expand-UiSemSubtree -Node $tree)
        }
        $list = $flat
      }
    }
    if ($list -eq $null) {
      return New-UiFail -Error 'semantics-shape-unknown' -LocatedBy 'semantics-file' -Diagnostics @{
        op = 'Get-UiSemantics'; path = $Path
        hint = 'top level must be an array or an object with nodes/elements/semantics/children'
      }
    }
    $nodes = @()
    $skipped = 0
    foreach ($n in $list) {
      $identifier = ''
      foreach ($k in @('identifier', 'id', 'automationId')) {
        if ($n.PSObject.Properties[$k] -ne $null -and [string]$n.$k -ne '') { $identifier = [string]$n.$k; break }
      }
      $label = ''
      foreach ($k in @('label', 'name', 'text', 'value')) {
        if ($n.PSObject.Properties[$k] -ne $null -and [string]$n.$k -ne '') { $label = [string]$n.$k; break }
      }
      $rectRaw = $null
      if ($n.PSObject.Properties['rect'] -ne $null) { $rectRaw = $n.rect }
      $rect = ConvertTo-UiRect -Raw $rectRaw
      if ($identifier -eq '' -or $rect -eq $null) { $skipped++; continue }
      $nodes += [pscustomobject]@{
        identifier = $identifier
        label      = $label
        rect       = $rect
        center     = (Get-UiRectCenter -Rect $rect)
      }
    }
    return [pscustomobject]@{
      ok         = $true
      error      = ''
      locatedBy  = 'semantics-file'
      rect       = $null
      path       = $Path
      count      = $nodes.Count
      nodes      = $nodes
      diagnostics = @{ skipped = $skipped; elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds }
    }
  } catch {
    return New-UiFail -Error ('semantics-parse-failed: ' + $_.Exception.Message) -LocatedBy 'semantics-file' -Diagnostics @{
      op = 'Get-UiSemantics'; path = $Path
    }
  }
}

function Get-UiElement {
  <#
  .SYNOPSIS
    Locates one element's screen rect. Priority: (1) UIA, (2) semantics
    snapshot file, (3) raw -X/-Y with -AllowCoordinates (marked locatedBy='coordinates').
  #>
  [CmdletBinding()]
  param(
    $Window = $null,
    [long]$Hwnd = 0,
    [int]$ExpectedPid = 0,
    [string]$AutomationId = '',
    [string]$Name = '',
    [string]$ControlType = '',
    [string]$Identifier = '',
    [string]$Label = '',
    [string]$SemanticsPath = '',
    [int]$X = 0,
    [int]$Y = 0,
    [switch]$AllowCoordinates,
    [int]$TimeoutSec = 10,
    [int]$PollMs = 400
  )
  $t0 = Get-Date
  try {
    if ($TimeoutSec -lt 1) { $TimeoutSec = 1 }
    if ($PollMs -lt 100) { $PollMs = 100 }
    $useUia = ($AutomationId -ne '' -or $Name -ne '' -or $ControlType -ne '')
    $useSem = ($Identifier -ne '' -or $Label -ne '')
    $useXy = ($PSBoundParameters.ContainsKey('X') -or $PSBoundParameters.ContainsKey('Y'))
    if (-not $useUia -and -not $useSem -and -not $useXy) {
      return New-UiFail -Error 'selector-required' -Diagnostics @{
        op = 'Get-UiElement'
        hint = 'pass UIA (-AutomationId/-Name/-ControlType), semantics (-Identifier/-Label + -SemanticsPath), or -X/-Y with -AllowCoordinates'
      }
    }

    # ---- (1) UIA path ----
    if ($useUia) {
      if (-not $script:UiAutomation_UiaAvailable) {
        return New-UiFail -Error 'uia-unavailable' -Diagnostics @{ op = 'Get-UiElement' }
      }
      $t = Resolve-UiTargetWindow -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid -TimeoutSec $TimeoutSec -PollMs $PollMs -Op 'Get-UiElement'
      if (-not $t.ok) { return $t }
      $conds = @()
      if ($AutomationId -ne '') {
        $conds += New-Object System.Windows.Automation.PropertyCondition(
          [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $AutomationId)
      }
      if ($Name -ne '') {
        $conds += New-Object System.Windows.Automation.PropertyCondition(
          [System.Windows.Automation.AutomationElement]::NameProperty, $Name)
      }
      if ($ControlType -ne '') {
        $ct = Find-UiAutomationControlType -Name $ControlType
        if ($ct -eq $null) {
          return New-UiFail -Error 'uia-controltype-unknown' -LocatedBy 'uia' -Diagnostics @{
            op = 'Get-UiElement'; controlType = $ControlType
          }
        }
        $conds += New-Object System.Windows.Automation.PropertyCondition(
          [System.Windows.Automation.AutomationElement]::ControlTypeProperty, $ct)
      }
      if ($conds.Count -eq 1) { $cond = $conds[0] }
      else { $cond = New-Object System.Windows.Automation.AndCondition($conds) }
      $deadline = (Get-Date).AddSeconds($TimeoutSec)
      $attempts = 0
      while ($true) {
        $attempts++
        try {
          $root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$t.hwndLong)
          if ($root -ne $null) {
            $found = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
            if ($found -ne $null) {
              $r = $found.Current.BoundingRectangle
              $rect = New-UiRect -X (ConvertTo-UiInt -Value $r.X) -Y (ConvertTo-UiInt -Value $r.Y) `
                -Width (ConvertTo-UiInt -Value $r.Width) -Height (ConvertTo-UiInt -Value $r.Height)
              $empty = ($rect.width -le 0 -or $rect.height -le 0)
              return [pscustomobject]@{
                ok = $true; error = ''; locatedBy = 'uia'; rect = $rect
                center = (Get-UiRectCenter -Rect $rect)
                windowHwnd = $t.hwndLong; windowPid = $t.expectedPid
                automationId = $found.Current.AutomationId; name = $found.Current.Name
                controlType = $found.Current.ControlType.ProgrammaticName
                diagnostics = @{
                  attempts = $attempts; elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
                  emptyRect = $empty
                  selector = ('automationId=' + $AutomationId + ';name=' + $Name + ';controlType=' + $ControlType)
                }
              }
            }
          }
        } catch {}
        if ((Get-Date) -ge $deadline) { break }
        Start-Sleep -Milliseconds $PollMs
      }
      return New-UiFail -Error 'uia-element-not-found' -LocatedBy 'uia' -Diagnostics @{
        op = 'Get-UiElement'; automationId = $AutomationId; name = $Name; controlType = $ControlType
        timeoutSec = $TimeoutSec; attempts = $attempts
      }
    }

    # ---- (2) semantics snapshot path ----
    if ($useSem) {
      if ($SemanticsPath -eq '') {
        return New-UiFail -Error 'semantics-path-required' -LocatedBy 'semantics' -Diagnostics @{
          op = 'Get-UiElement'; hint = 'pass -SemanticsPath with -Identifier/-Label'
        }
      }
      $t = Resolve-UiTargetWindow -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid -TimeoutSec $TimeoutSec -PollMs $PollMs -Op 'Get-UiElement'
      if (-not $t.ok) { return $t }
      $deadline = (Get-Date).AddSeconds($TimeoutSec)
      $attempts = 0
      $lastCount = 0
      while ($true) {
        $attempts++
        $sem = Get-UiSemantics -Path $SemanticsPath -TimeoutSec 1
        if ($sem.ok) {
          $lastCount = $sem.count
          foreach ($n in $sem.nodes) {
            $idOk = ($Identifier -eq '' -or $n.identifier -ieq $Identifier)
            $labelOk = ($Label -eq '' -or $n.label -ieq $Label)
            if ($idOk -and $labelOk) {
              return [pscustomobject]@{
                ok = $true; error = ''; locatedBy = 'semantics'; rect = $n.rect
                center = $n.center
                windowHwnd = $t.hwndLong; windowPid = $t.expectedPid
                identifier = $n.identifier; label = $n.label
                diagnostics = @{
                  attempts = $attempts; elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
                  semanticsPath = $SemanticsPath
                }
              }
            }
          }
        }
        if ((Get-Date) -ge $deadline) { break }
        Start-Sleep -Milliseconds $PollMs
      }
      return New-UiFail -Error 'semantics-no-match' -LocatedBy 'semantics' -Diagnostics @{
        op = 'Get-UiElement'; identifier = $Identifier; label = $Label
        semanticsPath = $SemanticsPath; candidates = $lastCount
        timeoutSec = $TimeoutSec; attempts = $attempts
      }
    }

    # ---- (3) raw coordinate fallback (explicit opt-in) ----
    if (-not $AllowCoordinates) {
      return New-UiFail -Error 'coordinates-require-allow' -Diagnostics @{
        op = 'Get-UiElement'
        hint = 'raw -X/-Y fallback needs -AllowCoordinates; result would be marked locatedBy=coordinates'
      }
    }
    $t = Resolve-UiTargetWindow -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid -TimeoutSec $TimeoutSec -PollMs $PollMs -Op 'Get-UiElement'
    if (-not $t.ok) { return $t }
    $rect = New-UiRect -X $X -Y $Y -Width 0 -Height 0
    return [pscustomobject]@{
      ok = $true; error = ''; locatedBy = 'coordinates'; rect = $rect
      center = ([ordered]@{ x = $X; y = $Y })
      windowHwnd = $t.hwndLong; windowPid = $t.expectedPid
      diagnostics = @{
        elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
        warning = 'coordinate fallback: brittle under DPI/layout change; prefer UIA or semantics selectors'
      }
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -Diagnostics @{ op = 'Get-UiElement' }
  }
}

function Invoke-UiMouseAtPoint {
  param(
    [long]$HwndLong, [int]$PidWant, [int]$X, [int]$Y,
    [string]$Button, [int]$Clicks, [string]$LocatedBy,
    [int]$TimeoutSec, [int]$PollMs, [bool]$EnsureForeground, [string]$Op
  )
  $t0 = Get-Date
  try {
    $t = Resolve-UiTargetWindow -Hwnd $HwndLong -ExpectedPid $PidWant -TimeoutSec $TimeoutSec -PollMs $PollMs -Op $Op
    if (-not $t.ok) { return $t }
    $h = [IntPtr]$HwndLong
    $fg = $false
    if ($EnsureForeground) {
      $fg = [UiAutoNative]::EnsureForeground($h)
      Start-Sleep -Milliseconds 350
    }
    [UiAutoNative]::MoveAbsolute($X, $Y)
    Start-Sleep -Milliseconds 60
    $sends = 0
    if ($Clicks -eq 2) {
      [UiAutoNative]::MouseDown($Button); Start-Sleep -Milliseconds 60; [UiAutoNative]::MouseUp($Button)
      $sends = 2
      Start-Sleep -Milliseconds 100
      [UiAutoNative]::MouseDown($Button); Start-Sleep -Milliseconds 60; [UiAutoNative]::MouseUp($Button)
      $sends = 4
    } else {
      [UiAutoNative]::MouseDown($Button); Start-Sleep -Milliseconds 60; [UiAutoNative]::MouseUp($Button)
      $sends = 2
    }
    return [pscustomobject]@{
      ok = $true; error = ''; locatedBy = $LocatedBy; rect = $null
      point = ([ordered]@{ x = $X; y = $Y }); button = $Button; clicks = $Clicks
      pidVerified = $PidWant; hwndHex = ('0x{0:X}' -f $HwndLong)
      diagnostics = @{
        op = $Op; foregroundEnsured = $fg; sendInputCalls = ($sends + 1)
        elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
        method = 'SendInput move + down/up (real input; no InvokePattern, no app API)'
      }
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -LocatedBy $LocatedBy -Diagnostics @{ op = $Op }
  }
}

function Resolve-UiClickPoint {
  param($Element = $null, [int]$X = 0, [int]$Y = 0, [bool]$HasPoint = $false, [switch]$AllowCoordinates,
        $Window = $null, [long]$Hwnd = 0, [int]$ExpectedPid = 0, [string]$Op = 'op')
  if ($Element -ne $null) {
    if (-not $Element.ok) {
      return New-UiFail -Error 'element-not-ok' -Diagnostics @{ op = $Op; elementError = $Element.error }
    }
    if ($Element.center -eq $null) {
      return New-UiFail -Error 'element-no-point' -Diagnostics @{ op = $Op }
    }
    $hwndLong = 0; $pidWant = 0
    if ($Element.windowHwnd -ne $null) { $hwndLong = [long]$Element.windowHwnd }
    if ($Element.windowPid -ne $null) { $pidWant = [int]$Element.windowPid }
    if ($hwndLong -eq 0 -or $pidWant -le 0) {
      if ($Window -ne $null) { $hwndLong = [long]$Window.hwnd; $pidWant = [int]$Window.pid }
      else { $hwndLong = $Hwnd; $pidWant = $ExpectedPid }
    }
    $loc = 'uia'
    if ($Element.locatedBy -ne $null -and $Element.locatedBy -ne '') { $loc = $Element.locatedBy }
    return [pscustomobject]@{
      ok = $true; error = ''; hwndLong = $hwndLong; pidWant = $pidWant
      x = [int]$Element.center.x; y = [int]$Element.center.y
      locatedBy = $loc; rect = $Element.rect
    }
  }
  if (-not $HasPoint) {
    return New-UiFail -Error 'point-required' -Diagnostics @{
      op = $Op; hint = 'pass -Element or -X/-Y (with -AllowCoordinates)'
    }
  }
  if (-not $AllowCoordinates) {
    return New-UiFail -Error 'coordinates-require-allow' -Diagnostics @{
      op = $Op; hint = 'raw -X/-Y clicks need -AllowCoordinates'
    }
  }
  $hwndLong = $Hwnd; $pidWant = $ExpectedPid
  if ($Window -ne $null) { $hwndLong = [long]$Window.hwnd; $pidWant = [int]$Window.pid }
  if ($hwndLong -eq 0 -or $pidWant -le 0) {
    return New-UiFail -Error 'window-context-required' -Diagnostics @{
      op = $Op; hint = 'pass -Window or -Hwnd plus -ExpectedPid'
    }
  }
  return [pscustomobject]@{
    ok = $true; error = ''; hwndLong = $hwndLong; pidWant = $pidWant
    x = $X; y = $Y; locatedBy = 'coordinates'; rect = (New-UiRect -X $X -Y $Y -Width 0 -Height 0)
  }
}

function Invoke-UiRealClick {
  <#
  .SYNOPSIS
    Left-clicks an element (or an explicit point) with REAL SendInput input.
  #>
  [CmdletBinding()]
  param(
    $Element = $null, $Window = $null,
    [long]$Hwnd = 0, [int]$ExpectedPid = 0,
    [int]$X = 0, [int]$Y = 0, [switch]$AllowCoordinates,
    [int]$TimeoutSec = 10, [int]$PollMs = 400,
    [bool]$EnsureForeground = $true
  )
  $hasXY = $PSBoundParameters.ContainsKey('X') -or $PSBoundParameters.ContainsKey('Y')
  $p = Resolve-UiClickPoint -Element $Element -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid `
    -X $X -Y $Y -HasPoint $hasXY -AllowCoordinates:$AllowCoordinates -Op 'Invoke-UiRealClick'
  if (-not $p.ok) { return $p }
  $r = Invoke-UiMouseAtPoint -HwndLong $p.hwndLong -PidWant $p.pidWant -X $p.x -Y $p.y `
    -Button 'left' -Clicks 1 -LocatedBy $p.locatedBy -TimeoutSec $TimeoutSec -PollMs $PollMs `
    -EnsureForeground $EnsureForeground -Op 'Invoke-UiRealClick'
  if ($r.ok -and $p.rect -ne $null) { $r.rect = $p.rect }
  return $r
}

function Invoke-UiRealDoubleClick {
  <#
  .SYNOPSIS
    Double-left-clicks an element (or an explicit point) with REAL SendInput input.
  #>
  [CmdletBinding()]
  param(
    $Element = $null, $Window = $null,
    [long]$Hwnd = 0, [int]$ExpectedPid = 0,
    [int]$X = 0, [int]$Y = 0, [switch]$AllowCoordinates,
    [int]$TimeoutSec = 10, [int]$PollMs = 400,
    [bool]$EnsureForeground = $true
  )
  $hasXY = $PSBoundParameters.ContainsKey('X') -or $PSBoundParameters.ContainsKey('Y')
  $p = Resolve-UiClickPoint -Element $Element -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid `
    -X $X -Y $Y -HasPoint $hasXY -AllowCoordinates:$AllowCoordinates -Op 'Invoke-UiRealDoubleClick'
  if (-not $p.ok) { return $p }
  $r = Invoke-UiMouseAtPoint -HwndLong $p.hwndLong -PidWant $p.pidWant -X $p.x -Y $p.y `
    -Button 'left' -Clicks 2 -LocatedBy $p.locatedBy -TimeoutSec $TimeoutSec -PollMs $PollMs `
    -EnsureForeground $EnsureForeground -Op 'Invoke-UiRealDoubleClick'
  if ($r.ok -and $p.rect -ne $null) { $r.rect = $p.rect }
  return $r
}

function Invoke-UiRealRightClick {
  <#
  .SYNOPSIS
    Right-clicks an element (or an explicit point) with REAL SendInput input.
  #>
  [CmdletBinding()]
  param(
    $Element = $null, $Window = $null,
    [long]$Hwnd = 0, [int]$ExpectedPid = 0,
    [int]$X = 0, [int]$Y = 0, [switch]$AllowCoordinates,
    [int]$TimeoutSec = 10, [int]$PollMs = 400,
    [bool]$EnsureForeground = $true
  )
  $hasXY = $PSBoundParameters.ContainsKey('X') -or $PSBoundParameters.ContainsKey('Y')
  $p = Resolve-UiClickPoint -Element $Element -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid `
    -X $X -Y $Y -HasPoint $hasXY -AllowCoordinates:$AllowCoordinates -Op 'Invoke-UiRealRightClick'
  if (-not $p.ok) { return $p }
  $r = Invoke-UiMouseAtPoint -HwndLong $p.hwndLong -PidWant $p.pidWant -X $p.x -Y $p.y `
    -Button 'right' -Clicks 1 -LocatedBy $p.locatedBy -TimeoutSec $TimeoutSec -PollMs $PollMs `
    -EnsureForeground $EnsureForeground -Op 'Invoke-UiRealRightClick'
  if ($r.ok -and $p.rect -ne $null) { $r.rect = $p.rect }
  return $r
}

function Send-UiText {
  <#
  .SYNOPSIS
    Types ASCII-only text into the foreground target via REAL SendInput keystrokes.
    The text is NEVER echoed back: results carry textLength + textSha256Prefix only.
  #>
  [CmdletBinding()]
  param(
    $Window = $null,
    [long]$Hwnd = 0,
    [int]$ExpectedPid = 0,
    [string]$Text = '',
    [int]$TimeoutSec = 10,
    [int]$PollMs = 400,
    [int]$CharDelayMs = 5,
    [bool]$EnsureForeground = $true
  )
  $t0 = Get-Date
  try {
    if ($Text -eq '') {
      return New-UiFail -Error 'empty-text' -LocatedBy 'keyboard-text' -Diagnostics @{ op = 'Send-UiText' }
    }
    if ([regex]::IsMatch($Text, '[^\x20-\x7E\r\n\t]')) {
      return New-UiFail -Error 'ascii-only' -LocatedBy 'keyboard-text' -Diagnostics @{
        op = 'Send-UiText'
        hint = 'only printable ASCII plus CR/LF/TAB are supported; use Send-UiKey for special keys'
      }
    }
    $t = Resolve-UiTargetWindow -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid -TimeoutSec $TimeoutSec -PollMs $PollMs -Op 'Send-UiText'
    if (-not $t.ok) {
      $t.locatedBy = 'keyboard-text'
      return $t
    }
    $fg = $false
    if ($EnsureForeground) {
      $fg = [UiAutoNative]::EnsureForeground([IntPtr]$t.hwndLong)
      Start-Sleep -Milliseconds 350
    }
    if ($CharDelayMs -lt 0) { $CharDelayMs = 0 }
    if ($CharDelayMs -gt 500) { $CharDelayMs = 500 }
    $sent = 0
    foreach ($c in $Text.ToCharArray()) {
      [UiAutoNative]::TypeUnicode($c)
      $sent++
      if ($CharDelayMs -gt 0) { Start-Sleep -Milliseconds $CharDelayMs }
    }
    return [pscustomobject]@{
      ok = $true; error = ''; locatedBy = 'keyboard-text'; rect = $null
      textLength = $Text.Length
      textSha256Prefix = (Get-UiSha256Prefix -Value $Text)
      charsSent = $sent
      pidVerified = $t.expectedPid
      diagnostics = @{
        op = 'Send-UiText'; foregroundEnsured = $fg
        elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
        method = 'SendInput unicode keystrokes (real input; text redacted)'
      }
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -LocatedBy 'keyboard-text' -Diagnostics @{ op = 'Send-UiText' }
  }
}

$script:UiAutomation_KeyMap = @{
  'ESC' = 0x1B; 'ESCAPE' = 0x1B; 'TAB' = 0x09; 'ENTER' = 0x0D; 'RETURN' = 0x0D
  'SPACE' = 0x20; 'BACKSPACE' = 0x08; 'BACK' = 0x08; 'DELETE' = 0x2E; 'DEL' = 0x2E
  'INSERT' = 0x2D; 'INS' = 0x2D; 'UP' = 0x26; 'DOWN' = 0x28; 'LEFT' = 0x25; 'RIGHT' = 0x27
  'HOME' = 0x24; 'END' = 0x23; 'PAGEUP' = 0x21; 'PGUP' = 0x21; 'PRIOR' = 0x21
  'PAGEDOWN' = 0x22; 'PGDN' = 0x22; 'NEXT' = 0x22
  'F1' = 0x70; 'F2' = 0x71; 'F3' = 0x72; 'F4' = 0x73; 'F5' = 0x74; 'F6' = 0x75
  'F7' = 0x76; 'F8' = 0x77; 'F9' = 0x78; 'F10' = 0x79; 'F11' = 0x7A; 'F12' = 0x7B
}
$script:UiAutomation_ExtendedKeys = @('UP', 'DOWN', 'LEFT', 'RIGHT', 'HOME', 'END',
  'PAGEUP', 'PGUP', 'PRIOR', 'PAGEDOWN', 'PGDN', 'NEXT', 'INSERT', 'INS', 'DELETE', 'DEL')
$script:UiAutomation_ModMap = @{
  'CTRL' = 0x11; 'CONTROL' = 0x11; 'SHIFT' = 0x10; 'ALT' = 0x12; 'MENU' = 0x12
}

function Send-UiKey {
  <#
  .SYNOPSIS
    Sends one special key (ESC/TAB/ENTER/arrows/...) with optional modifiers
    via REAL SendInput input. The WIN key is blocked.
  #>
  [CmdletBinding()]
  param(
    $Window = $null,
    [long]$Hwnd = 0,
    [int]$ExpectedPid = 0,
    [string]$Key = '',
    [string[]]$Modifiers = @(),
    [int]$TimeoutSec = 10,
    [int]$PollMs = 400,
    [bool]$EnsureForeground = $true
  )
  $t0 = Get-Date
  try {
    $k = $Key.ToUpperInvariant()
    if ($k -eq '' -or $k -like '*WIN*') {
      return New-UiFail -Error 'key-blocked' -LocatedBy 'keyboard-key' -Diagnostics @{
        op = 'Send-UiKey'; hint = 'empty and WIN keys are refused'
      }
    }
    $vk = $null
    if ($script:UiAutomation_KeyMap.ContainsKey($k)) {
      $vk = [int]$script:UiAutomation_KeyMap[$k]
    } elseif ($k.Length -eq 1 -and $k -match '^[A-Z0-9]$') {
      $vk = [int][char]$k
    } else {
      return New-UiFail -Error 'key-unknown' -LocatedBy 'keyboard-key' -Diagnostics @{
        op = 'Send-UiKey'; key = $Key
        supported = 'ESC/TAB/ENTER/SPACE/BACKSPACE/DELETE/INSERT/arrows/HOME/END/PAGEUP/PAGEDOWN/F1-F12/A-Z/0-9'
      }
    }
    $modVks = @()
    foreach ($m in $Modifiers) {
      $mu = $m.ToUpperInvariant()
      if (-not $script:UiAutomation_ModMap.ContainsKey($mu)) {
        return New-UiFail -Error 'modifier-unknown' -LocatedBy 'keyboard-key' -Diagnostics @{
          op = 'Send-UiKey'; modifier = $m; supported = 'Ctrl/Shift/Alt'
        }
      }
      $modVks += [int]$script:UiAutomation_ModMap[$mu]
    }
    $t = Resolve-UiTargetWindow -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid -TimeoutSec $TimeoutSec -PollMs $PollMs -Op 'Send-UiKey'
    if (-not $t.ok) {
      $t.locatedBy = 'keyboard-key'
      return $t
    }
    $fg = $false
    if ($EnsureForeground) {
      $fg = [UiAutoNative]::EnsureForeground([IntPtr]$t.hwndLong)
      Start-Sleep -Milliseconds 350
    }
    $ext = ($script:UiAutomation_ExtendedKeys -contains $k)
    $sends = 0
    foreach ($mvk in $modVks) { [UiAutoNative]::KeyVk($mvk, $true, $false); $sends++ }
    [UiAutoNative]::KeyVk($vk, $true, $ext); $sends++
    Start-Sleep -Milliseconds 40
    [UiAutoNative]::KeyVk($vk, $false, $ext); $sends++
    for ($i = $modVks.Count - 1; $i -ge 0; $i--) { [UiAutoNative]::KeyVk($modVks[$i], $false, $false); $sends++ }
    return [pscustomobject]@{
      ok = $true; error = ''; locatedBy = 'keyboard-key'; rect = $null
      key = $k; modifiers = $Modifiers
      pidVerified = $t.expectedPid
      diagnostics = @{
        op = 'Send-UiKey'; foregroundEnsured = $fg; sendInputCalls = $sends
        elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
        method = 'SendInput virtual-key (real input; no app API)'
      }
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -LocatedBy 'keyboard-key' -Diagnostics @{ op = 'Send-UiKey' }
  }
}

function Wait-UiWindow {
  <#
  .SYNOPSIS
    Waits for a window to appear (same selectors as Get-UiWindow).
  #>
  [CmdletBinding()]
  param(
    [string]$Title = '',
    [int]$ProcessId = 0,
    [long]$Hwnd = 0,
    [int]$TimeoutSec = 30,
    [int]$PollMs = 400
  )
  return Get-UiWindow -Title $Title -ProcessId $ProcessId -Hwnd $Hwnd -TimeoutSec $TimeoutSec -PollMs $PollMs
}

function Wait-UiCondition {
  <#
  .SYNOPSIS
    Polls a scriptblock until it returns truthy or the timeout expires.
  #>
  [CmdletBinding()]
  param(
    [scriptblock]$Condition = $null,
    [int]$TimeoutSec = 30,
    [int]$PollMs = 400
  )
  $t0 = Get-Date
  try {
    if ($Condition -eq $null) {
      return New-UiFail -Error 'condition-required' -LocatedBy 'condition' -Diagnostics @{ op = 'Wait-UiCondition' }
    }
    if ($TimeoutSec -lt 1) { $TimeoutSec = 1 }
    if ($PollMs -lt 100) { $PollMs = 100 }
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $attempts = 0
    $lastResult = ''
    $lastError = ''
    while ($true) {
      $attempts++
      try {
        $r = & $Condition
        if ($r -is [array]) { $r = $r[-1] }
        $s = [string]$r
        if ($s.Length -gt 200) { $s = $s.Substring(0, 200) }
        $lastResult = $s
        if ($r) {
          return [pscustomobject]@{
            ok = $true; error = ''; locatedBy = 'condition'; rect = $null
            attempts = $attempts
            elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
            diagnostics = @{ lastResult = $lastResult }
          }
        }
      } catch {
        $lastError = $_.Exception.Message
        if ($lastError.Length -gt 200) { $lastError = $lastError.Substring(0, 200) }
      }
      if ((Get-Date) -ge $deadline) { break }
      Start-Sleep -Milliseconds $PollMs
    }
    return New-UiFail -Error 'condition-timeout' -LocatedBy 'condition' -Diagnostics @{
      op = 'Wait-UiCondition'; timeoutSec = $TimeoutSec; attempts = $attempts
      lastResult = $lastResult; lastError = $lastError
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -LocatedBy 'condition' -Diagnostics @{ op = 'Wait-UiCondition' }
  }
}

function Save-UiShot {
  <#
  .SYNOPSIS
    Captures ONLY the target window rect to a PNG (GetWindowRect + CopyFromScreen,
    same approach as tools/acceptance/sp30_ui_hooks_capture.ps1). No full-screen capture.
  #>
  [CmdletBinding()]
  param(
    $Window = $null,
    [long]$Hwnd = 0,
    [int]$ExpectedPid = 0,
    [string]$Path = '',
    [int]$TimeoutSec = 10,
    [int]$PollMs = 400,
    [switch]$BringToFront
  )
  $t0 = Get-Date
  try {
    if ($Path -eq '') {
      return New-UiFail -Error 'path-required' -LocatedBy 'window-rect' -Diagnostics @{ op = 'Save-UiShot' }
    }
    $t = Resolve-UiTargetWindow -Window $Window -Hwnd $Hwnd -ExpectedPid $ExpectedPid -TimeoutSec $TimeoutSec -PollMs $PollMs -Op 'Save-UiShot'
    if (-not $t.ok) {
      $t.locatedBy = 'window-rect'
      return $t
    }
    $h = [IntPtr]$t.hwndLong
    if ($BringToFront) {
      [UiAutoNative]::EnsureForeground($h) | Out-Null
      Start-Sleep -Milliseconds 900
    }
    $rectArr = [UiAutoNative]::GetRectArr($h)
    if ($rectArr -eq $null -or $rectArr[2] -le 0 -or $rectArr[3] -le 0) {
      return New-UiFail -Error 'window-rect-empty' -LocatedBy 'window-rect' -Diagnostics @{ op = 'Save-UiShot' }
    }
    $rect = New-UiRect -X $rectArr[0] -Y $rectArr[1] -Width $rectArr[2] -Height $rectArr[3]
    $parent = Split-Path -Parent $Path
    if ($parent -ne '' -and -not (Test-Path -LiteralPath $parent)) {
      New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    $bmp = New-Object System.Drawing.Bitmap($rect.width, $rect.height)
    try {
      $g = [System.Drawing.Graphics]::FromImage($bmp)
      try {
        $g.CopyFromScreen($rect.x, $rect.y, 0, 0, (New-Object System.Drawing.Size($rect.width, $rect.height)))
      } finally {
        $g.Dispose()
      }
      $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
      $bmp.Dispose()
    }
    $info = Get-Item -LiteralPath $Path
    $hasher = [System.Security.Cryptography.SHA256]::Create()
    try {
      $fs = [System.IO.File]::OpenRead($Path)
      try {
        $hash = $hasher.ComputeHash($fs)
        $hex = ([System.BitConverter]::ToString($hash)).Replace('-', '').ToLowerInvariant()
      } finally {
        $fs.Close()
      }
    } finally {
      $hasher.Dispose()
    }
    return [pscustomobject]@{
      ok = $true; error = ''; locatedBy = 'window-rect'; rect = $rect
      path = $Path; width = $rect.width; height = $rect.height
      sizeBytes = $info.Length; sha256 = $hex
      diagnostics = @{
        op = 'Save-UiShot'; broughtToFront = [bool]$BringToFront
        elapsedMs = [long]((Get-Date) - $t0).TotalMilliseconds
      }
    }
  } catch {
    return New-UiFail -Error ('exception: ' + $_.Exception.Message) -LocatedBy 'window-rect' -Diagnostics @{ op = 'Save-UiShot' }
  }
}
