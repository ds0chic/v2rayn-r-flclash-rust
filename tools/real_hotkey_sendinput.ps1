# Real global-hotkey test helper (SR-REAL-HOTKEY / SR-HOTKEY-CONFLICT).
#
# Three modes, all using only user32 SendInput / RegisterHotKey:
#   - Send  : inject a real key combination (default). Used to trigger a hotkey
#             registered by the app under test through the OS input queue.
#   - Probe : try RegisterHotKey for the combination, then immediately
#             UnregisterHotKey. Success proves the combination is free at the
#             OS level (no hotkey_manager residue); failure reports the Win32
#             error (1409 = ERROR_HOTKEY_ALREADY_REGISTERED).
#   - Hold  : RegisterHotKey the combination and keep this process alive until
#             the parent closes stdin, then UnregisterHotKey. Used as an
#             independent, external owner to prove the app reports the conflict
#             instead of faking success.
#
# The allowed test combinations are Ctrl+Alt+F11 / Ctrl+Alt+F12 only. Never
# touches the system proxy, registry, routing or TUN.
param(
  [Parameter(Mandatory = $true)][string]$Combo,
  [switch]$Probe,
  [switch]$Hold,
  [int]$HoldMs = 80
)

$ErrorActionPreference = 'Stop'

$VK = @{
  'ctrl' = 0x11; 'control' = 0x11
  'alt' = 0x12
  'shift' = 0x10
  'win' = 0x5B; 'meta' = 0x5B; 'lwin' = 0x5B
}
for ($i = 1; $i -le 24; $i++) { $VK["f$i"] = 0x70 + ($i - 1) }
for ($i = 0; $i -le 9; $i++) { $VK["$i"] = 0x30 + $i }
for ($i = 0; $i -lt 26; $i++) {
  $ch = [string][char](0x61 + $i)
  $VK[$ch] = 0x41 + $i
}

$tokens = $Combo.ToLower().Split('+') | ForEach-Object { $_.Trim() } |
  Where-Object { $_ -ne '' }
if ($tokens.Count -lt 1) { throw "empty combo" }
$mainToken = $tokens[-1]
$modTokens = @()
if ($tokens.Count -gt 1) { $modTokens = $tokens[0..($tokens.Count - 2)] }

if (-not $VK.ContainsKey($mainToken)) { throw "unsupported main key: $mainToken" }
$mainVk = [uint16]$VK[$mainToken]
$modVks = New-Object System.Collections.Generic.List[uint16]
$modFlags = [uint32]0
foreach ($m in $modTokens) {
  if (-not $VK.ContainsKey($m)) { throw "unsupported modifier: $m" }
  $modVks.Add([uint16]$VK[$m])
  switch ($m) {
    { $_ -in 'ctrl', 'control' } { $modFlags = $modFlags -bor 0x0002 }
    'alt' { $modFlags = $modFlags -bor 0x0001 }
    'shift' { $modFlags = $modFlags -bor 0x0004 }
    { $_ -in 'win', 'meta', 'lwin' } { $modFlags = $modFlags -bor 0x0008 }
  }
}

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class RealHotkeyInterop {
  [StructLayout(LayoutKind.Sequential)]
  public struct INPUT { public uint type; public InputUnion U; }

  [StructLayout(LayoutKind.Sequential)]
  public struct MOUSEINPUT {
    public int dx;
    public int dy;
    public uint mouseData;
    public uint dwFlags;
    public uint time;
    public IntPtr dwExtraInfo;
  }

  [StructLayout(LayoutKind.Sequential)]
  public struct KEYBDINPUT {
    public ushort wVk;
    public ushort wScan;
    public uint dwFlags;
    public uint time;
    public IntPtr dwExtraInfo;
  }

  [StructLayout(LayoutKind.Sequential)]
  public struct HARDWAREINPUT {
    public uint uMsg;
    public ushort wParamL;
    public ushort wParamH;
  }

  // The union must be as large as the largest member (MOUSEINPUT, 32 bytes on
  // x64); otherwise Marshal.SizeOf(INPUT) is wrong and SendInput returns 0 with
  // ERROR_INVALID_PARAMETER (87).
  [StructLayout(LayoutKind.Explicit)]
  public struct InputUnion {
    [FieldOffset(0)] public MOUSEINPUT mi;
    [FieldOffset(0)] public KEYBDINPUT ki;
    [FieldOffset(0)] public HARDWAREINPUT hi;
  }

  private const uint INPUT_KEYBOARD = 1;
  private const uint KEYEVENTF_KEYUP = 0x0002;

  [DllImport("user32.dll", SetLastError = true)]
  private static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);

  [DllImport("user32.dll", SetLastError = true)]
  private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

  [DllImport("user32.dll", SetLastError = true)]
  private static extern bool UnregisterHotKey(IntPtr hWnd, int id);

  private static INPUT Key(ushort vk, bool up) {
    INPUT i = new INPUT();
    i.type = INPUT_KEYBOARD;
    i.U.ki.wVk = vk;
    i.U.ki.wScan = 0;
    i.U.ki.dwFlags = up ? KEYEVENTF_KEYUP : 0u;
    i.U.ki.time = 0;
    i.U.ki.dwExtraInfo = IntPtr.Zero;
    return i;
  }

  public static uint Send(ushort[] mods, ushort key) {
    var list = new List<INPUT>();
    foreach (var m in mods) list.Add(Key(m, false));
    list.Add(Key(key, false));
    list.Add(Key(key, true));
    for (int i = mods.Length - 1; i >= 0; i--) list.Add(Key(mods[i], true));
    var arr = list.ToArray();
    return SendInput((uint)arr.Length, arr, Marshal.SizeOf(typeof(INPUT)));
  }

  public static int Probe(uint fsModifiers, uint key) {
    const int id = 0x9A01;
    bool ok = RegisterHotKey(IntPtr.Zero, id, fsModifiers, key);
    int err = Marshal.GetLastWin32Error();
    if (ok) UnregisterHotKey(IntPtr.Zero, id);
    return ok ? 0 : err;
  }

  private const int HoldId = 0x9A03;

  public static int Hold(uint fsModifiers, uint key) {
    bool ok = RegisterHotKey(IntPtr.Zero, HoldId, fsModifiers, key);
    return ok ? 0 : Marshal.GetLastWin32Error();
  }

  public static void Release() {
    UnregisterHotKey(IntPtr.Zero, HoldId);
  }
}
'@

if ($Probe) {
  $err = [RealHotkeyInterop]::Probe($modFlags, $mainVk)
  [pscustomobject]@{
    mode      = 'probe'
    combo     = $Combo
    modFlags  = $modFlags
    mainVk    = $mainVk
    free      = ($err -eq 0)
    lastError = $err
  } | ConvertTo-Json -Compress
  if ($err -ne 0) { exit 2 }
  exit 0
}

if ($Hold) {
  $err = [RealHotkeyInterop]::Hold($modFlags, $mainVk)
  [pscustomobject]@{
    mode      = 'hold'
    combo     = $Combo
    modFlags  = $modFlags
    mainVk    = $mainVk
    held      = ($err -eq 0)
    lastError = $err
  } | ConvertTo-Json -Compress
  if ($err -ne 0) { exit 2 }
  try {
    # Block until the parent closes stdin, then release the combination.
    [void][Console]::In.ReadLine()
  } finally {
    [RealHotkeyInterop]::Release()
  }
  exit 0
}

# Send: modifiers down, key down, key up, modifiers up (one atomic SendInput).
$sent = [RealHotkeyInterop]::Send($modVks.ToArray(), $mainVk)
$err = 0
if ($sent -eq 0) { $err = [Runtime.InteropServices.Marshal]::GetLastWin32Error() }
[pscustomobject]@{
  mode      = 'send'
  combo     = $Combo
  request   = ($modVks.Count * 2) + 2
  sent      = $sent
  lastError = $err
} | ConvertTo-Json -Compress
if ($sent -eq 0) { exit 3 }
exit 0
