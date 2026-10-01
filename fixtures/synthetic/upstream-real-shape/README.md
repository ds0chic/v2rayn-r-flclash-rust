# `upstream-real-shape` synthetic fixtures

These fixtures are **derived, sanitized copies** of a real v2rayN profile
directory. They exist to regression-test the T04 importer/migration against the
*data shapes* produced by real upstream builds (column order, extra/missing
columns, version distributions, non-ASCII and emoji text).

## Provenance

- Producer: `tools/sanitize_upstream.py` (read-only over the real source, one-way
  deterministic replacement, then a fresh rebuild + `VACUUM`).
- The real source is **never** copied here byte-for-byte: each database is
  recreated from its `sqlite_master` schema and every `TEXT` value is replaced by
  a content-addressed synthetic value.
- Embedded-JSON `TEXT` columns (`ProfileItem.ProtoExtra`,
  `ProfileItem.TransportExtra`, `ProfileItem.Extra`, `RoutingItem.RuleSet`) keep
  their object/array structure, key names and numeric/bool/null leaves; only
  their string leaves are replaced. This is required because the importer parses
  those columns and upstream always writes valid JSON there.
- The generator proves that **no original user-bearing text value survives** as a
  substring of the sanitized corpus (see the `scan: 0 hits` output when running
  the script). "User-bearing" means plain `TEXT` cells plus the string leaves of
  embedded-JSON columns; JSON keys and numeric/bool/null leaves are schema and
  are intentionally preserved.

## Files

| file | derived from | shape |
|---|---|---|
| `guiNConfig.json` | current `guiNConfig.json` | 25 root keys, `UiItem` with 14 windows / 14 columns |
| `guiNDB.db` | current `guiNDB.db` | `ConfigVersion` 4; `ProfileItem` has 41 columns incl. `EchForceQuery` |
| `upstream-old-v1.db` | `upstream-old-20260602.db` | `ConfigVersion` 2, 35-column `ProfileItem`, no `ProtoExtra`/`TransportExtra` |
| `upstream-old-v2.db` | `upstream-old-1780395234.db` | `ConfigVersion` 2, 35-column `ProfileItem` |
| `upstream-bak.db` | `upstream-bak-1788539222.db` | `ConfigVersion` 4, `ConfigType` includes `11` (Anytls) and `3` (Shadowsocks) |

Row counts, `ConfigVersion`/`ConfigType` distributions, declared sqlite-net
column types (`varchar`/`float`/`INTEGER`) and per-`Remarks` non-ASCII/emoji
counts are preserved exactly.

## Rules

- These files contain **no real node addresses, ports, credentials, subscription
  URLs or user remarks**. All strings are synthetic.
- Do **not** attempt to reverse the substitution; the mapping is a one-way hash
  and the original values are not stored anywhere in this repository.
- Regenerate only through `tools/sanitize_upstream.py`; do not hand-edit.
