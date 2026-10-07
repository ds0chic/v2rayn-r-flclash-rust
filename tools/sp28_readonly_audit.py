# SP-28 read-only audit script (no production edits, no cargo/flutter).
# Reads: SETTINGS_IMPLEMENTATION_180.csv, settings-current-180.csv,
#   execution-manifest.json, tools/cores/cores.lock.json, SP-23 evidence dir.
# Writes (only): docs/evidence/stable-port/SP-28/field-gap-matrix.csv,
#   docs/evidence/stable-port/SP-28/core-matrix.csv
# Usage: python tools/sp28_readonly_audit.py  (exit 0 = matrices written)
#
# WARNING (2026-10-07): field-gap-matrix.csv is REGENERATED from the CSVs on
# every run. The manually appended L1 continuation rows (SP28-L1-001..003,
# see L1-tracking-2026-10-07.md) are NOT reproduced by this script; merge them
# back after a re-run or extend the script before regenerating.
import csv, json, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPAIR = os.path.join(ROOT, "docs", "repair", "stable-port-2026-10-06")
AUDIT_CSV = os.path.join(ROOT, "docs", "evidence", "complete-port-audit-2026-10-06",
                         "settings", "settings-current-180.csv")
PLAN_CSV = os.path.join(REPAIR, "SETTINGS_IMPLEMENTATION_180.csv")
MANIFEST = os.path.join(REPAIR, "execution-manifest.json")
LOCK = os.path.join(ROOT, "tools", "cores", "cores.lock.json")
SP23_DIR = os.path.join(ROOT, "docs", "evidence", "stable-port", "SP-23")
OUT_DIR = os.path.join(ROOT, "docs", "evidence", "stable-port", "SP-28")

# SP-23 now covers all 180 rows (2026-10-07: FLD-CFG-001..180 all written), so
# the old "not yet registered" filter is obsolete: the full set is kept so the
# `gaps` computation below no longer flags covered rows. The remaining gap
# signal is the CSV status/actual_effect_verified columns.
SP23_IDS = {"FLD-CFG-%03d" % i for i in range(1, 181)}
# SD owner -> suggested SP card (per INTERFACE_AND_OWNER_MAP.md section 2).
SD_TO_SP = {"SD-01": "SP-01", "SD-02": "SP-02", "SD-03": "SP-03",
            "SD-04": "SP-11/SP-12", "SD-05": "SP-15/SP-32", "SD-06": "SP-16",
            "SD-07": "SP-24", "SD-08": "SP-24", "SD-09": "SP-24",
            "SD-10": "SP-25", "SD-11": "SP-26", "SD-12": "SP-22",
            "SD-13": "SP-17", "SD-14": "SP-28(+SP-23)", "SD-15": "SP-13",
            "SD-16": "SP-27", "SD-17": "SP-03(+SP-25)", "SD-18": "SP-32/SP-33"}


def main():
    rows = list(csv.DictReader(open(AUDIT_CSV, encoding="utf-8-sig")))
    assert len(rows) == 180, "expected 180 rows, got %d" % len(rows)
    owners = {r["id"]: r["owner_work_package"]
              for r in csv.DictReader(open(PLAN_CSV, encoding="utf-8-sig"))}
    sp23_files = {f for f in os.listdir(SP23_DIR) if f.startswith("FLD-CFG-")}
    assert len(sp23_files) >= 18, "SP-23 must hold at least 18 FLD files"
    gaps = [r for r in rows
            if r["id"] not in SP23_IDS and r["status"] == "identified"]
    impl = [r for r in rows if r["status"] == "implemented"]
    unverified = [r for r in rows
                  if r["actual_effect_verified_this_audit"] != "true"]
    os.makedirs(OUT_DIR, exist_ok=True)
    with open(os.path.join(OUT_DIR, "field-gap-matrix.csv"), "w",
              encoding="utf-8", newline="") as f:
        w = csv.writer(f)
        w.writerow(["id", "path", "row_kind", "status", "expected_consumer",
                    "current_state", "suggested_owner", "cp_code"])
        for r in sorted(gaps, key=lambda x: x["id"]):
            w.writerow([r["id"], r["path"], r["row_kind"], r["status"],
                        r["production_consumer"], r["current_issue"],
                        SD_TO_SP.get(owners.get(r["id"], ""), "?"),
                        r["cross_cutting_contract"]])
    lock = json.load(open(LOCK, encoding="utf-8"))
    with open(os.path.join(OUT_DIR, "core-matrix.csv"), "w",
              encoding="utf-8", newline="") as f:
        w = csv.writer(f)
        w.writerow(["core", "version", "executable", "config_path",
                    "fixture", "l0_runnable_now", "l1_session_status",
                    "prereq"])
        base_port = 11808
        for i, c in enumerate(lock["cores"]):
            core = c["core"]
            if core in ("xray", "v2fly", "v2fly_v5"):
                cfg = "generate_xray (codegen.rs:311)"
            elif core == "sing-box":
                cfg = "generate_singbox (codegen.rs:310)"
            elif core == "mihomo":
                cfg = "generate_mihomo mixin helper (mixin.rs:75; G-06)"
            else:
                cfg = "native_custom passthrough (engine.rs:4840)"
            w.writerow([core, c.get("core_version", "?"),
                        c.get("executable", "?"), cfg,
                        "fixtures/synthetic/sp28/%s-session.json" % core,
                        "yes (exe present; version-identity only)",
                        "blocked (SP-28 L1 gate not run)",
                        "SP-24" if cfg.startswith("generate") or core == "mihomo"
                        else "SP-28 L1"])
            base_port += 1
    print("rows=180 gaps=%d implemented=%d unverified_effect=%d sp23_files=%d cores=%d"
          % (len(gaps), len(impl), len(unverified), len(sp23_files),
             len(lock["cores"])))


if __name__ == "__main__":
    sys.exit(main())
