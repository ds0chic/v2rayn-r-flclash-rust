# SP-03 Wave B / Tier 4 — synthetic-local WebDAV E2E (FLD-CFG-143/144/145/146)

Local-only, loopback only. No real remote WebDAV, no real credentials, no commit.

## 0. Owner ruling (record only, manifest untouched)

Per `docs/evidence/stable-port/SP-28/open-gap-propagation-2026-10-07.md`
(row `SD-17 t16`: "143-146 WebDAV real-protocol acceptance" -> **SP-03**,
basis status-axes s3.5 / `INTERFACE_AND_OWNER_MAP.md:47`), the SD-17/t16
backup-link consumer gap for FLD-CFG-143/144/145/146 is owned **primary by
SP-03**. SP-23 remains the field registry owner; the DTO/fake registration
stays as documented in `SP-23/FLD-CFG-143.md`..`146.md`. This file records the
ruling only — ledger CSV, FLD docs and manifest were not edited.

## 1. Files changed (this workstream only)

- `crates/application/tests/wave_b_webdav.rs` (new, 6 tests): synthetic-local
  WebDAV harness + E2E + failure matrix. Tests only.
- `crates/application/src/webdav.rs`: **not modified by this workstream**.
  A parallel workstream refactored it mid-task (reqwest client ->
  `platform::http::SharedHttpClient`, public API unchanged); the harness
  below passed against both versions (see §3).
- No edits to bridge_api, ipc_contract, services, workspace Cargo, FRB
  files, apps/desktop, ledger CSV, FLD docs, manifest, or other
  workstreams' files.

## 2. Harness design

- In-test WebDAV server (tiny_http) bound to the first free 127.0.0.1 port in
  `11808..11900`, asserted `>= 11808` and `!= 10808` at bind time. Speaks
  PROPFIND (207 multistatus reflecting live stored length) / MKCOL / PUT /
  GET / DELETE, all gated by synthetic Basic auth (`waveb-synthetic-user` /
  `waveb-synthetic-pass`, exact pair compared after base64 decode).
- Fault injection per test: `fail_next_put` / `fail_next_get` (status without
  replacing stored bytes), `truncate_get_to` (short 200 body), `delay`
  (timeout/interrupt). PUT failure never touches `stored`: old remote backup
  intact, no partial replace.
- Flow per FLD docs: backup-settings save (a `WebDavConfig` built from the
  exact saved WebDavItem fields Url/UserName/Password/DirName, with
  `backup_url()` + empty-dir `DEFAULT_DIR` fallback asserted) -> `check()` ->
  `zip_upstream_layout` bundle upload -> `list()` -> `download()` ->
  sha256 equality gate -> `recognize()` upstream -> `import_upstream()` into
  a fresh install -> `AppEngine::open` -> drop -> second `open` (reopen).
  Import is only ever called on hash-verified bytes, so every failure path
  leaves local state consistent by construction; restore itself is
  transactional (covered by `t16_backup.rs`).
- The formal settings-window save (DTO/wire) lives in bridge_api/Dart and is
  out of scope; the config-mapping step above is its application-side image.

## 3. Commands + results

HEAD `adad4ca` at run time. Co-edited tree: one transient breakage observed
(parallel `platform::http` work, E0631 + stale `Cargo.lock` vs `--locked`);
this workstream reverted its own incidental `Cargo.lock` touch, waited for
the tree to settle, and re-ran. Final results below are all post-settle.

| command | exit | result |
|---|---|---|
| `cargo fmt -p application -- --check` | 0 | clean, whole package |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | 0 | no warnings |
| `cargo test -p application --locked --test wave_b_webdav` | 0 | **6/6** pass |
| `cargo test -p application --locked --test t16_webdav` | 0 | **7/7** pass (existing suite, no regression) |

New-target cases: `waveb_e2e_save_upload_list_download_restore_reopen`,
`waveb_auth_failure_leaves_local_and_remote_state`,
`waveb_put_failure_keeps_old_remote_then_retry_succeeds`,
`waveb_truncated_download_rejected_by_hash_without_restore`,
`waveb_interrupted_download_timeout_keeps_state_retryable`,
`waveb_delete_removes_remote_backup`. The 6/6 was additionally observed
pre-refactor (original reqwest client) during the task; the recorded final
run is post-refactor (`SharedHttpClient`).

## 4. Per-id status

| id | field | status here |
|---|---|---|
| FLD-CFG-143 | WebDavItem.Url | synthetic-local E2E green: saved address drives real PROPFIND/PUT/GET roundtrip, hash-identical bundle, import + double reopen serves the restored active profile. |
| FLD-CFG-144 | WebDavItem.UserName | green: wrong username -> `E_PERMISSION_DENIED` on check/list/upload/download, no leak, remote old backup and local install intact. |
| FLD-CFG-145 | WebDavItem.Password | green: wrong password same as 144; error details asserted free of username/password; mid-transfer 500 is `E_UNAVAILABLE` + retryable and retry succeeds. |
| FLD-CFG-146 | WebDavItem.DirName | green: configured dir encodes `/{dir}/backup.zip`; empty dir falls back to `v2rayN_backup`; truncated-200 caught by hash gate, never restored; DELETE removes the entry (list/GET confirm absence). |

No production defect was exposed by the matrix (401 classification,
retryable 5xx, timeout, 404, list parsing all correct), so per the task
brief the change is tests-only. `verified` against a real remote is
explicitly **not** claimed (see §5).

## 5. Gaps (remain open per policy)

- Real remote WebDAV/TLS interop unverified: no real server, no TLS peer,
  no proxy-path run in this task (loopback plain HTTP only, direct
  connection). `HttpsTrust`/cert-provider paths are exercised only to the
  extent the client builds with system trust.
- Partial-PUT-against-a-real-server behavior is a server-side property; the
  synthetic server replaces bytes only after full receipt. Atomic
  PUT-tmp+MOVE semantics, if ever required, belong to a future card.
- Formal backup-settings window -> save -> reopen (bridge_api/Dart) and the
  t16 FRB backup/restore entry points remain Wave B GUI follow-on outside
  `crates/application`.
- `cargo test -p application --locked --test t16_backup` not re-run here
  (out of the tasked check list; engine co-edited by parallel work).
