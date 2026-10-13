# FIXREPORT-EOL — line-ending unification (fix/EOL)

Base: b2bcb7b. Standing stop-order: if `git add --renormalize .` stages any
unexpected file, stop and report — no commit. It fired twice; both findings
below were adjudicated before landing.

## Landed commits

1. `641e378` — `chore(eol): add .gitattributes (text=auto, LF for source/docs, binaries untouched)`
2. `ee51a5d` — `chore(eol): normalize outputs/UPSTREAM_INVENTORY.md mixed endings to LF (content-identical)`
   Evidence in message: `git diff --ignore-cr-at-eol` empty (0 output lines);
   122 CRLF of 221 lines → LF; numstat 108/108. Adjudicated option (a):
   one-time normalization, file not under compat/ ledger.
3. (this report + EOL-SCAN.md, single docs commit; trailers as below)

Trailers on all EOL commits:

```text
Co-Authored-By: Claude Code <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01LVqrcJqocWMXVaSeYcm3q7
```

## Final .gitattributes (committed, 53 lines)

`* text=auto`; LF enforced for dart/rs/cpp/h/cc/cmake/md/txt/json/yaml/yml/
toml/ps1/iss/sh/bat/cmd/gradle/podspec/rc; binary for png/ico/icns/dll/exe/
so/dat/db/db-shm/db-wal/zip/cer. Deliberately ABSENT (both adjudicated out):

- No `*.log` rule (option c). `*.log -text` was tried and FALSIFIED:
  renormalize staged dozens of `docs/evidence/T06b.runs/matrix-logs/*.log`
  (worktree CRLF bytes over LF-normalized blobs left from the
  core.autocrlf=true era; confirmed via numstat + `^M` diff lines).
  -text rewrites evidence blobs instead of preserving them. Final
  renormalize stages zero .log files; check-attr on a sample log:
  `text: auto, eol: unspecified`.
- No `outputs/UPSTREAM_INVENTORY.md text eol=crlf` exception (options 2, a).
  FALSIFIED twice over: the "keep generated CRLF byte-identical" premise
  was wrong (file is MIXED endings, see EOL-SCAN.md), and any text rule
  converts a mixed blob — eol=crlf staged 108/108 (LF→CRLF), while
  `*.md eol=lf` stages 108/108 (CRLF→LF). Accepted the latter: one-time,
  content-identical, matches "text stored LF" goal.

## Verification evidence

- Final `git add --renormalize .`: staged only UPSTREAM_INVENTORY.md
  (+ untracked .gitattributes); zero .log, zero other files.
- check-attr: inventory `text+eol=lf`; `main.cpp`/`build_windows.ps1`/`generated_plugin_registrant.cc` `eol=lf`; sample .log `text:auto`.
- `flutter pub get` in `apps/desktop` (no flutter.lock contention; 10 pins
  reported outdated-but-constrained, lockfile untouched): the three
  `windows/flutter/generated_plugin*` files are BYTE-IDENTICAL to HEAD
  blobs (sha1 equal for worktree/index/HEAD, sizes 1040/302/863, 0 CR
  bytes). Transient phantom-M after pub get was stat-only (index still
  recorded the 1066-byte autocrlf-checkout size vs regenerated 1040-byte
  LF); resolved by content-verified stat refresh, no blob change.
- Post-landing: `git status --short` empty; `git diff HEAD --numstat` empty.
- No `build/` dirs created (pub get only) — nothing to delete.
- Env: core.autocrlf=true, core.eol unset, git 2.54.0.windows.1.

## Measurement lesson (caused two wrong adjudication premises)

- `grep -c $'\r'` under POSIX sh/GIt Bash counts NOTHING useful: sh does
  not expand `$'...'` (pattern degrades to a literal), so it reported
  "CR count 0" → false "all LF" premise.
- The earlier "221/221 CRLF" scan was likewise wrong (CR-insensitive read).
- Ground truth (`tr -cd '\r' | wc -c`, POSIX-safe): 122 CR bytes / 221
  lines, identical in HEAD blob and worktree → mixed 122 CRLF + 99 LF.
- Rule: count CR bytes with `tr -cd '\r' | wc -c` or PowerShell byte
  counting; never `$'\r'` in sh, never eyeballing Read output.
