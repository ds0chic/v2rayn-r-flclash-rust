# SP-30 UIA→semantics bridge (2026-10-08)

Minimal viable bridge for UIA-blind agents: an ARMED-ONLY hook that dumps the
Flutter semantics tree (stable identifiers/labels + global rects) to a JSON
file. The agent reads the file to locate elements, then performs REAL
mouse/keyboard input itself. The hook never clicks, changes state, or touches
the network/OS beyond the file write.

## Engine gap evidence

- Probe evidence (task context): UIA/MSAA see only the FLUTTERVIEW pane; the
  pinned Flutter Windows engine (`flutter_windows.dll`) lacks
  `IAccessibleEx`/`AccessibilityBridgeWindows` — no accessibility bridge is
  exposed, so agents cannot locate widgets via UIA.
- Consequence: `V2RAYN_R_ENABLE_SEMANTICS=1` (existing `forceSemantics` in
  `main.dart`) forces the framework semantics tree on, but with no engine
  bridge it is still invisible to UIA. This hook exports that same tree to a
  file instead.

## Implementation

- New: `apps/desktop/lib/perf/semantics_dump_hook.dart` (`SemanticsDumpHook`).
- Wired in `apps/desktop/lib/main.dart`: `if (SemanticsDumpHook.enabled)
  SemanticsDumpHook.start();` right after the existing `forceSemantics` block.
- Arming (same pattern as `ImportSynthHook`/`T18Bench`): compile-time
  `V2RAYN_R_SMOKE_ARMED=true` AND env `V2RAYN_R_SEMANTICS_DUMP=<absolute file
  path>`. Unarmed/official builds ignore the env var entirely
  (`File(outPath).isAbsolute` also enforced; non-absolute values disable).
- `start()`: calls `SemanticsBinding.instance.ensureSemantics()` and keeps the
  `SemanticsHandle` alive in a static field (works even if
  `V2RAYN_R_ENABLE_SEMANTICS` is unset), writes one dump immediately, then a
  `Timer.periodic(400 ms)` walk. Every pass is `try/catch` — never crashes
  the app. Atomic write: temp file in the same directory + rename (with
  delete+rename fallback for Windows replace semantics).
- Exact root accessor used (Flutter 3.47.5): per `RenderView` in
  `RendererBinding.instance.renderViews`,
  `renderView.owner?.semanticsOwner?.rootSemanticsNode`
  (`RenderObject.owner` → `PipelineOwner.semanticsOwner` →
  `SemanticsOwner.rootSemanticsNode`, i.e. `_nodes[0]`). Rejected alternative:
  `RenderObject.debugSemantics` returns null in release builds (by design), so
  it is unusable for armed release drivers.
- Per node: `SemanticsNode.getSemanticsData()` → `identifier`/`label`/`value`;
  `node.rect` mapped through accumulated `node.transform` chain to global
  logical pixels via `Matrix4.storage` (column-major 2D:
  `x' = m0*x + m4*y + m12`, `y' = m1*x + m5*y + m13`); actions decoded from the
  `SemanticsData.actions` bitfield via `dart:ui SemanticsAction.values`
  (`index` bit → `name`); flags from `SemanticsData.flagsCollection`
  (`SemanticsFlags`: `isSelected`/`isEnabled`/`isFocused` are `Tristate`,
  `isChecked` is `CheckedState`).
- Window info from `RendererBinding.instance.platformDispatcher.views.first`:
  `devicePixelRatio`, logical size = `physicalSize / devicePixelRatio`.

## JSON schema

```jsonc
{
  "tool": "v2rayn-r semantics_dump",
  "status": "ok | waiting",   // "waiting" before first frame / empty tree
  "pid": 1234,
  "devicePixelRatio": 1.5,
  "windowSize": [1280.0, 800.0],   // logical pixels [w, h]
  "nodeCount": 42,
  "roots": [
    {
      "index": 0,                  // render-view index (single-view app: 0)
      "tree": {
        "id": 0,                   // framework SemanticsNode.id
        "identifier": "",          // SemanticsNode identifier (stable selector)
        "label": "…",
        "value": "…",
        "rect": [x, y, w, h],      // global logical pixels
        "actions": ["tap", "focus", /* dart:ui SemanticsAction names */],
        "flags": {
          "selected": true,        // present only when applicable (Tristate != none)
          "checked": true,         // true | false | "mixed"
          "enabled": false,        // present only when applicable
          "focused": false         // present only when applicable
        },
        "children": [ /* nested nodes, same shape */ ]
      }
    }
  ]
}
```

`tree` is `null` while that view has no semantics root yet.

## Checks (2026-10-08, targeted only)

- `dart format lib\perf\semantics_dump_hook.dart lib\main.dart` → 1 file
  changed by formatter; re-run with `--set-exit-if-changed` → 0 changed.
- `flutter analyze --no-pub lib\perf\semantics_dump_hook.dart lib\main.dart`
  → No issues found.
- `flutter test test\repair\sp_16_sub_entry_test.dart` → All tests passed
  (9/9, incl. SP-18 cases in the same file).
- NOT run (per task): cargo, release/armed builds, whole-suite gate.

## Integrator run command

Build armed (integrator builds), then run with the dump path set — never
port 10808, no OS side effects, synthetic only:

```powershell
flutter build windows --release --dart-define=V2RAYN_R_SMOKE_ARMED=true
$env:V2RAYN_R_SEMANTICS_DUMP = 'C:\Users\Colby\AppData\Local\Temp\opencode\semantics-dump.json'
.\build\windows\x64\runner\Release\v2rayn_desktop.exe
# every ~400 ms the file is atomically rewritten; agent reads rects, then
# drives REAL mouse/keyboard input itself.
```

Unset the env var (or use an unarmed build) and the hook is fully inert.

## UiAutomation.ps1 neutral-target validation (2026-10-08)

Reusable agent layer: `tools/ui/UiAutomation.ps1` (dot-sourceable) + agent
guide `tools/ui/README.md`. Every function returns
`{ ok, error, locatedBy, rect, ... }`, never throws for expected failures,
takes `-TimeoutSec`, and redacts `-Text` (only `textLength` +
`textSha256Prefix` reported). Real input is SendInput-only (mouse move +
down/up incl. double-click timing; unicode/virtual-key keystrokes) — no UIA
InvokePattern, no app API. PID gate: every action re-verifies the HWND is
live and owned by the expected PID (`pid-mismatch-refused` otherwise).

Neutral target: `notepad.exe` launched with a `%TEMP%` seed file (owned
launcher PID), found + real-clicked + typed + screenshotted, then only the
owned PIDs stopped. No cargo/flutter, no 10808/proxy/registry/TUN contact.

Exact commands (all `ok` unless noted; `. tools/ui/UiAutomation.ps1` first):

```powershell
$proc = Start-Process notepad.exe -ArgumentList $tmp -PassThru   # launcher pid
$w     = Get-UiWindow -Title 'uia_probe_neutral\.txt' -TimeoutSec 10
$eDoc  = Get-UiElement -Window $w -ControlType 'Document' -TimeoutSec 10
$eMenu = Get-UiElement -Window $w -AutomationId 'MenuBar' -TimeoutSec 10
$sem   = Get-UiSemantics -Path $snap
$eSem  = Get-UiElement -Window $w -SemanticsPath $snap -Identifier 'notepad.editor'
$eNo   = Get-UiElement -Window $w -X 100 -Y 100                  # expect refusal
$eCrd  = Get-UiElement -Window $w -X 928 -Y 603 -AllowCoordinates # expect marked
$rc = Invoke-UiRealRightClick -Element $eDoc
$ek = Send-UiKey -Window $w -Key 'ESC'
$dc = Invoke-UiRealDoubleClick -Element $eDoc
$sc = Invoke-UiRealClick -Element $eDoc
$t1 = Send-UiText -Window $w -Text 'UIA-NEUTRAL-PROBE-20261008-A '
$en = Send-UiKey -Window $w -Key 'ENTER'
$t2 = Send-UiText -Window $w -Text 'UIA-NEUTRAL-PROBE-20261008-B'
$sv = Send-UiKey -Window $w -Key 'S' -Modifiers @('Ctrl')
$sh = Save-UiShot -Window $w -Path $shot -BringToFront
Stop-Process -Id <windowPid>; Stop-Process -Id <launcherPid>    # owned only
```

Results (run 2, after two module bugs found by run 1 were fixed — see below):

| step | ok | detail |
|---|---|---|
| window by launcher PID | `false` (`window-not-found`) | correct: Win11 Notepad hosts the window in a child proc |
| window by title regex | `true` | `uia_probe_neutral.txt - Notepad`, class `Notepad`, hwnd `0x6D2070`, windowPid 43452, rect 208,208,1440x753; WMI parent of 43452 == launcher 39716 (owned) |
| window by window PID | `true` | `hwndMatch=True` |
| UIA Document | `true`, `locatedBy=uia` | rect 214,283,1428x640, center 928,603 |
| UIA MenuBar | `true`, `locatedBy=uia` | rect 214,250,168x32, center 298,266 |
| semantics file | `true`, count=1 | flat hook-style node matched: `locatedBy=semantics`, same rect |
| coords w/o `-AllowCoordinates` | `false` (`coordinates-require-allow`) | refusal works |
| coords with `-AllowCoordinates` | `true`, `locatedBy=coordinates` | point 928,603, brittle-use warning in diagnostics |
| right / double / single real click | all `true` | `SendInput move + down/up`, points 928,603, pidVerified 43452 |
| ESC / ENTER / Ctrl+S keys | all `true` | virtual-key SendInput (WIN key stays blocked) |
| type x2 | `true` | 29 + 28 chars sent; text redacted to `textLength` + sha prefix only |
| file verify | `hasA=True hasB=True bytes=61` | typed text survived Ctrl+S to the owned temp file |
| window-rect shot | `true` | 1440x753 PNG, 70219 bytes, sha256 `689aea5f…669` |
| cleanup | both PIDs dead, temp files removed | no residue; no other process touched |

Module bugs caught by run 1 and fixed (both re-proven in run 2):

1. `uia-controltype-unknown` for `Document`: `ControlType.*` members are
   static FIELDS, not properties — `Find-UiAutomationControlType` used
   `GetProperty` only. Fixed to `GetField` first (property fallback kept).
2. `Send-UiKey` threw on `[short]` argument binding to the C# `KeyVk(short,
   ...)` P/Invoke. Fixed by widening the C# signature to `int` (cast to
   `short` inside). Added `ConvertTo-UiInt` saturation for extreme UIA rects
   found during discovery.

Nested-shape interop (same day, after this file's hook schema appeared):
`Get-UiSemantics` now also flattens the real `roots[].tree.children`
hierarchy (`Expand-UiSemSubtree`; `rect: [x,y,w,h]` arrays; `value` label
alias). Proven with a synthetic 4-node nested dump → `count=4`, all rects
and identifiers recovered (`settings.tab.tun`, `deep.node` incl.).

Side effects: none beyond the owned notepad (closed) and `%TEMP%\opencode`
scratch files (removed). 10808 untouched; no proxy/registry/routes/TUN;
ledger/FLD/manifest untouched; nothing committed.
