# UiAutomation.ps1 — agent usage guide

Reusable desktop UI automation layer for agents. Dot-sourceable, no side
effects on import, no app-specific knowledge.

```powershell
. tools/ui/UiAutomation.ps1
```

Every function returns a structured object
`{ ok, error, locatedBy, rect, ... }`, never throws for expected failures,
takes `-TimeoutSec`, and redacts `-Text` (only `textLength` +
`textSha256Prefix`, first 16 hex chars of SHA-256 over UTF-8, are reported).

## Entry points (exact commands)

```powershell
# 1. Find the window you started (prefer -ProcessId of your OWNED pid).
$w = Get-UiWindow -ProcessId $pid -TimeoutSec 15
$w = Get-UiWindow -Title 'my app.*' -TimeoutSec 15      # regex on title
$w = Get-UiWindow -Hwnd 123456 -ProcessId $pid          # rebind + PID check

# 2. Locate an element (priority: UIA -> semantics file -> coordinates).
$e = Get-UiElement -Window $w -AutomationId 'loginBtn'
$e = Get-UiElement -Window $w -Name 'Sign in' -ControlType 'Button'
$e = Get-UiElement -Window $w -ControlType 'Edit'
$e = Get-UiElement -Window $w -SemanticsPath $snap -Identifier 'settings.tab.tun'
$e = Get-UiElement -Window $w -SemanticsPath $snap -Label 'TUN'
$e = Get-UiElement -Window $w -X 640 -Y 320 -AllowCoordinates   # fallback only

# 3. REAL input (SendInput only; never InvokePattern, never app APIs).
$c = Invoke-UiRealClick -Element $e
$c = Invoke-UiRealDoubleClick -Element $e
$c = Invoke-UiRealRightClick -X 640 -Y 320 -Window $w -AllowCoordinates
$t = Send-UiText -Window $w -Text 'synthetic probe text'
$k = Send-UiKey -Window $w -Key 'ENTER'
$k = Send-UiKey -Window $w -Key 'S' -Modifiers @('Ctrl')   # Ctrl+S
$k = Send-UiKey -Window $w -Key 'ESC'

# 4. Wait / capture / semantics file.
$w2 = Wait-UiWindow -Title 'Save As' -TimeoutSec 20
$wc = Wait-UiCondition -Condition { (Get-Process -Id $pid -ErrorAction SilentlyContinue) -eq $null } -TimeoutSec 10
$s = Save-UiShot -Window $w -Path "$env:TEMP\shot.png" -BringToFront
$sem = Get-UiSemantics -Path $snap
```

Check `$r.ok` after every call; on `$false`, read `$r.error` +
`$r.diagnostics` (error codes: `window-not-found`, `uia-element-not-found`,
`semantics-no-match`, `coordinates-require-allow`, `pid-mismatch-refused`,
`hwnd-not-live`, `ascii-only`, `key-unknown`, `key-blocked`,
`condition-timeout`, `window-rect-empty`, ...).

## Selector priority

| # | Selector | Args | `locatedBy` |
|---|----------|------|-------------|
| 1 | UIA tree | `-AutomationId` / `-Name` (exact) / `-ControlType` (e.g. `Edit`, `Button`; `ControlType.X` also accepted) | `uia` |
| 2 | Semantics snapshot file | `-SemanticsPath` + `-Identifier` / `-Label` (case-insensitive exact) | `semantics` |
| 3 | Raw screen coords | `-X -Y` **plus** `-AllowCoordinates` | `coordinates` |

Priority is strict: if UIA selectors are present, only UIA is tried (a miss
returns `uia-element-not-found`, it does NOT silently fall through to
coordinates). Coordinate use must always be explicit and is always marked.

## Semantics snapshot file schema (written by our armed app hook)

Flat contract (what `Get-UiSemantics` normalizes to):

```json
{ "nodes": [
  { "identifier": "settings.tab.tun", "label": "TUN",
    "rect": { "x": 120, "y": 200, "width": 90, "height": 28 } }
] }
```

Also accepted: a bare array; `.elements` / `.semantics` / `.children`
instead of `.nodes`; per-node `id`/`automationId` and `name`/`text`/`value`
aliases; rect as `[x, y, w, h]` array or `left/top/right/bottom`. Rects are
absolute screen pixels.

Nested hook shape (`apps/desktop/lib/perf/semantics_dump_hook.dart`,
`{ "roots": [ { "tree": { "identifier": ..., "rect": [x,y,w,h],
"children": [...] } } ] }`) is flattened automatically — every descendant
becomes a matchable node. See `Get-UiSemantics` / `ConvertTo-UiRect` /
`Expand-UiSemSubtree` for the exact rules.

## Safety boundaries (hard, enforced in code)

- PID gate: every action re-verifies the HWND is live AND still owned by the
  expected PID (`-Window` carries it; otherwise pass `-Hwnd` + `-ExpectedPid`).
  Mismatch is REFUSED (`pid-mismatch-refused`), never retargeted.
- No networking anywhere in this module: no sockets, no ports, and in
  particular 127.0.0.1:10808 is never referenced. No proxy/WinINET, no
  registry, no routes, no TUN changes — such code does not exist in this file.
- This module starts and kills nothing. Operate only on processes you started
  yourself; close only that owned PID tree
  (e.g. `Stop-Process -Id <ownedPid>`), never by process name.
- `Send-UiText` accepts printable ASCII + CR/LF/TAB only (`ascii-only`
  otherwise). `Send-UiKey` blocks the WIN key (`key-blocked`).
- Typing is redacted: results never echo `-Text`.

## Limitations (read before scripting clicks)

- Engine accessibility gap (the reason this module exists): the unarmed
  release (Flutter) exposes only the top-level window + one `FLUTTERVIEW`
  pane to UIA — no AutomationIds/Names inside (see
  `tools/ui/probe_uia.ps1`: `descendant_count` stays 0/1, and
  `V2RAYN_R_ENABLE_SEMANTICS=1` only works on armed builds). For our app,
  strategy (1) will therefore miss in-app controls until the engine bridge
  lands; use strategy (2) with a hook-written snapshot file, or documented
  strategy (3) coordinates as a last resort.
- All clicks/keys go to whatever is at the screen point after an
  ensure-foreground: keep the target unobscured, mind multi-monitor/DPI
  (physical pixels), and expect focus steal while a script runs.
- UIA `-Name` is exact-match; regex/substring search is not implemented.
- `Send-UiText` sends keystroke-by-keystroke (slow for long text); prefer it
  for short synthetic strings, not bulk paste.
- Window search matches visible, titled top-level windows only (same rule as
  `tools/acceptance/sp30_ui_hooks_capture.ps1`).
- Neutral-target proof (notepad find + real-click + type + shot): see
  `docs/evidence/stable-port/SP-30/uia-semantics-bridge-2026-10-08.md`.
