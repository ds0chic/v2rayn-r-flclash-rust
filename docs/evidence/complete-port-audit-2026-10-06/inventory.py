"""Freeze ledger and shipped package identities without reading user data."""
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

import yaml

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as src:
        for block in iter(lambda: src.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


report = {"recorded_at": datetime.now(timezone.utc).isoformat(),
          "head": git("rev-parse", "HEAD"),
          "audit_start_source_clean": True,
          "source_diff_now": git("diff", "--stat", "--", "apps", "crates", "services", "Cargo.lock", "compat"),
          "scope_note": "Ledger entries are heterogeneous inventory, not a completion denominator.",
          "ledgers": {}, "official_zips": []}
for name, key in [("features", "features"), ("actions", "items"),
                  ("fields.settings", "items"), ("fields.entities", "items"),
                  ("layouts", "items")]:
    doc = yaml.safe_load((ROOT / "compat" / (name + ".yaml")).read_text(encoding="utf-8"))
    rows = doc.get(key, [])
    if not rows and name.startswith("fields"):
        rows = doc.get("fields", [])
    report["ledgers"][name] = {"rows": len(rows),
        "status_counts": dict(Counter(row.get("status", "missing") for row in rows)),
        "domain_counts": dict(Counter(row.get("domain", row.get("family", "unspecified")) for row in rows))}
    if name == "layouts":
        report["ledgers"][name]["main_layouts"] = len(doc.get("main_layouts", []))
        report["ledgers"][name]["window_inventory"] = len(doc.get("window_inventory", []))
locks = json.loads((ROOT / "tools/cores/cores.lock.json").read_text(encoding="utf-8"))
report["core_locks"] = [{key: row.get(key) for key in
    ("core", "core_version", "sha256_verified", "platform")} for row in locks["cores"]]
for file in sorted((ROOT / "dist").glob("v2rayN-R-*-windows-x64.zip")):
    item = {"path": str(file.relative_to(ROOT)), "sha256": sha(file), "size": file.stat().st_size}
    with zipfile.ZipFile(file) as z:
        metadata = [n for n in z.namelist() if n.endswith("/build-info.json") or n == "build-info.json"]
        item["build_info"] = [json.loads(z.read(n).decode("utf-8-sig")) for n in metadata]
        item["runtime_members"] = [n for n in z.namelist() if n.endswith(("v2rayn_desktop.exe", "net_host.exe", "privileged_helper.exe", "v2rayN-upgrade.exe", "bridge_api.dll"))]
    report["official_zips"].append(item)
report["sha256sums"] = (ROOT / "dist/SHA256SUMS").read_text(encoding="utf-8-sig")
report["flutter_test_files"] = len(list((ROOT / "apps/desktop/test").rglob("*_test.dart")))
(HERE / "baseline.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
