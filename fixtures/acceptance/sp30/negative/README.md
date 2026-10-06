# fixtures/acceptance/sp30/negative — package-identity negative fixtures

Synthetic `build-info.json` variants consumed by
`tools/acceptance/sp30_package_identity_selftest.ps1`, which builds small
throwaway zips in `%TEMP%` (four flat-layout exe stubs + one variant) and
asserts the identity script rejects each negative case. Nothing here is a
real package; the `git_commit` values are fake (`aaaa…` = expected baseline
for the selftest, `bbbb…` = drifted commit). No secrets, no network.

| file | case | identity script must report |
|---|---|---|
| `build-info.good.json` | control: clean + unarmed + expected commit | exit 0 (`pass`) |
| `build-info.dirty.json` | dirty tree (`git_dirty=true`) | `fail`: `git_dirty != false` |
| `build-info.drift.json` | commit drift (`bbbb…` vs expected `aaaa…`) | `fail`: `git_commit … != expected …` |
| `build-info.armed.json` | smoke-armed package (`smoke_armed=true`) | `fail`: `smoke_armed != false` |
| `build-info.nofield.json` | missing metadata (no `git_commit`) | `fail`: `build-info missing field: git_commit` |

Further negative dimensions need no extra fixture files — the selftest
derives them at runtime from the control zip:

- `nometa`: zip with no `build-info.json` at all → `build-info.json not in zip`.
- `hashmismatch`: SHA256SUMS entry tampered → `zip sha256 mismatch`.
- `missingentry`: SHA256SUMS without this zip's basename → `sha256sums entry missing`.
- `noshafile`: `-ShaFile` pointing at a nonexistent path → `sha256sums file not found`.
- `setup-mismatch`: stub `setup.exe` whose SHA256SUMS entry is tampered → `setup sha256 mismatch`.
- `cleantree-dirty/clean`: temp `git init` repo with/without an untracked file
  exercises `-RequireCleanTree` live rejection vs pass without touching
  the real working tree.
