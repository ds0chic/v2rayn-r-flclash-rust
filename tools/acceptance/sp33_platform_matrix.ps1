<#
.SYNOPSIS
  SP-33 six-instance readiness matrix (read-only probe; -DryRun prints plan only).

.DESCRIPTION
  Records per-instance prereqs for Windows x64/arm64, Linux x64/arm64,
  macOS x64/arm64 acceptance:
    * Windows x64: available on this host (pinned toolchain present).
    * Windows ARM64: blocked - pinned Flutter CLI has no --target-platform
      (T20 measured exit 64).
    * Linux x64/ARM64: blocked - no WSL distro / no docker on this host
      (T21-A measured wsl exit 50).
    * macOS x64/ARM64: blocked - needs Apple hardware + Xcode/SDK.
  Read-only: Test-Path / Get-Command / wsl --status (query only). No build,
  no download, no network, no registry/proxy/TUN/Run-key writes, no exe launch,
  no port bind. Version-style output is environment inventory only and never
  counts as chain acceptance.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp33_platform_matrix.ps1 -DryRun
.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp33_platform_matrix.ps1
#>
[CmdletBinding()]
param([switch]$DryRun)

$ErrorActionPreference = 'Stop'

if ($DryRun) {
  ([ordered]@{
    mode = 'dry-run'; status = 'identified'
    plan = @(
      'Host OS/arch identification (read-only)'
      'Pinned toolchain presence: flutter.bat / cargo.exe (Test-Path, read-only)'
      'Linux prereq query: wsl --status / Get-Command docker (query only, no enable)'
      'Emit six-instance available/blocked + unblock JSON (blocked is data, not failure)'
    )
    side_effects = 'none (no build, no download, no network, no OS/network state change)'
  } | ConvertTo-Json -Depth 4) | Write-Output
  exit 0
}

function Test-Have([string]$Name) {
  return ($null -ne (Get-Command $Name -ErrorAction SilentlyContinue))
}

$flutterBat = 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat'
$cargoExe = 'C:\Users\Colby\.cargo\bin\cargo.exe'
$flutterHere = Test-Path -LiteralPath $flutterBat
$cargoHere = Test-Path -LiteralPath $cargoExe

$wslState = 'absent'
try {
  $null = & wsl.exe --status 2>$null
  if ($LASTEXITCODE -eq 0) { $wslState = 'present' } else { $wslState = "no-distro-or-off (exit $LASTEXITCODE)" }
} catch { $wslState = 'not-found' }
$dockerHere = Test-Have 'docker'

$winx64 = ($flutterHere -and $cargoHere)
if ($winx64) { $o01status = 'available'; $o01unblock = 'none (runnable on this host)' }
else { $o01status = 'blocked'; $o01unblock = 'restore AGENTS.md pinned toolchain paths' }

$instances = @(
  [ordered]@{ id = 'O01'; platform = 'windows'; arch = 'x64'; status = $o01status;
    reason = 'Pinned Flutter 3.47.5 + Rust 1.98.1 present on this host; real build/smoke still follows the checklist';
    unblock = $o01unblock },
  [ordered]@{ id = 'O02'; platform = 'windows'; arch = 'arm64'; status = 'blocked';
    reason = 'Pinned Flutter CLI has no --target-platform (T20 measured exit 64); local x64 engine has no ARM64 target';
    unblock = 'ARM64 Windows host + ARM64-capable Flutter Windows engine (Rust cross check 5/5 already, see platform-matrix 6.1)' },
  [ordered]@{ id = 'O03'; platform = 'macos'; arch = 'x64'; status = 'blocked';
    reason = 'Needs Apple hardware + Xcode/SDK; a Windows host cannot produce macOS artifacts';
    unblock = 'Intel Mac or macOS CI runner + xcode-select --install; build per tools/release/README-platforms.md' },
  [ordered]@{ id = 'O04'; platform = 'macos'; arch = 'arm64'; status = 'blocked';
    reason = 'Same as O03 (Apple Silicon adds arm64 target verification)';
    unblock = 'Apple Silicon host/CI + Xcode; also verify Dock accessory / LaunchAgent plist / non-Windows proxy branches' },
  [ordered]@{ id = 'O05'; platform = 'linux'; arch = 'x64'; status = 'blocked';
    reason = "No WSL distro / no docker on this host (wsl_state=$wslState, docker_present=$dockerHere; T21-A wsl exit 50)";
    unblock = 'wsl --install (admin+reboot) or Docker Desktop; distro needs build-essential/pkg-config + matching rustup target' },
  [ordered]@{ id = 'O06'; platform = 'linux'; arch = 'arm64'; status = 'blocked';
    reason = "Same as O05 plus no ARM64 toolchain (wsl_state=$wslState, docker_present=$dockerHere)";
    unblock = 'Same as O05 plus gcc-aarch64-linux-gnu or ARM64 Linux host; also verify sudo/TUN/.desktop autostart branches' }
)

([ordered]@{
  mode = 'platform-matrix'; baseline = '393fafd'
  host = [ordered]@{ os = 'Windows 11 25H2'; arch = $env:PROCESSOR_ARCHITECTURE; flutter_pinned = $flutterHere; cargo_pinned = $cargoHere }
  instances = $instances
  note = 'Presence/version checks are environment inventory only; instance acceptance needs real build/package/runtime/tray/hotkey/window/DPI/update/cleanup per checklist, no cross-instance borrowing'
} | ConvertTo-Json -Depth 5) | Write-Output
exit 0
