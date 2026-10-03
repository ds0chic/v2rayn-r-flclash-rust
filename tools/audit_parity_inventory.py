"""Build a read-only audit inventory from the frozen compatibility ledgers."""

import json
from collections import Counter
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "docs/evidence/parity-review-2026-10-03"
FEATURE_OWNERS = {
    **dict.fromkeys(("profile", "import-export", "test"), "profiles"),
    **dict.fromkeys(("subscription", "routing", "dns", "backup", "help"), "settings"),
    **dict.fromkeys(
        ("core", "system-proxy", "tun", "monitor", "desktop", "app-mgmt"), "runtime"
    ),
}
ENTITY_OWNERS = {
    **dict.fromkeys(
        (
            "ProfileItem", "ProtocolExtraItem", "TransportExtraItem", "ProfileGroupItem",
            "ProfileExItem", "ServerStatItem", "FullConfigTemplateItem",
        ),
        "profiles",
    ),
    **dict.fromkeys(("SubItem", "DNSItem", "RoutingItem", "RulesItem"), "settings"),
}
ACTION_OWNERS = {
    **dict.fromkeys(("profiles", "testing"), "profiles"),
    **dict.fromkeys(
        ("subscriptions", "settings", "routing", "dns", "hotkey", "backup"), "settings"
    ),
    **dict.fromkeys(("tray", "statusbar"), "runtime"),
    "main-menu": "root",
}
WINDOW_OWNERS = {
    **dict.fromkeys(
        (
            "ProfilesView", "AddServerWindow", "AddServer2Window", "AddGroupServerWindow",
            "ProfilesSelectWindow", "FullConfigTemplateWindow", "QrcodeView", "JsonEditor",
        ),
        "profiles",
    ),
    **dict.fromkeys(
        (
            "ThemeSettingView", "OptionSettingWindow", "DNSSettingWindow", "RoutingSettingWindow",
            "RoutingRuleSettingWindow", "RoutingRuleDetailsWindow", "SubSettingWindow",
            "SubEditWindow", "GlobalHotkeySettingWindow", "BackupAndRestoreView",
        ),
        "settings",
    ),
    **dict.fromkeys(
        (
            "ClashProxiesView", "ClashConnectionsView", "MsgView", "StatusBarView",
            "CheckUpdateView", "SudoPasswordInputView",
        ),
        "runtime",
    ),
}


def read(name):
    return yaml.safe_load((ROOT / "compat" / name).read_text(encoding="utf-8"))


def build():
    rows = []

    def add(kind, values, owner, source):
        for index, value in enumerate(values, 1):
            ident = value.get("id") or f"{kind.upper()}-SOURCE-{index:03d}"
            rows.append({
                "key": f"{kind}:{ident}",
                "id": ident,
                "kind": kind,
                "owner": owner(value),
                "ledger": f"compat/{source}",
                "label": value.get("name") or value.get("entity") or value.get("core_type"),
                "ledger_status": value.get("status"),
                "source_contract": value,
            })

    features = read("features.yaml")
    add("feature", features["features"], lambda x: FEATURE_OWNERS[x["family"]], "features.yaml")
    add("setting", read("fields.settings.yaml")["items"], lambda _: "settings", "fields.settings.yaml")
    add("entity", read("fields.entities.yaml")["items"], lambda x: ENTITY_OWNERS[x["entity"]], "fields.entities.yaml")
    add("action", read("actions.yaml")["items"], lambda x: ACTION_OWNERS[x["domain"]], "actions.yaml")
    layouts = read("layouts.yaml")
    add("layout", layouts["items"], lambda x: WINDOW_OWNERS.get(x["window"], "root"), "layouts.yaml")
    add("main_layout", layouts["main_layouts"], lambda _: "root", "layouts.yaml")
    add("window", layouts["window_inventory"], lambda x: WINDOW_OWNERS.get(x["name"], "root"), "layouts.yaml")
    add("config_type", features["config_types"], lambda _: "profiles", "features.yaml")
    add("format", features["fmt_formats"], lambda _: "profiles", "features.yaml")
    add("core_matrix", features["core_capability_matrix"], lambda _: "runtime", "features.yaml")
    add("background", features["scheduled_and_background"], lambda _: "runtime", "features.yaml")
    add("scheduled", read("actions.yaml")["scheduled_tasks"], lambda _: "runtime", "actions.yaml")
    add("enum", features["enum_scan"], lambda _: "root", "features.yaml")
    assert len({row["key"] for row in rows}) == len(rows)
    DEST.mkdir(parents=True, exist_ok=True)
    summary = {
        "application_commit": "1251cbc6821276e35d082f7739b8b9c15b6f93dd",
        "upstream_commit": features["source_commit"],
        "purpose": "Assignment and source inventory; inclusion does not mean functional verification.",
        "total_rows": len(rows),
        "by_kind": dict(Counter(row["kind"] for row in rows)),
        "by_owner": dict(Counter(row["owner"] for row in rows)),
    }
    (DEST / "inventory.json").write_text(json.dumps({"summary": summary, "items": rows}, ensure_ascii=False, indent=2), encoding="utf-8")
    for owner in ("profiles", "settings", "runtime", "root"):
        subset = [row for row in rows if row["owner"] == owner]
        (DEST / f"{owner}-assigned.json").write_text(json.dumps(subset, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    build()
