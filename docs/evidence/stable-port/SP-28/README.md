# SP-28 只读核对与夹具准备 — 证据（未实施，不标 verified）

状态：identified。基线：HEAD `92d46dd`。生产代码零改动；`work/`、`outputs/` 只读未动；`compat/` 未动。不 commit。
方法：只读扫描 `SETTINGS_IMPLEMENTATION_180.csv`（180 行）与
`settings-current-180.csv`（134 implemented / 46 identified），对照
`crates/config_codegen`、`crates/application`、`crates/runtime/src/adapter.rs`
真实符号，复核 SP-23 已登记 18 叶（G-01..G-09 在本基线仍成立），其余缺口全部登记为建议卡，不削减范围。

## 1. 只读命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `python tools/sp28_readonly_audit.py` | 0 | `rows=180 gaps=34 implemented=134 unverified_effect=180 sp23_files=18 cores=14`；生成本目录 `field-gap-matrix.csv`、`core-matrix.csv` |
| 只读 `rg` 核对（G-01..G-07） | 0 | 下列符号位置与 SP-23 一致，无漂移：`input.rs:562` 声明/`dns.rs:381` 无条件写 `happyEyeballs`；`settings.rs:93 parse::<u32>`；`option_setting_window.dart:1032-1035` 仅表单；`webdav.rs:110` 自建 Client；`codegen.rs:461` 定义、调用仅 `mod tests` 内 1076/1092 行；`input.rs:469` 声明/`ruleset.rs:82` 消费、无生产填充者；`engine.rs:4840 native_custom` 直通 |
| `Test-Path tools/cores/<exe>` ×14 | 0 | 14 核二进制均在位（见 core-matrix.csv） |
| cargo / flutter 门禁 | 未运行 | 本卡零生产改动，按 VALIDATION_POLICY.md 不跑发布门禁 |

## 2. 缺口清单摘要（34 个 identified 且未被 SP-23 登记；详见 field-gap-matrix.csv）

- 内部 2：FLD-CFG-001/002（SD-03）→ 建议 SP-03。
- 平台/自启 5（SD-05→SP-15/SP-32）：056 AutoRun；138/139/140/141 系统代理组。
- UI 缓存 12（SD-06→SP-16）：067/068/069/081/082/083/117/118/119/136/156/157/158（列宽/窗口几何多为 native/INI 私有缓存，无 canonical 消费者）。
- TUN 4（SD-07→SP-24 实现 + SP-10 生效）：100/102/103/105。
- 路由 1（SD-15→SP-13）：116 RoutingIndexId。
- 日志/统计/ClashUI 5（SD-13→SP-17）：065/066 MsgUI；131/132/134 ClashUI 轮询（CP-SET-03 跨 SP-01/02/12 前置）。
- 更新 2（SD-16→SP-27）：147/148。
- 跨平台 3（SD-18→SP-32/SP-33）：080 macOS Dock；142 PAC 脚本路径；均须隔离环境，宿主不写。
- 冲突类（状态为 implemented 但 `actual_effect_verified=false`，180/180 全量）：静态可达 ≠ 生效。代表：G-01 Happy 开关零门控、G-02 MaxSplit `"1-3"` 校验与 wire 首整数分裂、G-06 mihomo custom 正式 merge 缺失、G-07 `local_srs_files` 无生产填充、非 SingBox 全走 `generate_xray`（v2fly/v2fly_v5 共享模板，逐核有效性待 SP-28 L1）、9 独立核走 `native_custom` 原文直通。另 G-08 `validate_plan.py` stale 断言 → SP-00；G-09 18 叶均缺 E2E → 各移交卡（SP-24/25/26）。

## 3. 14 核夹具状态（L0 可立即跑，L1 全部等待前置；详见 core-matrix.csv）

- L0（现在可跑，无监听、无 OS 副作用）：14/14 —— fixture（`fixtures/synthetic/sp28/<core>-session.json`，127.0.0.1:11808-11821）+ exe 在位 + `<exe> version` 身份确认（version 仅身份识别，不作 SP-28 验收）。
- L1（真实 GUI→FRB→host→flow/exit，均 blocked）：xray/sing-box/v2fly/v2fly_v5 待 SP-24（G-01/G-02）+ SP-04/05/06/14 链路 verified 化；mihomo 待 SP-24（G-06）(+ TUN 生效待 SP-10 identified)；9 独立核（hysteria/hysteria2/naiveproxy/tuic/juicity/brook/overtls/shadowquic/mieru）待 SP-28 L1 门禁（passthrough 形状在运行时固定）。
- SP-10（TUN 提权）identified：任何 TUN 真实生效不安排，仅回环 L0。

## 4. 新建文件与 git 状态

见最终消息。收尾要求：除 `tools/sp28_readonly_audit.py`、`fixtures/synthetic/sp28/`（14）、本目录（3）、`docs/evidence/stable-port/SP-23/SP-28-readonly-note.md` 外无其它改动；生产代码零改动。

## 5. L1 续审（2026-10-06，只读；`field-gap-matrix.csv` 34 → 37 行）

方法：沿上游冻结 `work/research-v2rayn/source/2dust-v2rayN-2813985/v2rayN/ServiceLib`
（`Handler/CoreConfigHandler.cs`、`Services/CoreConfig/*`、`Manager/*`、`ViewModels/*`、
`Enums/*`、`Models/*`）逐区 `rg`/源码对照 `crates/`、`apps/desktop/lib` 真实符号；
只读查询（`rg`/CSV 解析），未启动任何内核进程，未写 OS（代理/路由/TUN/DNS/Run 键），
未跑 cargo/flutter，未改 `SP-28/` 之外任何文件，不 commit。
新增行 `row_kind=l1`（行为级 L1 缺口，非 180 字段表成员），状态均为 identified
（已登记、正式入口→真实 flow/exit 未验证，不写 verified）。

- SP28-L1-001（update，→SP-27）：更新窗无 GeoFiles 行。上游
  `ViewModels/CheckUpdateViewModel.cs:82-91`（`GetGeoFileCheckUpdateModel`，
  `IsGeoFile=true`，`SelectedCoreTypes` 存 `GeoFiles`）、`:195-197`（选中即走
  `CheckUpdateGeo`）、`:235-247`（`UpdateService.UpdateGeoFileAll`）；
  当前 `crates/application/src/update_service.rs:131-135`（`ui_targets` 仅
  v2rayN+14 核）、`apps/desktop/lib/features/update/update_controller.dart:130-151,213-221`
 （targets/seed/save 均为 core-only），`features/update` 内零 geo 命中。
- SP28-L1-002（custom/profile，→SP-03）：证书链获取仅叶子。上游
  `ViewModels/AddServerViewModel.cs:519-544`（`FetchCertChain` 经
  `CertPemManager.GetCertChainPemAsync` + `ConcatenatePemChain` 返回完整链）；
  当前 `apps/desktop/lib/features/profiles/profile_fields.dart:945-975`
 （`leafOnly` 恒 true，`dart:io` 只暴露叶子），接线见
  `profile_editor_dialog.dart:440-468`。
- SP28-L1-003（mihomo/clash，→SP-24）：custom YAML 预处理缺失。上游
  `Services/CoreConfig/CoreConfigClashService.cs:55-65`（`!<str>` 标签替换、
  `<<:`/`*`/`&` 锚预处理）与 `:123-143`（REALITY short-id 防 float 加引号）；
  当前 `crates/application/src/mixin.rs:49-60` 为裸 `serde_yaml` 解析，
  `:75-125` 的 merge 复刻了改写/mixin（`mixed-port`/`log-level`/`external-controller`/
  `secret`/`allow-lan`/`ipv6`/`mode`/TUN/`prepend-/append-/removed-`）但无上述预处理。

逐区结论（有缺口则已编号，无则给出守卫证据，不推测）：
xray——`CoreConfigHandler.cs:28-30` 非 custom/非 sing-box 统一走 V2ray 路径，
  当前 `codegen.rs:313-319` 同策略；balancer/observatory（`xray/routing.rs`、
  `xray/outbound.rs:1084-1132`）、sing-box 能力门（`singbox/mod.rs:29-45` 11 类 =
  `Global.cs:378-391`）、日志映射（`singbox/log.rs:11-21` = `SingboxLogService.cs:6-38`）、
  geosite/geoip→ruleset（`singbox/ruleset.rs:1-155` = `SingboxRulesetService.cs`）、
  TUN inbound（`singbox/inbound.rs:27-70`）、direct-exe 规则
  （`xray/routing.rs:166-172` 对 `V2rayRoutingService.cs:239-278`）均对齐。
mihomo/clash——Clash API 客户端覆盖
  `ClashApiManager.cs:11-184` 全方法（`clash_api.rs:194-475`，
  `update_mode` 即 `UpdateClashMode→UpdateConfig` 唯一生产调用）；
  组 delay 批量（`clashGroupDelay`，`monitor_controller.dart:608`）对齐；
  仅 SP28-L1-003 新增。
custom——`generate()` 分发、`native_custom`（`engine.rs:4875-4927`）与上游
  `CoreConfigHandler.cs:16-31`（mihomo-custom merge、其余原文直通）同形；
  mihomo 非 custom 走 xray JSON 与上游 else 分支一致；`ECoreType` 14 核+99
  （`enums.rs:140-255`）与上游 `ECoreType.cs` 对齐；Clash 订阅全文导入
  （`subscriptions/.../batch.rs:74-75 is_clash_full` 对 `ConfigHandler.cs:1768`）对齐；
  仅 SP28-L1-002 新增。
tun——`TunModeItem` 11 字段全量存在（`settings.rs:266-306` 对 `ConfigItems.cs:141-154`），
  计划/校验（`tun_plan.rs`、`codegen.rs:439-447`）对齐；单 TUN provider 抑制
  （`engine.rs:4859-4874`）保留。
dns——`DNSItem` 9 字段（`entities.rs:131-144` 对 `DNSItem.cs`）、`SimpleDNSItem`
  18 字段、`HappyEyeballs4RayItem` 对齐；`domain_dns_address`/
  `strategy4_freedom` 到达 xray（`xray/dns.rs:211-264,405-457`）与 sing-box
  （`singbox/dns.rs:727-732`）均有消费者。
routing——`RoutingProfile` 13 字段（`entities.rs:92-109` 对 `RoutingItem.cs`）、
  `RulesItem` 13 字段（`entities.rs:61-78` 对 `RulesItem.cs`，含 `process`/
  `enabled`/`rule_type`）、`ERuleMode`/`ERuleType`/`ESysProxyType` 枚举值对齐。
subscriptions——`SubItem` 17 字段全量（`entities.rs:18-42` 对 `SubItem.cs`）；
  调度器（订阅+Geo 定时，`engine.rs:5097-5225`）对 `TaskManager.cs:22-155`
  同形（1 分钟订阅检查、20 分钟落盘、小时级 Geo、日级更新检查）。
update——core 更新管线（`update_service.rs` + `updater/channel.rs` 对
  `UpdateService.cs:49-137`/`CoreInfoManager.cs`）对齐；仅 SP28-L1-001 新增
  （更新窗 Geo 行）。
platform/proxy——PAC 服务（`platform/src/pac.rs:25-92` 对 `PacManager.cs:15-58`，
  `__PROXY__` 替换一致）、bypass `<local>;`（`sysproxy/windows.rs:456-469` 对
  `SysProxyHandler.cs:85-91`）对齐；热键 5 动作（`hotkeys.dart:9-27` 对
  `EGlobalHotkey.cs`）+ 编辑期 pause（`hotkeys.dart:475-548` 对
  `HotkeyManager.cs:10,145`）对齐。
webdav——check/list/upload/download（`webdav.rs:198-267` 对
  `WebDavManager.cs:98-183`）、远端目录/文件名（`webdav.rs:14-16`
  `v2rayN_backup`/`backup.zip` 对 `WebDavManager.cs:13-15`）、远端备份/恢复接线
  （`backup_controller.dart:518-560` 对 `BackupAndRestoreViewModel.cs:60-90`）对齐。
backup——本地备份/恢复/校验/识别（`backup_service.rs:72-335` 对
  `BackupAndRestoreViewModel.cs:92-143`）同形；`backup_*.zip` 往返
  （`backup_picker.dart:6-11`）对齐。

附带只读发现（非缺口行，不改其它目录）：`tools/sp28_readonly_audit.py:39`
  断言 SP-23 仅 18 个 FLD 文件，当前已有 36 个（第二批 +18），该脚本现 exit=1；
  修脚本属整合者事项，本卡仅记录。core-matrix.csv 未变（L1 仍全部 blocked，
  待 SP-24/SP-28 L1 门禁）。
