"""Run new audit contracts and individually recheck interrupted batch files."""
import json
import os
from pathlib import Path
import re

from run_checks import APP, HERE, FLUTTER, ROOT, run

records = []
for name in ["native_reply_loss", "group_reopen", "status_viewport", "routing_partial_commit"]:
    path = f"../../docs/evidence/complete-port-audit-2026-10-06/ui/{name}_test.dart"
    record = run("audit-ui-" + name, [str(FLUTTER), "test", "--no-pub", path, "--reporter", "expanded"], APP, 180)
    records.append(record)
for phase in ["seed", "reopen"]:
    os.environ["V2RAYN_R_AUDIT_PHASE"] = phase
    record = run("audit-frb-" + phase, [str(FLUTTER), "test", "--no-pub", "../../docs/evidence/complete-port-audit-2026-10-06/frb_persistence_probe_test.dart", "--reporter", "expanded"], APP, 360)
    records.append(record)
    if record["exit_code"] != 0:
        break
batch = (HERE / "checks/flutter-batch.log").read_text(encoding="utf-8", errors="replace")
files = sorted(set(re.findall(r"(?:loading )?C:/[^\n]*?/apps/desktop/(test/[^\n]*?_test\.dart)(?:: [^\n]*)? \[E\]", batch)))
(HERE / "checks/batch-failure-files.json").write_text(json.dumps(files, indent=2) + "\n", encoding="utf-8")
for path in files:
    name = "batch-recheck-" + re.sub(r"[^a-zA-Z0-9_.-]", "_", path)
    records.append(run(name, [str(FLUTTER), "test", "--no-pub", path, "--reporter", "expanded"], APP, 180))
(HERE / "checks/followup-results.json").write_text(json.dumps(records, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
