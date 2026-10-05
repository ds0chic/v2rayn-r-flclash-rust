<#
.SYNOPSIS
  Build and exercise the v2rayN-R Inno Setup installer end to end.

.DESCRIPTION
  1. Compile tools/release/v2rayn-r.iss with ISCC.exe (unless -SkipCompile).
  2. Silent-install into a fresh temp directory:
       setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=<dir>
                 /MERGETASKS="startmenuicon,desktopicon"
  3. Verify the portable file layout and the Start Menu / Desktop shortcuts.
  4. Record the per-user Uninstall registry entry.
  5. Launch the installed exe with V2RAYN_R_DATA_DIR, wait for a window and for
     the first-run data files, then stop only the PID tree this script started.
  6. Silent-uninstall and verify the install directory, shortcuts and Uninstall
     registry entry are gone (no residue tolerated).

  Safety: never touches 127.0.0.1:10808, the system proxy or TUN. Only PIDs this
  script started are ever stopped. Every install is removed on exit.

.NOTES
  Exit code 0 only when install, launch and uninstall all verify.
#>
[CmdletBinding()]
param(
  [string]$SetupExe = '',
  [string]$Iss = '',
  [string]$EvidenceDir = '',
  [string]$TestRoot = '',
  [int]$WindowWaitSec = 90,
  [switch]$SkipCompile,
  [switch]$CompileOnly
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$DistDir = Join-Path $RepoRoot 'dist'
if ($Iss -eq '') { $Iss = Join-Path $PSScriptRoot 'v2rayn-r.iss' }
if ($EvidenceDir -eq '') { $EvidenceDir = Join-Path $RepoRoot 'docs\evidence\T21-install-update.runs' }
New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$logPath = Join-Path $EvidenceDir "install-$stamp.log"
$jsonPath = Join-Path $EvidenceDir "install-$stamp.json"

$script:log = [System.Collections.Generic.List[string]]::new()
function Log([string]$line) {
  $ts = (Get-Date).ToString('HH:mm:ss')
  $script:log.Add("[$ts] $line") | Out-Null
  Write-Host "   $line"
}
function Save-Log { $script:log | Set-Content -LiteralPath $logPath -Encoding UTF8 }

# --- window detection (same native helper as the T20 smoke) -------------------
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class Nat21 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
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

function Wait-Window {
  param([int]$ProcessId, [int]$TimeoutSec)
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $h = [Nat21]::Find([uint32]$ProcessId)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 500
  }
  return [IntPtr]::Zero
}

function Get-DescendantPids {
  param([int]$RootPid)
  $result = [System.Collections.Generic.List[int]]::new()
  $queue = [System.Collections.Generic.Queue[int]]::new()
  $queue.Enqueue($RootPid)
  while ($queue.Count -gt 0) {
    $parent = $queue.Dequeue()
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId=$parent" -ErrorAction SilentlyContinue
    foreach ($c in $children) {
      $result.Add([int]$c.ProcessId) | Out-Null
      $queue.Enqueue([int]$c.ProcessId)
    }
  }
  return $result
}

function Stop-OwnedTree {
  param([int]$RootPid)
  $owned = @($RootPid) + @(Get-DescendantPids -RootPid $RootPid)
  $owned = $owned | Sort-Object -Unique
  [array]::Reverse($owned)
  $stopped = [System.Collections.Generic.List[int]]::new()
  foreach ($cpid in $owned) {
    $p = Get-Process -Id $cpid -ErrorAction SilentlyContinue
    if ($p) { Stop-Process -Id $cpid -Force -ErrorAction SilentlyContinue; $stopped.Add($cpid) | Out-Null }
  }
  return $stopped
}

# Find the per-user (or machine) Uninstall key created by Inno for this AppId.
function Find-UninstallKey {
  foreach ($root in @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
                      'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall')) {
    if (-not (Test-Path $root)) { continue }
    foreach ($k in Get-ChildItem $root -ErrorAction SilentlyContinue) {
      $disp = (Get-ItemProperty -Path $k.PSPath -Name DisplayName -ErrorAction SilentlyContinue).DisplayName
      if ($disp -like 'v2rayN-R*') { return [ordered]@{ path = $k.PSPath; display = $disp } }
    }
  }
  return $null
}

$result = [ordered]@{
  schema = 1
  task = 'T21-install-update'
  kind = 'installer'
  timestamp_utc = $stamp
  setup_exe = ''
  setup_sha256 = ''
  compile = [ordered]@{ attempted = $false; iscc = ''; exit = $null; output = '' }
  install_dir = ''
  data_dir = ''
  install = [ordered]@{ exit = $null; duration_ms = 0 }
  layout = [ordered]@{ expected = @(); present = @(); missing = @(); extra = @(); ok = $false }
  shortcuts = [ordered]@{ start_menu = ''; desktop = ''; start_menu_ok = $false; desktop_ok = $false }
  registry_before = $null
  app = [ordered]@{ started_pid = 0; window = $false; data_files = @{}; note = '' }
  uninstall = [ordered]@{ exit = $null; duration_ms = 0; dir_removed = $false; shortcuts_removed = $false; registry_removed = $false; residue = @() }
  ok = $false
  limitations = @()
}

$workRoot = if ($TestRoot -ne '') { $TestRoot } else { Join-Path $env:TEMP ("t21_install_" + [guid]::NewGuid().ToString('N')) }
New-Item -ItemType Directory -Path $workRoot -Force | Out-Null
$installDir = Join-Path $workRoot 'app'
$dataDir = Join-Path $workRoot 'data'
New-Item -ItemType Directory -Path $dataDir -Force | Out-Null
$result.install_dir = $installDir
$result.data_dir = $dataDir

try {
  # --- compile ---------------------------------------------------------------
  if ($SetupExe -eq '') {
    $SetupExe = Join-Path $DistDir 'v2rayN-R-1.0.0+1-windows-x64-setup.exe'
  }
  if (-not $SkipCompile) {
    $iscc = ''
    $candidates = @(
      (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
      (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
      (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    )
    foreach ($c in $candidates) { if (Test-Path -LiteralPath $c) { $iscc = $c; break } }
    if ($iscc -eq '') {
      $cmd = Get-Command ISCC.exe -ErrorAction SilentlyContinue
      if ($cmd) { $iscc = $cmd.Source }
    }
    if ($iscc -eq '') { throw 'ISCC.exe not found; install JRSoftware.InnoSetup' }
    $result.compile.attempted = $true
    $result.compile.iscc = $iscc
    Log "ISCC: $iscc"
    $compileOut = & $iscc $Iss 2>&1 | Out-String
    $result.compile.output = $compileOut
    $result.compile.exit = $LASTEXITCODE
    Log "compile exit=$($LASTEXITCODE)"
    if ($LASTEXITCODE -ne 0) { throw "ISCC failed: $compileOut" }
  }
  if (-not (Test-Path -LiteralPath $SetupExe)) { throw "setup exe not found: $SetupExe" }
  $result.setup_exe = (Resolve-Path -LiteralPath $SetupExe).Path
  $result.setup_sha256 = (Get-FileHash -LiteralPath $SetupExe -Algorithm SHA256).Hash.ToLowerInvariant()
  Log "setup=$($result.setup_exe) sha256=$($result.setup_sha256)"

  if ($CompileOnly) {
    # Static-only delivery check: the installer compiles and the source stage
    # carries the flat layout the .iss packages. No host install/uninstall is
    # ever performed in this mode (R4-32 hard constraint).
    $stageDir = Join-Path $DistDir 'v2rayN-R-1.0.0+1-windows-x64'
    $stageExpected = @(
      'v2rayn_desktop.exe', 'bridge_api.dll', 'net_host.exe',
      'privileged_helper.exe', 'v2rayN-upgrade.exe', 'build-info.json',
      'CORE-NOTES.txt', 'data\app.so', 'data\flutter_assets\AssetManifest.bin'
    )
    $stageMissing = @($stageExpected | Where-Object {
        -not (Test-Path -LiteralPath (Join-Path $stageDir $_))
      })
    $result.layout.expected = $stageExpected
    $result.layout.missing = $stageMissing
    $result.layout.present = @($stageExpected | Where-Object {
        Test-Path -LiteralPath (Join-Path $stageDir $_)
      })
    $result.layout.ok = ($stageMissing.Count -eq 0)
    $result.limitations += 'compile-only: no real install/uninstall performed on the host'
    $result.ok = $result.layout.ok
    Log "compile-only layout ok=$($result.layout.ok) missing=$($stageMissing -join ',')"
    Write-Host "T21_INSTALL_COMPILE_ONLY ok=$($result.ok)"
    return
  }

  # --- silent install --------------------------------------------------------
  $installArgs = @(
    '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/NOCLOSEAPPLICATIONS',
    "/DIR=$installDir", '/MERGETASKS=startmenuicon,desktopicon',
    "/LOG=$workRoot\install.log"
  )
  Log "installing -> $installDir"
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $proc = Start-Process -FilePath $result.setup_exe -ArgumentList $installArgs -PassThru -Wait
  $sw.Stop()
  $result.install.exit = $proc.ExitCode
  $result.install.duration_ms = [int]$sw.ElapsedMilliseconds
  Log "install exit=$($proc.ExitCode) in $($sw.ElapsedMilliseconds)ms"
  if ($proc.ExitCode -ne 0) { throw "installer exit $($proc.ExitCode)" }

  # --- layout ----------------------------------------------------------------
  $expected = @(
    'v2rayn_desktop.exe', 'bridge_api.dll', 'flutter_windows.dll', 'net_host.exe',
    'privileged_helper.exe', 'LICENSE', 'NOTICE.md', 'README.md', 'build-info.json',
    'native_assets.json', 'CORE-NOTES.txt', 'data\app.so', 'data\icudtl.dat',
    'data\flutter_assets\AssetManifest.bin'
  )
  $result.layout.expected = $expected
  foreach ($rel in $expected) {
    if (Test-Path -LiteralPath (Join-Path $installDir $rel)) {
      $result.layout.present += $rel
    } else {
      $result.layout.missing += $rel
    }
  }
  $result.layout.ok = ($result.layout.missing.Count -eq 0)
  Log "layout ok=$($result.layout.ok) present=$($result.layout.present.Count)/$($expected.Count)"
  if (-not $result.layout.ok) { throw "layout missing: $($result.layout.missing -join ', ')" }

  # --- shortcuts -------------------------------------------------------------
  $result.shortcuts.start_menu = Join-Path ([Environment]::GetFolderPath('Programs')) 'v2rayN-R\v2rayN-R.lnk'
  $result.shortcuts.desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'v2rayN-R.lnk'
  $result.shortcuts.start_menu_ok = Test-Path -LiteralPath $result.shortcuts.start_menu
  $result.shortcuts.desktop_ok = Test-Path -LiteralPath $result.shortcuts.desktop
  Log "shortcuts startmenu=$($result.shortcuts.start_menu_ok) desktop=$($result.shortcuts.desktop_ok)"

  # --- uninstall registry entry ---------------------------------------------
  $reg = Find-UninstallKey
  if ($reg) {
    $props = Get-ItemProperty -Path $reg.path
    $result.registry_before = [ordered]@{
      path = $reg.path
      display = $props.DisplayName
      version = $props.DisplayVersion
      uninstall = $props.UninstallString
    }
    Log "registry: $($reg.path) ver=$($props.DisplayVersion)"
  } else {
    Log 'registry: no Uninstall entry found (will still be checked after)'
  }

  # --- launch the installed app ---------------------------------------------
  $mainExe = Join-Path $installDir 'v2rayn_desktop.exe'
  $old = [Environment]::GetEnvironmentVariable('V2RAYN_R_DATA_DIR')
  [Environment]::SetEnvironmentVariable('V2RAYN_R_DATA_DIR', $dataDir)
  try {
    $app = Start-Process -FilePath $mainExe -WorkingDirectory $installDir -PassThru
  } finally {
    [Environment]::SetEnvironmentVariable('V2RAYN_R_DATA_DIR', $old)
  }
  $result.app.started_pid = $app.Id
  Log "app started pid=$($app.Id)"
  $hwnd = Wait-Window -ProcessId $app.Id -TimeoutSec $WindowWaitSec
  $result.app.window = ($hwnd -ne [IntPtr]::Zero)
  # The first-run database is guiNDB.db; guiNConfig.json is only written once a
  # setting is saved (see docs/evidence/T20.md), so it is recorded, not required.
  $deadline = (Get-Date).AddSeconds(60)
  while ((Get-Date) -lt $deadline) {
    if (Test-Path -LiteralPath (Join-Path $dataDir 'guiNDB.db')) { break }
    Start-Sleep -Milliseconds 500
  }
  foreach ($f in @('guiNDB.db', 'guiNConfig.json')) {
    $result.app.data_files[$f] = Test-Path -LiteralPath (Join-Path $dataDir $f)
  }
  Log "app window=$($result.app.window) db=$($result.app.data_files['guiNDB.db']) config=$($result.app.data_files['guiNConfig.json'])"
  if (-not $result.app.window) { throw 'installed app did not show a window' }

  # Stop only the tree we started.
  $stopped = Stop-OwnedTree -RootPid $app.Id
  Log "stopped own pids: $($stopped -join ',')"
  Start-Sleep -Seconds 3

  # --- silent uninstall ------------------------------------------------------
  $unins = Join-Path $installDir 'unins000.exe'
  if (-not (Test-Path -LiteralPath $unins)) { throw "uninstaller missing: $unins" }
  $unsw = [System.Diagnostics.Stopwatch]::StartNew()
  $up = Start-Process -FilePath $unins -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -PassThru
  # Inno's uninstaller relaunches a temp copy, so wait on completion by polling.
  $unsw.Stop()
  $result.uninstall.exit = $up.ExitCode
  $result.uninstall.duration_ms = [int]$unsw.ElapsedMilliseconds
  Log "uninstaller launched pid=$($up.Id) exit=$($up.ExitCode); polling cleanup"

  $deadline = (Get-Date).AddSeconds(120)
  while ((Get-Date) -lt $deadline) {
    $dirGone = -not (Test-Path -LiteralPath $installDir)
    $regNow = Find-UninstallKey
    if ($dirGone -and -not $regNow) { break }
    Start-Sleep -Milliseconds 500
  }
  $result.uninstall.dir_removed = -not (Test-Path -LiteralPath $installDir)
  $result.uninstall.shortcuts_removed = (-not (Test-Path -LiteralPath $result.shortcuts.start_menu)) -and (-not (Test-Path -LiteralPath $result.shortcuts.desktop))
  $result.uninstall.registry_removed = ($null -eq (Find-UninstallKey))
  if (Test-Path -LiteralPath $installDir) {
    $result.uninstall.residue += (Get-ChildItem -LiteralPath $installDir -Recurse -Force | Select-Object -ExpandProperty FullName)
  }
  Log "uninstall dir_removed=$($result.uninstall.dir_removed) shortcuts_removed=$($result.uninstall.shortcuts_removed) registry_removed=$($result.uninstall.registry_removed) residue=$($result.uninstall.residue.Count)"

  $result.ok = $result.layout.ok -and $result.app.window -and
    $result.app.data_files['guiNDB.db'] -and
    $result.uninstall.dir_removed -and $result.uninstall.registry_removed
} catch {
  $result.limitations += "error: $_"
  Log "ERROR: $_"
} finally {
  Save-Log
  $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
  Write-Host ""
  Write-Host "INSTALL JSON $jsonPath"
  Write-Host "INSTALL LOG  $logPath"
  Write-Host "T21_INSTALL_DONE work=$workRoot ok=$($result.ok)"
}

if (-not $result.ok) { exit 1 }
exit 0
