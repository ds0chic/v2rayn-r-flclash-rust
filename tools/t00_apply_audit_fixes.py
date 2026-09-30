"""T00 audit remediation.

Applies the union of Gemini 3.8 Flash and Muse Spark 1.3 audit findings to the
T00 ledgers, then regenerates compat/fields.yaml from its two partitions.

Read-only inputs: work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967
Writes only: compat/{layouts,fields.settings,fields.entities,actions,features,fields}.yaml
"""

import copy
import json
import sys

import yaml

ROOT = r"C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn"
COMPAT = f"{ROOT}/compat"
COMMIT = "7d6a967c18c697f28dc6917122ed3a4993fcf336"

changes = []


def load(name):
    with open(f"{COMPAT}/{name}", encoding="utf-8") as fh:
        return yaml.safe_load(fh)


def dump(name, data, header=None):
    text = yaml.safe_dump(
        data, allow_unicode=True, sort_keys=False, default_flow_style=False, width=4096
    )
    if header:
        text = header + text
    with open(f"{COMPAT}/{name}", "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)


def note(rec, text):
    changes.append({"id": rec.get("id"), "note": text})


def set_unresolved(rec, items):
    rec["unresolved"] = items
    note(rec, f"unresolved -> {items}")


def downgrade(rec, reason):
    if rec.get("default_verified") is True:
        rec["default_verified"] = False
        u = rec.get("unresolved") or []
        msg = f"审计整改：{reason}"
        if msg not in u:
            u.append(msg)
        rec["unresolved"] = u
        note(rec, f"default_verified true->false: {reason}")


# ---------------------------------------------------------------------------
# layouts.yaml
# ---------------------------------------------------------------------------
layouts = load("layouts.yaml")

inv_missing = [i["id"] for i in layouts["window_inventory"] if "status" not in i]
for inv in layouts["window_inventory"]:
    inv.setdefault("status", "identified")
changes.append({"file": "layouts.yaml", "note": f"window_inventory status added: {len(inv_missing)} entries"})

lay = {i["id"]: i for i in layouts["items"]}

lay["LAY-MAIN-004"]["unresolved"] = []
lay["LAY-MAIN-004"]["evidence"] += ["v2rayN/v2rayN.Desktop/Views/MainWindow.axaml:101"]
lay["LAY-MAIN-004"]["notes"] = "审计 FND-003：menuPromotion 在 Avalonia 亦存在（MainWindow.axaml:101），原 unresolved 关闭。"
note(lay["LAY-MAIN-004"], "promotion menu unresolved closed")

lay["LAY-PROFILES-002"]["unresolved"] = []
lay["LAY-PROFILES-002"]["evidence"] += [
    "v2rayN/v2rayN.Desktop/Views/ProfilesView.axaml.cs:367",
    "v2rayN/v2rayN.Desktop/Views/ProfilesView.axaml.cs:371",
    "v2rayN/v2rayN.Desktop/Views/ProfilesView.axaml.cs:428",
]
lay["LAY-PROFILES-002"]["notes"] = "审计 FND-003：Avalonia 列恢复由 ProfilesView.axaml.cs RestoreUI(:367)/MainColumnItem(:371)/StorageUI(:428) 驱动，原 unresolved 关闭。"
note(lay["LAY-PROFILES-002"], "avalonia column restore unresolved closed")

lay["LAY-ADDSERVER-001"]["unresolved"] = []
lay["LAY-ADDSERVER-001"]["evidence"] += [
    "v2rayN/v2rayN/Views/AddServerWindow.xaml:151",
    "v2rayN/v2rayN/Views/AddServerWindow.xaml:1009",
]
lay["LAY-ADDSERVER-001"]["notes"] = "审计 FND-003：WPF AddServerWindow.xaml 同名 gridVMess(:151)…gridTransport(:1009)，两套分区命名一致，原 unresolved 关闭。"
note(lay["LAY-ADDSERVER-001"], "addserver grid mapping unresolved closed")

lay["LAY-SUBSET-001"]["anchors"] = [a if a != "Avalonia" else "Avalonia lstSubscription:33" for a in lay["LAY-SUBSET-001"]["anchors"]]
lay["LAY-SUBSET-001"]["evidence"] += ["v2rayN/v2rayN.Desktop/Views/SubSettingWindow.axaml:33"]
note(lay["LAY-SUBSET-001"], "avalonia anchor added")

lay["LAY-FULLCONFIG-001"]["anchors"] = [a if a != "Avalonia" else "Avalonia rayFullConfigTemplate:102 / sbFullConfigTemplate:180" for a in lay["LAY-FULLCONFIG-001"]["anchors"]]
lay["LAY-FULLCONFIG-001"]["evidence"] += [
    "v2rayN/v2rayN.Desktop/Views/FullConfigTemplateWindow.axaml:26",
    "v2rayN/v2rayN.Desktop/Views/FullConfigTemplateWindow.axaml:102",
    "v2rayN/v2rayN.Desktop/Views/FullConfigTemplateWindow.axaml:180",
]
note(lay["LAY-FULLCONFIG-001"], "avalonia anchors added")

lay["LAY-BACKUP-001"]["anchors"] = [a if a != "Avalonia" else "Avalonia menuWebDavCheck:162" for a in lay["LAY-BACKUP-001"]["anchors"]]
lay["LAY-BACKUP-001"]["evidence"] += ["v2rayN/v2rayN.Desktop/Views/BackupAndRestoreView.axaml:162"]
note(lay["LAY-BACKUP-001"], "avalonia anchor added")

lay["LAY-CHECKUPDATE-001"]["anchors"] = [a if a != "Avalonia" else "Avalonia togEnableUpdateViaProxy:34 / btnCheckUpdate:55 / lstCheckUpdates:68" for a in lay["LAY-CHECKUPDATE-001"]["anchors"]]
lay["LAY-CHECKUPDATE-001"]["evidence"] += [
    "v2rayN/v2rayN.Desktop/Views/CheckUpdateView.axaml:34",
    "v2rayN/v2rayN.Desktop/Views/CheckUpdateView.axaml:55",
    "v2rayN/v2rayN.Desktop/Views/CheckUpdateView.axaml:68",
]
note(lay["LAY-CHECKUPDATE-001"], "avalonia anchors added")

dump("layouts.yaml", layouts)

# ---------------------------------------------------------------------------
# fields.settings.yaml
# ---------------------------------------------------------------------------
fs = load("fields.settings.yaml")
fm = {i["id"]: i for i in fs["items"]}

downgrade(fm["FLD-CFG-001"], "CLR 缺省（无初始化器）；ConfigHandler 无 IndexId 初始化分支，默认值实为选中态回写（A03）。")
downgrade(fm["FLD-CFG-041"], "CLR 缺省 false；ConfigHandler 新建 Inbound 未显式初始化该字段（A03）。")
downgrade(fm["FLD-CFG-042"], "CLR 缺省 false；ConfigHandler 新建 Inbound 未显式初始化该字段（A03）。")
downgrade(fm["FLD-CFG-044"], "CLR 缺省 null；未见 LoadConfig/初始化追踪（A03）。")
downgrade(fm["FLD-CFG-118"], "CLR 缺省 0；未见初始化路径追踪（A03）。")
downgrade(fm["FLD-CFG-130"], "CLR 缺省 false；未见初始化路径追踪（A03）。")

fm["FLD-CFG-056"]["evidence"] = [
    "v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:69",
    "v2rayN/ServiceLib/ViewModels/OptionSettingViewModel.cs:43",
    "v2rayN/ServiceLib/ViewModels/OptionSettingViewModel.cs:176",
    "v2rayN/ServiceLib/ViewModels/OptionSettingViewModel.cs:351",
    "v2rayN/ServiceLib/Handler/AutoStartupHandler.cs:15",
    "v2rayN/v2rayN/Views/OptionSettingWindow.xaml.cs:95",
]
fm["FLD-CFG-056"]["notes"] = "保存时调用 AutoStartupHandler.UpdateTask（OptionSettingViewModel.cs:402）；默认值追踪行已按 FND-002 修正为 :43/:176/:351。"
note(fm["FLD-CFG-056"], "evidence lines fixed (FND-002)")

fm["FLD-CFG-062"]["unresolved"] = []
fm["FLD-CFG-062"]["evidence"] += [
    "v2rayN/v2rayN/Views/MainWindow.xaml.cs:151",
    "v2rayN/v2rayN/Views/MainWindow.xaml.cs:153",
]
fm["FLD-CFG-062"]["notes"] = "硬件加速；needReboot 集合内。消费点：MainWindow 构造按 !EnableHWA 设 RenderOptions.ProcessRenderMode=SoftwareOnly（审计 FND-004，原 unresolved 关闭）。"
note(fm["FLD-CFG-062"], "EnableHWA unresolved closed")

fm["FLD-CFG-098"]["default_value"] = None
fm["FLD-CFG-098"]["effective_fallback"] = "gvisor（空值时 SingboxInboundService 取 TunStacks.First()）"
downgrade(fm["FLD-CFG-098"], "有效值 gvisor 系生成器运行时回退（SingboxInboundService.cs:60-62），非持久默认（A02）。")

fm["FLD-CFG-112"]["default_value"] = None
fm["FLD-CFG-112"]["effective_fallback"] = "1000（null 时 SpeedtestService 回退 Global.SpeedTestPageSize）"
downgrade(fm["FLD-CFG-112"], "有效值 1000 系 SpeedtestService 运行时回退（SpeedtestService.cs:12），非持久默认（A02）。")

fm["FLD-CFG-170"]["unresolved"] = []
fm["FLD-CFG-170"]["evidence"] += [
    "v2rayN/ServiceLib/Services/CoreConfig/V2ray/V2rayDnsService.cs:50",
    "v2rayN/ServiceLib/Services/CoreConfig/V2ray/V2rayDnsService.cs:60",
]
fm["FLD-CFG-170"]["generated_into"] = {"xray": ["dns outbounds-any.targetStrategy（非 AsIs 时）"], "sing-box": []}
fm["FLD-CFG-170"]["notes"] = "Xray DNS outbound targetStrategy；审计 FND-004 关闭原 unresolved（此前未定位读取点的说法作废）。"
note(fm["FLD-CFG-170"], "Strategy4Proxy unresolved closed")

# Systemic A03 sweep: default_verified requires load/init/UI-tracing evidence.
markers = (
    "ConfigHandler.cs", "AppManager.cs", "OptionSettingViewModel.cs",
    "ThemeSettingViewModel.cs", "DNSSettingViewModel.cs", "SubEditViewModel.cs",
    "GlobalHotkeySettingViewModel.cs", "CheckUpdateViewModel.cs", "MsgViewModel.cs",
    ".xaml",
)
swept = 0
for it in fs["items"]:
    if it.get("default_verified") is True:
        ev = " ".join(it.get("evidence") or [])
        if not any(m in ev for m in markers):
            downgrade(it, "系统筛查（A03）：仅有 initializer/消费方证据，未见 LoadConfig/初始化/界面初始化追踪。")
            swept += 1
changes.append({"file": "fields.settings.yaml", "note": f"systemic default_verified downgrades: {swept}"})

dump("fields.settings.yaml", fs)

# ---------------------------------------------------------------------------
# fields.entities.yaml
# ---------------------------------------------------------------------------
fe = load("fields.entities.yaml")
em = {i["id"]: i for i in fe["items"]}

em["FLD-ENT-015"]["default_verified"] = False
em["FLD-ENT-015"]["default_source"] = "无初始化器，默认 null（仅迁移读取；审计 A03 降级）"
note(em["FLD-ENT-015"], "default_verified downgraded (A03)")

em["FLD-ENT-139"]["classification"] = "obsolete_candidate"
em["FLD-ENT-139"]["generated_into"] = "none（输入 Type 从未被生成器读取；Xray text 规则输出恒为 field，分支键为 RuleType）"
em["FLD-ENT-139"]["unresolved"] = []
em["FLD-ENT-139"]["evidence"] = [
    "v2rayN/ServiceLib/Models/Entities/RulesItem.cs:7",
    "v2rayN/ServiceLib/Services/CoreConfig/V2ray/V2rayRoutingService.cs:130",
]
em["FLD-ENT-139"]["notes"] = "审计 A06/U4：疑似死字段；保留数据不删行。生成期输出恒为 type=field，真正分支键是 ERuleType RuleType。"
note(em["FLD-ENT-139"], "U4 closed; obsolete_candidate")

ent_swept = 0
for it in fe["items"]:
    if it.get("default_verified") is True:
        ev = " ".join(it.get("evidence") or [])
        if not any(m in ev for m in markers):
            downgrade(it, "系统筛查（A03）：仅有 initializer/消费方证据，未见加载/迁移/初始化追踪。")
            ent_swept += 1
changes.append({"file": "fields.entities.yaml", "note": f"systemic default_verified downgrades: {ent_swept}"})

dump("fields.entities.yaml", fe)

# ---------------------------------------------------------------------------
# actions.yaml
# ---------------------------------------------------------------------------
act = load("actions.yaml")
am = {i["id"]: i for i in act["items"]}

am["ACT-ROUTE-003"]["entry"]["mouse"] = None
am["ACT-ROUTE-003"]["notes"] = "审计 A04：双击为编辑（RoutingSettingWindow.xaml.cs:93-95），设为默认仅 Enter/菜单（:67-70）。"
note(am["ACT-ROUTE-003"], "mouse misattribution fixed")

am["ACT-STAT-004"]["entry"]["mouse"] = "单击/按下（PreviewMouseDown；双击亦触发）"
am["ACT-STAT-004"]["notes"] = "审计 A04：绑定为 PreviewMouseDown（StatusBarView.xaml.cs:15-16），非严格双击事件。"
note(am["ACT-STAT-004"], "mouse description fixed")

am["ACT-MAIN-020"]["entry"]["tray"] = None
am["ACT-MAIN-020"]["scheduled"] = None
am["ACT-MAIN-020"]["notes"] = "审计 A05/FND-001：托盘入口为 ACT-TRAY-010（StatusBarViewModel.SubUpdateCmd）；SCH-001 由 TaskManager.cs:104 直接调用 SubscriptionHandler.UpdateProcess，不经过本命令。"
note(am["ACT-MAIN-020"], "tray/scheduled attribution fixed")

am["ACT-MAIN-021"]["entry"]["tray"] = None
am["ACT-MAIN-021"]["notes"] = "审计 A05：托盘入口为 ACT-TRAY-011（StatusBarViewModel.SubUpdateViaProxyCmd）。"
note(am["ACT-MAIN-021"], "tray attribution fixed")

am["ACT-PROF-038"]["notes"] = "审计 A05：本条是测速任务的取消入口；被取消任务 cancellable=true，见 ACT-PROF-014~019。"
note(am["ACT-PROF-038"], "cancellation semantics clarified")

for rule in act["hotkey_scope_rules"]:
    if rule["id"] == "HKR-001":
        rule["evidence"] = [
            "v2rayN/v2rayN/Views/ProfilesView.xaml.cs:220",
            "v2rayN/v2rayN/Views/ProfilesView.xaml.cs:222",
            "v2rayN/v2rayN/Views/ProfilesView.xaml.cs:260",
        ]
        note(rule, "evidence line corrected")

route008 = {
    "id": "ACT-ROUTE-008",
    "name": "路由设置-双击编辑规则集",
    "domain": "routing",
    "entry": {"menu_path": None, "shortcut": None, "mouse": "双击规则集行", "tray": None, "hotkey": None},
    "scope": "双击行对应规则集",
    "selection_rule": "MouseDoubleClick 绑定于 lstRoutings",
    "confirm": "无",
    "cancel": "关闭编辑窗口丢弃草稿",
    "async": {"job": False, "cancellable": False},
    "feedback": "打开 RoutingRuleSettingWindow(blNew=false)",
    "scheduled": None,
    "upstream_symbol": "RoutingSettingWindow.LstRoutings_MouseDoubleClick -> RoutingSettingViewModel.RoutingAdvancedEditAsync(false)",
    "source_file": [
        "v2rayN/v2rayN/Views/RoutingSettingWindow.xaml.cs",
        "v2rayN/ServiceLib/ViewModels/RoutingSettingViewModel.cs",
    ],
    "evidence": [
        "v2rayN/v2rayN/Views/RoutingSettingWindow.xaml.cs:12",
        "v2rayN/v2rayN/Views/RoutingSettingWindow.xaml.cs:93",
        "v2rayN/ServiceLib/ViewModels/RoutingSettingViewModel.cs:119",
    ],
    "implementation_location": None,
    "test_ids": [],
    "status": "identified",
    "notes": "审计 A04 拆分：双击=编辑，与原 ACT-ROUTE-003 的设为默认区分。",
}
act["items"].append(route008)
changes.append({"file": "actions.yaml", "note": "added ACT-ROUTE-008 (double-click edit)"})

dump("actions.yaml", act)

# ---------------------------------------------------------------------------
# features.yaml
# ---------------------------------------------------------------------------
feat = load("features.yaml")
fmap = {i["id"]: i for i in feat["features"]}

fd7 = fmap["F-DESKTOP-007"]
fd7["summary"] = "通过命名事件/互斥体防止多实例运行"
fd7["upstream_entry"] = {
    "window": "App / Program",
    "menu_path": None,
    "symbol": "App.ProgramStarted / ProgramStarted + Mutex",
    "files": ["v2rayN/v2rayN/App.xaml.cs", "v2rayN/v2rayN.Desktop/Program.cs"],
}
fd7["normal_behavior"] = "WPF 用 EventWaitHandle 抢锁（App.xaml.cs:11,29）；Avalonia Windows 用 EventWaitHandle、非 Windows 用命名 Mutex(\"v2rayN\")（Program.cs:32,41）；抢锁失败的第二实例退出。"
fd7["failure_behavior"] = "单实例锁失败时第二实例退出（Program.cs:42）。"
fd7["cancel_behavior"] = "无"
fd7["acceptance"] = "第二实例启动即退出；reboot-as-admin 除外路径按源码。"
fd7["evidence"] = [
    "v2rayN/v2rayN/App.xaml.cs:11",
    "v2rayN/v2rayN/App.xaml.cs:29",
    "v2rayN/v2rayN.Desktop/Program.cs:32",
    "v2rayN/v2rayN.Desktop/Program.cs:41",
]
fd7["unresolved"] = ""
fd7["notes"] = "审计 A06：原 unresolved 关闭（单实例实现已定位）。"
note(fd7, "single-instance resolved")

fm1 = fmap["F-MONITOR-001"]
fm1["evidence"] += [
    "v2rayN/ServiceLib/ViewModels/MsgViewModel.cs:12",
    "v2rayN/ServiceLib/ViewModels/MsgViewModel.cs:20",
    "v2rayN/ServiceLib/ViewModels/MsgViewModel.cs:109",
]
fm1["notes"] = "审计 A06/FND-004：日志过滤（MsgFilter/DoMsgFilter）与自动刷新（AutoRefresh）已实现；清空 ClearMsg 在冻结版本被注释（MsgViewModel.cs:109-112）。"
note(fm1, "log filter/pause resolved, clear noted")

fd1 = fmap["F-DESKTOP-001"]
fd1["evidence"] += [
    "v2rayN/ServiceLib/ViewModels/StatusBarViewModel.cs:229",
    "v2rayN/ServiceLib/ViewModels/StatusBarViewModel.cs:230",
    "v2rayN/v2rayN.Desktop/App.axaml:43",
    "v2rayN/v2rayN.Desktop/App.axaml:51",
]
note(fd1, "evidence completed (A07-L4)")

fc3 = fmap["F-CORE-003"]
fc3["evidence"] += ["v2rayN/AmazTool/UpgradeApp.cs:24"]
note(fc3, "UpgradeApp evidence added (A07-L2)")

dump("features.yaml", feat)

# ---------------------------------------------------------------------------
# Regenerate fields.yaml (mechanical merge of the two partitions)
# ---------------------------------------------------------------------------
fs2 = load("fields.settings.yaml")
fe2 = load("fields.entities.yaml")
for key in ("schema_version", "source_commit", "section"):
    fs2.pop(key, None)
for key in ("schema_version", "source_commit", "section", "source_version"):
    fe2.pop(key, None)
merged = {
    "schema_version": 1,
    "source_commit": COMMIT,
    "merged_from": ["fields.settings.yaml", "fields.entities.yaml"],
    "settings": fs2,
    "entities": fe2,
}
header = (
    "# fields.yaml - T00 merged fields ledger (settings + entities + migration + references)\n"
    "# Mechanical merge of fields.settings.yaml + fields.entities.yaml; partitions kept as provenance.\n"
    "# Audit remediation applied 2026-10-01 (see docs/evidence/T00.audit.md).\n"
    "# Entry schema: compat/SCHEMA.md. UTF-8, no tabs.\n\n"
)
dump("fields.yaml", merged, header=header)
changes.append({"file": "fields.yaml", "note": "regenerated from patched partitions"})

# ---------------------------------------------------------------------------
with open(f"{ROOT}/docs/evidence/audit/T00.fix-report.json", "w", encoding="utf-8") as fh:
    json.dump({"changes": changes}, fh, ensure_ascii=False, indent=1)

print(f"changes: {len(changes)}")
print(f"settings systemic downgrades: {swept}")
print(f"entities systemic downgrades: {ent_swept}")
print(f"layouts inventory status added: {len(inv_missing)}")
sys.exit(0)
