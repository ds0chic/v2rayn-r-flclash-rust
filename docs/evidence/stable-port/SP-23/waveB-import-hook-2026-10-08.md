# SP-23 waveB import hook (FLD-CFG-001/002) — 2026-10-08

ARMED-ONLY startup evidence hook for the FLD-CFG-001/002 import flow.
Local-only, synthetic-only; the official unarmed package is unaffected
(the hook compiles to a no-op without the armed dart-define).

## Hook design

- New file `apps/desktop/lib/perf/import_synth_hook.dart` (`ImportSynthHook`).
- Wired in `apps/desktop/lib/main.dart` right after `RustBridgeInit.init()`,
  before `runApp`: when enabled, run the import, write the result, `exit()`.
- Arming (same pattern as `V2RAYN_R_AUTO_SMOKE` / `T18Bench.enabled`):
  compile-time `V2RAYN_R_SMOKE_ARMED=true` **and** runtime
  `V2RAYN_R_IMPORT_SYNTHETIC=<absolute fixture path>`. Either absent =>
  normal launch, env ignored entirely.
- Real import paths reused (no product refactor, no new bridge surface):
  - `.zip` input: `FrbBridgePort.t16BackupImportUpstream(path)` — the exact
    call `BackupController.importUpstream` makes for an upstream gui-configs
    ZIP (IndexId remap happens in Rust, as in production).
  - text input: `previewImport(bridge, text)` + `commitImport(bridge, preview)`
    from `features/subs/import_persistence.dart` — the SP-14 seam
    `subs_actions._importPipeline` (clipboard/paste import) uses.
- Post-import read-back through the real store (FLD-CFG-001/002 observables):
  `queryAllProfiles()` (indexIds), `getActiveProfile()` (activeId),
  `listSubItems()` (groupIds).
- Output: `<V2RAYN_R_IMPORT_OUT>/import-result.json`:
  `ok`, `imported`, `sourceKind`, `indexIds`, `activeId`, `groupIds`
  (failure: `ok:false` + `error`, exit 1). Exit 0 on success.
- Guards: fixture path must be absolute and exist; empty text fails closed;
  preview-with-zero-nodes and commit-saving-zero both fail closed. No network,
  no subscription URLs, no credentials; fixture text is never logged nor
  written — only counts/ids go to stderr and the JSON.
- Untouched (per constraints): `bridge_api`, `ipc_contract`, `services`,
  workspace Cargo, FRB files, other features, ledger CSV, FLD docs, manifest.
  No commit.

## How to run (integrator, after the armed rebuild)

Build once with the armed flag (integrator-owned; NOT run here):

```powershell
flutter build windows --release --dart-define=V2RAYN_R_SMOKE_ARMED=true
```

Text fixture (synthetic share links, e.g. `fixtures/acceptance/sp30/synthetic-sub.txt`):

```powershell
$env:V2RAYN_R_IMPORT_SYNTHETIC = 'C:\...\fixtures\acceptance\sp30\synthetic-sub.txt'
$env:V2RAYN_R_IMPORT_OUT = 'C:\...\docs\evidence\stable-port\SP-23\import-out'
.\build\windows\x64\runner\Release\v2rayn_desktop.exe
# expect exit 0 + $env:V2RAYN_R_IMPORT_OUT\import-result.json
```

Upstream-layout ZIP fixture (synthetic gui-configs ZIP):

```powershell
$env:V2RAYN_R_IMPORT_SYNTHETIC = 'C:\...\synthetic-gui-configs.zip'
$env:V2RAYN_R_IMPORT_OUT = 'C:\...\docs\evidence\stable-port\SP-23\import-out'
.\build\windows\x64\runner\Release\v2rayn_desktop.exe
# expect exit 0 + import-result.json (indexIds = remapped IndexIds,
# activeId/groupIds = post-import observables for FLD-CFG-001/002)
```

Unarmed binary with the same env set must launch the normal UI and produce
no `import-result.json` (official package unaffected).

## Checks run (this change)

- `dart format --output=none --set-exit-if-changed lib/perf/import_synth_hook.dart lib/main.dart`
  => 0 changed after (one auto-format applied to `import_synth_hook.dart`, then clean).
- `flutter analyze` => No issues found.
- `flutter test test/repair/sp_16_sub_entry_test.dart test/repair/sp_14_import_batch_test.dart`
  => All tests passed (18/18: 9 SP-16/SP-18 + 9 SP-14).
  (`test/repair/table_actions_test.dart` does not exist in this tree; SP-14
  import-batch tests cover the reused preview/commit seam and SP-16 entry
  tests cover the group surface.)
- NOT run per constraints: cargo, release/armed build (integrator builds),
  whole-suite gate.
