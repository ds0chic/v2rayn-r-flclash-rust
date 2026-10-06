# SP-21 integrator addendum: async page FRB entry (2026-10-07)

The Dart-side continuation registered the missing `QueryProfilesPageAsync` FRB
entry. The integrator (main-control, shared FRB/DTO ownership) has now landed
it:

## Changes

- `crates/bridge_api/src/api/contract.rs` — `ProfilePageDto` gained
  `dataset_revision: u64` (the desired revision the page was actually read at)
  and `request_generation: u64` (caller generation echoed back unchanged).
- `crates/bridge_api/src/api/engine.rs` — new async entry
  `query_profiles_page_async(filter, sort, cursor, page_size,
  request_generation) -> ProfilePageDto`, running on the FRB worker pool so a
  slow page on a large store never blocks the UI isolate. The sync
  `query_profiles` and `groups.rs::group_children` share the same DTO and fill
  the meta fields (`request_generation = 0` on non-page entries).
- `apps/desktop/lib/bridge/bridge_port.dart` — seam
  `BridgePort.queryProfilesPageAsync(...)`; `FrbBridgePort` delegates to the
  generated binding, `SyntheticBridgePort` mirrors the contract (real cursor,
  generation echo, `datasetRevision`).
- FRB bindings regenerated; debug `bridge_api.dll` rebuilt.

## Verification

- `cargo test -p bridge_api --locked` — 82/82 (new
  `sp21_page_query_echoes_generation_and_revision`).
- `cargo clippy -p bridge_api --all-targets --locked -- -D warnings` — 0.
- `dart format` 0 changed; `flutter analyze` — no issues.
- `flutter test sp21_paged_load_test + t06a_frb_bridge_test` — 8/8 (new
  "async page seam echoes generation and slices honestly"; real FRB round-trip
  green against the rebuilt DLL).

## Not verified

- Real-GUI 100k timer/interaction sampling stays with SP-31 (release GUI
  measurement); the controller still walks pages through the sync summary
  seam until the next Dart wiring pass consumes the async entry.
