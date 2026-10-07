# Wave B (local-only) — FLD-CFG-156/157/158 window geometry, controller/state level (2026-10-07)

Scope: `apps/desktop/lib/**` (except `features/settings/option_setting_window.dart`,
untouched — another workstream owns it) + `apps/desktop/test/**`. No Rust touched
(`crates/application/src/settings.rs` `get_window_size`/`save_window_size` and the
`WindowSizeItemDto` already exist). No ledger CSV / FLD doc / manifest edits. No commit.

## Investigation (read-only, HEAD `17f8970`)

- Rust store in place: `get_window_size` (TypeName locate, degenerate rows treated as
  absent) + `save_window_size` (same-TypeName upsert, else append) at
  `crates/application/src/settings.rs:167-197`; DTO `WindowSizeItemDto` at
  `crates/bridge_api/src/api/settings.rs:368-407`, riding inside `UiItemDto.window_size_item`.
- Gap confirmed by grep: **no dedicated FRB binding** for per-window get/save
  (`pub fn` scan of `crates/bridge_api/src/api/*` shows only whole-tree/group
  `get_settings`/`save_settings*`). Rows must ride the `UiItem` group via
  `saveSettingsGroup` + `load` — registered as gap G-1 below, not added.
- Dart had no canonical-row consumer: `UiShellController.applySettingsDocument`
  never reads `UiItem.WindowSizeItem`; `DesktopIntegration.start` only sets title +
  minimum size (Win32 runner owns persisted size); `ui_state.json` geometry
  (`WindowGeometry`, `isValid = width > 0 && height > 0`) is a separate local plane
  with the same validity rule as Rust.

## Implementation (2 new files, nothing else touched)

- `apps/desktop/lib/features/profiles/window_geometry.dart` — pure, dependency-free
  contract over canonical `UiItem.WindowSizeItem` rows, reusing `WindowGeometry`:
  - 156: `parseWindowSizeItems` (empty/non-map rows refused; unknown TypeNames kept
    verbatim) + `locateWindowSize` (exact case-sensitive match, first wins,
    degenerate -> null).
  - 157/158: `resolveWindowSize` (stored size else 1200x800 fallback, never
    zero-size) + `upsertWindowSize` (in-place same-TypeName replace preserving
    `MainGirdHeight*`/orientation, else append; siblings untouched).
  - Seams: `encodeWindowSizeItems`, `readWindowSizes(document)`,
    `uiGroupWithWindowSize(document, typeName, w, h)` returning the new `UiItem`
    group value for `saveSettingsGroup('UiItem', ...)`.
- `apps/desktop/test/wave_b_window_geometry_test.dart` — 8 tests, synthetic only
  (`SyntheticBridgePort` + `MemoryUiStateStore`, same harness shape as
  `t12a_settings_storage_test.dart`): locate/absent/case-sensitivity/first-wins,
  degenerate-as-absent, unknown-row retention, resolve/fallback, upsert/append/
  sibling-independence, resize->saveGroup->reopen Width+Height restore (1200x800
  -> 1440x900), multi-window independence + unknown-row survival + never-stored
  fallback.

## Per-id status

| id | status | evidence |
|---|---|---|
| FLD-CFG-156 (TypeName locate) | implemented (controller/state level) | `locateWindowSize` + 4 locate tests green; real multi-window open->locate->restore observation NOT run (Wave B GUI follow-on) |
| FLD-CFG-157 (Width restore) | implemented (controller/state level) | two-step resize->save->reopen test green; DPI-clamp + real-window observation NOT run |
| FLD-CFG-158 (Height restore) | implemented (controller/state level) | Height asserted in the same save/reopen loop; real same-TypeName window Height observation NOT run |

None claimed `verified`: no GUI click-through ran in this local-only window.

## Commands + results (targeted only, `apps/desktop`)

- `dart format lib/features/profiles/window_geometry.dart test/wave_b_window_geometry_test.dart` — formatted 2 files (first run); re-check `--output=none --set-exit-if-changed` — 0 changed, FORMAT-CLEAN.
- `flutter analyze --no-pub lib/features/profiles/window_geometry.dart test/wave_b_window_geometry_test.dart` — No issues found.
- `flutter test test/wave_b_window_geometry_test.dart` — 8/8 passed first run (no retry needed). No 10808 touch; no OS side effects (memory store + synthetic bridge only).
- Rust: not run (no Rust files touched). Workspace-wide / release build: not run per scope.

## Gaps registered (not added)

- G-1: no dedicated FRB `get_window_size`/`save_window_size` binding exists; wiring
  uses the `UiItem` group save seam instead. If a per-window binding is later wanted,
  it belongs to the bridge workstream.
- G-2: real `window_manager` consumer missing — nothing applies the resolved size on
  window open or re-stores on close/resize; needs a GUI/armed-hook window (Wave B
  follow-on, cf. acceptance rows 156-158 click-through hooks).
- G-3: DPI-clamp + INI-migration + backup-restore end-to-end observation unrun.
