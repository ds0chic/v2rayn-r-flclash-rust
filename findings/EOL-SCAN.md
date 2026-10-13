# EOL-SCAN — renormalize scan record (fix/EOL, CORRECTED)

Two earlier readings of `outputs/UPSTREAM_INVENTORY.md` were both wrong;
this file records the corrected numbers and why the wrong ones happened.

## Corrected ground truth

Method: `git show HEAD:<path> | tr -cd '\r' | wc -c` and
`tr -cd '\r' < <worktree> | wc -c` (`tr` interprets `\r`; POSIX-sh-safe),
plus `cat -A` (`^M`) on the staged diff and
`git diff --cached --ignore-cr-at-eol` (0 lines = pure EOL change).

| file | lines | CR bytes (blob) | CR bytes (worktree) | verdict |
|---|---|---|---|---|
| outputs/UPSTREAM_INVENTORY.md | 221 | 122 | 122 | MIXED: 122 CRLF + 99 LF, same pattern both sides |
| generated_plugin_registrant.cc | — | 0 | 0 | LF, byte-identical blob/worktree/index (sha f9af37d) |
| generated_plugin_registrant.h | 302 B | 0 | 0 | LF, identical (sha dc139d8) |
| generated_plugins.cmake | 863 B | 0 | 0 | LF, identical (sha e0f8427) |
| sample T06b matrix .log | — | — | CRLF worktree / LF blob | untouched by final rules (zero staged) |

Renormalize deltas observed: WITH `eol=crlf` exception → stages 108/108
(LF→CRLF direction); WITHOUT it (`*.md eol=lf`) → stages 108/108
(CRLF→LF direction). Landed the latter as one-time normalization
(commit ee51a5d); `git diff HEAD --numstat` is empty after landing.

## Retracted readings

1. "221/221 lines CRLF" (early scan): overcount from a CR-insensitive read.
   Refuted by `tr` count: 122, not 221.
2. "CR count 0 in blob and worktree / all LF" (mid-task grep): INVALID
   measurement. `grep -c $'\r'` under POSIX sh never matches CR because sh
   does not expand `$'...'` ANSI-C quoting — the pattern degrades and
   matches (almost) nothing, printing 0. Any adjudication premise built on
   that 0 is void. Use `tr -cd '\r' | wc -c` or PowerShell byte counting.

## Scan hygiene for future EOL work

- Byte counts first (`tr`/`cat -A`), prose readings never.
- `git add --renormalize .` is index-only and safe to trial; `git reset`
  restores the decision point. The standing stop-order (unexpected staged
  file → stop, no commit) fired twice here and each firing produced a
  falsification worth keeping: `-text` rewrites LF-blobs; any text rule
  converts mixed blobs.
