"""Validate assignments and consolidate audit records, not product completion."""

import csv
import hashlib
import json
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "docs/evidence/parity-review-2026-10-03"
OWNERS = ("profiles", "settings", "runtime", "root")
STATES = {"identified", "implemented", "verified", "preserved_only", "blocked", "not_applicable"}
REQUIRED = {"key", "id", "kind", "status", "upstream_expected", "current_behavior", "difference", "upstream_refs", "current_refs", "evidence_level", "tests_run", "limitations", "next_action"}


def read(name):
    return json.loads((DEST / name).read_text(encoding="utf8"))


def build():
    inventory = read("inventory.json")
    expected = {x["key"] for x in inventory["items"]}
    rows = []
    coverage = {}
    missing_refs = []
    for owner in OWNERS:
        assigned = read(f"{owner}-assigned.json")
        items = read(f"{owner}-items.json")
        keys = [x["key"] for x in items]
        assert len(keys) == len(set(keys)), (owner, "duplicate")
        assert set(keys) == {x["key"] for x in assigned}, (owner, "assignment mismatch")
        for item in items:
            assert REQUIRED <= set(item), (item["key"], REQUIRED - set(item))
            assert item["status"] in STATES, (item["key"], item["status"])
            assert item["upstream_expected"] and item["current_behavior"], item["key"]
            item["owner"] = owner
            for group in ("upstream_refs", "current_refs"):
                for ref in item[group]:
                    if not isinstance(ref, str) or "/" not in ref or ref.startswith(("http", "symbol:")):
                        continue
                    path = re.split(r":\d|::", ref, maxsplit=1)[0]
                    if not (ROOT / path).exists():
                        missing_refs.append({"key": item["key"], "group": group, "ref": ref})
        coverage[owner] = {"assigned": len(assigned), "recorded": len(items), "missing": 0, "duplicates": 0}
        rows.extend(items)
    assert len(rows) == len({x["key"] for x in rows})
    assert {x["key"] for x in rows} == expected
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, capture_output=True, check=True).stdout.strip()
    summary = dict(inventory["summary"], audit_status="verified", product_status="identified",
                   scope="All inventoried rows source-reviewed; six current Windows flows and two original-wire codec assertions executed. Not all functional paths tested.",
                   end_commit=head, baseline_unchanged=head == inventory["summary"]["application_commit"],
                   coverage=coverage, raw_record_statuses=dict(Counter(x["status"] for x in rows)),
                   evidence_levels=dict(Counter(x["evidence_level"] for x in rows)),
                   reference_warnings=missing_refs,
                   limitations=["800 rows include fields, enums, windows, layouts and functions; no completion percentage is computed.",
                                "Original expected behavior comes from frozen source, not a live original GUI run.",
                                "Current six Windows scenarios use actual Flutter/FRB/Rust/SQLite and synthetic isolated data; no kernel or host network-setting effects.",
                                "No macOS/Linux/ARM64 execution; no release package produced in this audit."])
    (DEST / "all-items.json").write_text(json.dumps({"summary": summary, "items": rows}, ensure_ascii=False, indent=2), encoding="utf8")
    (DEST / "coverage.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf8")
    columns = ["key", "id", "kind", "owner", "label", "status", "priority", "evidence_level", "finding_ids", "upstream_expected", "current_behavior", "difference", "next_action", "upstream_refs", "current_refs", "tests_run", "limitations"]
    with (DEST / "all-items.csv").open("w", encoding="utf-8-sig", newline="") as out:
        writer = csv.DictWriter(out, fieldnames=columns, extrasaction="ignore")
        writer.writeheader()
        for row in rows:
            writer.writerow({key: json.dumps(row.get(key), ensure_ascii=False) if isinstance(row.get(key), (list, dict)) else row.get(key, "") for key in columns})
    hashes = {p.relative_to(DEST).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in DEST.rglob("*") if p.is_file() and p.name != "evidence-sha256.json"}
    (DEST / "evidence-sha256.json").write_text(json.dumps(hashes, ensure_ascii=False, indent=2), encoding="utf8")
    print(json.dumps({"rows": len(rows), "coverage": coverage, "baseline_unchanged": summary["baseline_unchanged"],
                      "raw_record_statuses": summary["raw_record_statuses"], "reference_warning_count": len(missing_refs)}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    build()
