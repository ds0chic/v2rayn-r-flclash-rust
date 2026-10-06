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
