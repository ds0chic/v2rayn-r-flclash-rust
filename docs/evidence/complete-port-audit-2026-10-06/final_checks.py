"""Adaptive rechecks. Keep failures; never change expectations for the product."""
import json
import os
from run_checks import APP, HERE, FLUTTER, run

records = []
for name in ["native_reply_loss", "group_reopen", "ungrouped_import_failure"]:
    records.append(run("audit-ui-" + name + "-confirmed", [str(FLUTTER), "test", "--no-pub", f"../../docs/evidence/complete-port-audit-2026-10-06/ui/{name}_test.dart", "--reporter", "expanded"], APP, 180))
records.append(run("audit-fixed-six", [str(FLUTTER), "test", "--no-pub", "test/repair/audit_tun_settings_contract_test.dart", "--reporter", "expanded"], APP, 180))
previous = json.loads((HERE / "checks/followup-results.json").read_text(encoding="utf-8"))
for record in previous:
    if record["name"].startswith("batch-recheck-") and record["exit_code"] != 0:
        records.append(run(record["name"] + "-retry", record["command"], APP, 180))
for phase in ["seed", "reopen"]:
    os.environ["V2RAYN_R_AUDIT_PHASE"] = phase
    os.environ["V2RAYN_R_AUDIT_ROWS"] = "100000"
    record = run("audit-frb-100000-" + phase, [str(FLUTTER), "test", "--no-pub", "../../docs/evidence/complete-port-audit-2026-10-06/frb_persistence_probe_test.dart", "--reporter", "expanded"], APP, 420)
    records.append(record)
    if record["exit_code"] != 0:
        break
records.append(run("flutter-release", [str(FLUTTER), "build", "windows", "--release", "--no-pub"], APP, 1800))
(HERE / "checks/final-results.json").write_text(json.dumps(records, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
