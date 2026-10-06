"""Validate plan inventory, dependencies and document links; never run the app."""
import csv
import json
import re
from collections import Counter
from pathlib import Path

import yaml


def main():
    plan = Path(__file__).resolve().parent
    repo = plan.parents[2]
    manifest = json.loads((plan / "execution-manifest.json").read_text(encoding="utf-8"))
    upstream = "7d6a967c18c697f28dc6917122ed3a4993fcf336"
    tasks = manifest["tasks"]
    indexed = {task["id"]: task for task in tasks}
    assert len(tasks) == len(indexed) == 36
    assert set(indexed) == {f"SP-{i:02d}" for i in range(36)}
    assert manifest["upstream_commit"] == upstream
    assert manifest["production_edits"] is False
    assert all(task["status"] == "identified" for task in tasks)
    visiting, visited = set(), set()
    topological = []

    def visit(task_id):
        assert task_id in indexed, f"Unknown dependency: {task_id}"
        assert task_id not in visiting, f"Dependency cycle: {task_id}"
        if task_id in visited:
            return
        visiting.add(task_id)
        for dependency in indexed[task_id]["dependencies"]:
            visit(dependency)
        visiting.remove(task_id)
        visited.add(task_id)
        topological.append(task_id)

    for task in tasks:
        visit(task["id"])

    expected_findings = {f"CP-{i:02d}" for i in range(1, 18)}
    assert set(manifest["findings_to_tasks"]) == expected_findings
    for finding, owners in manifest["findings_to_tasks"].items():
        assert owners and all(finding in indexed[owner]["findings"] for owner in owners)
    for task in tasks:
        assert all(task["id"] in manifest["findings_to_tasks"][f] for f in task["findings"])

    markers = [
        "任务 ID：", "本次唯一用户流程：", "前置任务及已验证证据：",
        "对应 feature / field / action / layout ID：", "必读上游文件、符号和固定 commit：",
        "输入、输出、错误、取消、权限、持久化及生效语义：", "允许修改的模块：",
        "禁止改变的已有行为：", "测试夹具和原版预期：",
        "本次必须通过的命令/真实场景：", "证据文件位置：", "完成条件：",
        "发现接口缺口时的处理：",
    ]
    for task in tasks:
        card = plan / "tasks" / f"{task['id']}.md"
        content = card.read_text(encoding="utf-8")
        assert all(marker in content for marker in markers), str(card)
        assert "状态：identified" in content
        assert "CommitUnknown/RecoveryRequired" in content and "datasetEpoch" in content
        for legacy in task["legacy_tasks"]:
            assert (repo / "docs" / "repair" / "tasks" / f"{legacy}.md").exists(), legacy

    ledger = yaml.safe_load((repo / "compat" / "fields.settings.yaml").read_text(encoding="utf-8"))
    field_ids = {item["id"] for item in ledger["items"]}
    with (plan / "SETTINGS_IMPLEMENTATION_180.csv").open(encoding="utf-8-sig", newline="") as handle:
        fields = list(csv.DictReader(handle))
    assert len(fields) == 180 and len({row["id"] for row in fields}) == 180
    assert {row["id"] for row in fields} == field_ids
    kinds = Counter(row["row_kind"] for row in fields)
    assert kinds == {"container": 23, "internal": 2, "leaf": 155}, kinds
    required_columns = [
        "id", "canonical_path", "owner_work_package", "final_consumer", "apply_timing",
        "required_verification", "dependencies", "current_gap", "research_status",
        "platform_scope", "source_file", "source_symbol", "source_commit",
    ]
    frozen_root = repo / "work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967"
    for row in fields:
        assert all(row[column].strip() for column in required_columns), row["id"]
        assert row["source_commit"] == upstream, row["id"]
        assert re.fullmatch(r"SD-\d{2}", row["owner_work_package"]), row["id"]
        assert 1 <= int(row["owner_work_package"].split("-")[1]) <= 18
        assert (frozen_root / row["source_file"]).is_file(), row["source_file"]

    links_checked = 0
    for document in plan.rglob("*.md"):
        content = document.read_text(encoding="utf-8")
        for target in re.findall(r"\[[^\]\n]+\]\(([^)\n]+)\)", content):
            target = target.strip("<>")
            if "://" in target or target.startswith("#"):
                continue
            relative = target.split("#", 1)[0]
            assert (document.parent / relative).exists(), f"Broken link {document}: {target}"
            links_checked += 1
    result = {
        "validation_passed": True,
        "scope": "documentation consistency only",
        "integration_cards": len(tasks),
        "findings_mapped": len(expected_findings),
        "field_ids": len(fields),
        "field_kinds": dict(kinds),
        "frozen_source_files_exist": True,
        "dependency_graph_acyclic": True,
        "topological_order": topological,
        "local_links_checked": links_checked,
        "production_edits": False,
        "product_tests_this_turn": "未运行",
        "product_status": "identified",
    }
    (plan / "document-validation.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result, ensure_ascii=True, indent=2))


if __name__ == "__main__":
    main()
