# SP-23 逐字段实例登记 — 证据（第一批：CP-11 高优先 18 叶）

状态：implemented（文档登记完成；正式入口→最终消费者实测未跑，不写 verified）。
基线：HEAD `7466bee13f2c82241efb41ebad2cf74368ae09ab`（= 任务基线 `7466bee`，`git status` 干净）。
上游冻结：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
审计基线：`a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。
允许模块内作业：仅 `docs/repair/stable-port-2026-10-06`（读）与 `docs/evidence/stable-port/SP-23/`（写）。
生产代码零改动；`work/`、`outputs/` 只读未动；`compat/` 未动。不要 commit，根整合者验收提交。

## 0. 方法（本批如何产生）

1. 核对 `SETTINGS_IMPLEMENTATION_180.csv`（180 行：2 internal / 23 container / 155 leaf，
   与 `compat/fields.settings.yaml` ID 集合一致）与
   `docs/evidence/complete-port-audit-2026-10-06/settings/settings-current-180.csv`
   （134 implemented / 46 identified，均为静态定位、非端到端验证）对应行。
2. 对本批 18 个叶 ID 沿源码 `rg` 读取真实消费者（provider / caller / DTO /
   生效点），逐项登记缺口，不把“写表单存在”当消费证明。
3. 每个字段一个独立实例文件（`FLD-CFG-<n>.md`），按 `tasks/FIELD_INSTANCE_TEMPLATE.md`
   逐节填写：唯一用户流程、原版符号、当前 provider/caller、DTO、版本/错误/
   取消/持久化/生效时机、最终唯一验收流程、未验证前置。不得整批标完成，不得自造 ID。

## 1. 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `python docs/repair/stable-port-2026-10-06/build_settings_implementation_matrix.py` | 0 | `rows:180, row_kinds:{internal:2,container:23,leaf:155}`；CSV 与生成器一致，`git status` 仍干净（字节一致） |
| `python validate_plan.py`（同目录） | 1 | `AssertionError` 于 `assert all(task["status"] == "identified")`：validator  stale——manifest 中 SP-00/01/02/03/04/11 已为 `implemented`，断言仍要求全 `identified`。本卡仅记录，不修 validator（修它属 SP-00/整合者事项，已登记为缺口 G-08） |
| Rust / Flutter 门禁 | 未运行 | 本卡零生产代码改动，按卡面要求不跑；误改则必须回退（未发生） |

## 2. 本批完成的字段 ID（18 叶）与各自唯一验收流程摘要

| ID | 最终消费者（当前真实定位） | 唯一验收流程 | 移交 |
|---|---|---|---|
| FLD-CFG-176 EnableHappyEyeballs | `config_codegen/src/xray/dns.rs:374-382` 应门控但未读开关 | DNS 窗关开关→保存→restart_core→生成 JSON 无 `happyEyeballs` 块→真实 xray 校验通过→重开仍关 | SP-24 |
| FLD-CFG-177 TryDelayMs | 同上（参数通道 `codegen.rs:399`） | 开关开+非默认 300ms→`tryDelayMs:300` 落盘生效；开关关→同参数不输出 | SP-24 |
| FLD-CFG-178 PrioritizeIPv6 | 同上（`codegen.rs:400`） | 开+true→`prioritizeIPv6:true`；关→不输出（dualstack 合成对照） | SP-24 |
| FLD-CFG-179 Interleave | 同上（`codegen.rs:401`） | 开+2→`interleave:2`；关→不输出 | SP-24 |
| FLD-CFG-180 MaxConcurrentTry | 同上（`codegen.rs:402-403`） | 开+3→`maxConcurrentTry:3`；关→不输出 | SP-24 |
| FLD-CFG-150 Packets | `config_codegen/src/xray/config.rs:248-273` fragment секции | raw `Packets=tlshello`→wire `packets`→xray 校验→重开往返 | SP-24 |
| FLD-CFG-151 Lengths | 同上 | `Lengths=50-100`→wire `lengths`→校验→重开 | SP-24 |
| FLD-CFG-152 Delays | 同上 | `Delays=10-20`→wire `delays`→校验→重开 | SP-24 |
| FLD-CFG-153 MaxSplit | 同上 wire 首整数；`application/src/settings.rs:91-99` 校验门 | `MaxSplit=1-3` 可保存且 wire `maxSplit=1`（首整数），`0/10000/倒序/负数/多段/空/非法`按冻结语义 | SP-24 |
| FLD-CFG-154 Length（legacy） | 同上（legacy 迁移源） | 旧值迁移→lengths 首值→wire→重开 | SP-24 |
| FLD-CFG-155 Interval（legacy） | 同上（legacy 迁移源） | 旧值迁移→delays 首值→wire→重开 | SP-24 |
| FLD-CFG-062 EnableHWA | 无 runner 消费者（`apps/desktop/windows` 零命中） | 先 SP-26 可行性：锁定 engine 源码证据→最小 release 实验→主/独立窗真实 renderer/GPU 观察与帧耗时 | SP-26 |
| FLD-CFG-063 EnableLog | 无应用 logger 读者（仅 DTO/存储） | restart_app 后关=无应用日志写入、开=轮转/脱敏；journal 恢复仍可用 | SP-25 |
| FLD-CFG-064 RootCertProvider | `platform/src/cert/mod.rs` 映射存在；`subscriptions/download.rs:119` 自建 Client 未读 | 受控合成 CA：mozilla 源接受自签订阅源、system 源拒绝；订阅/更新/WebDAV/路由/Geo 逐 client；不装 OS 证书 | SP-25 |
| FLD-CFG-129 EnableIPv6（mihomo） | `codegen.rs:461-479` 投影存在但无生产调用；`engine.rs:4325-4339` 原文旁路 | Mihomo custom 计划开→YAML `ipv6:true`→真实会话；关→不改自带值 | SP-24 |
| FLD-CFG-130 EnableMixinContent | 同上（`mixin.rs:75 generate_mihomo`） | 开+受控 mixin 文件→merge 生效；关→文件保留且不 merge；坏 YAML 拒绝 | SP-24 |
| FLD-CFG-086 SrsSourceUrl | `dns.rs:241 effective_srs_source` + `engine.rs:4054` 模板生效；`local_srs_files` 无生产填充 | 本地合成源下载 srss→`local_srs_files` 快照→断网仍选本地、不暗访 remote | SP-25 |
| FLD-CFG-087 RouteRulesTemplateSourceUrl | `routing.rs:411-428` 配 URL 即显式 UNAVAILABLE，无异步 fetch | 新异步 job：下载→schema 校验→路由候选提交/刷新 UI；坏内容/中断/取消保留旧规则（需 SP-00 新 API） | SP-25 |

各文件含：原版入口与符号、当前 provider/caller/DTO（文件:行）、版本/错误/
取消/持久化/生效时机、唯一验收命令、未验证前置。SP-23 登记通过不代表任一
功能通过；每叶仍须逐实例完成正式入口/重开/最终消费者证据。

## 3. 发现的缺失消费者/缺口清单（供 SP-24/25/26/27 领取）

- G-01（SP-24）：`enable_happy_eyeballs` 在 `config_codegen` 内零读者
  （`input.rs:562` 声明；`dns.rs:222-266` 两处调用均传 `Some(params)`，
  `set_sockopt_domain_strategy` 只比 `params != default`）。开关关+参数非默认
  仍输出 `happyEyeballs`（审计 pure probe 已复现 off/on JSON 一致）。
- G-02（SP-24）：`settings.rs:93` `parse::<u32>()` 拒绝原版合法 `"1-3"`；
  wire 侧 `config.rs:258-262` 取首整数，二者语义分裂。`option_setting_window.dart:358-364`
  同样整数-only。
- G-03（SP-26）：`apps/desktop/windows` 内 `hwa|HWA|impeller|software render`
  零命中；EnableHWA 仅 DTO/存储/timing。禁 Impeller/低功耗 GPU 不等于软件渲染，
  须先行可行性卡。
- G-04（SP-25）：EnableLog 仅 DTO/存储/timing/edge 用例；无 logger 初始化/轮转/
  禁写读者。不得以内核 LogEnabled（FLD-CFG-026）替代。
- G-05（SP-25）：`subscriptions/download.rs:119` 自建 reqwest Client 未读 provider；
  审计定位 `updater/src/fetch.rs:36`、`updater/src/download.rs:104`、
  `application/src/webdav.rs:110` 同类自建（本卡仅复核 download.rs，其余沿审计行）。
  需共享 HttpPolicy/ClientFactory（SP-00 整合）。
- G-06（SP-24）：`mixin_options_from_app`（`codegen.rs:461`）无非测试调用者；
  `engine.rs:4325-4339` native_custom 分支原文直通。Helper 单测 ≠ 正式 custom 计划 merge。
- G-07（SP-25）：`local_srs_files`（`input.rs:469`）无生产填充者（仅测试插入）；
  `ruleset.rs:82` 消费者空等。下载落地 ≠ 生成器选择本地。
- G-08（SP-00/整合者）：`validate_plan.py:22` 全 `identified` 断言与 manifest
  演进冲突（SP-00/01/02/03/04/11 已 implemented），当前 exit=1。修 validator
  属整合者事项，本卡不动。
- G-09（通用前置）：本批 18 叶均缺“正式 UI→真实 FRB→Rust 保存→独立重开→
  最终消费者”完整证据；CP-SET-01/02/03（保存/重试）与 CP-SET-15（Rust 空/坏整树默认）
  为跨领域前置（SP-01/02/12）。

## 4. 剩余工作

- 180 − 18 = 162 个 ID 未登记（含 2 internal、23 container、137 leaf）。
  容器/内部仅按结构/迁移/引用验收，不造开关；每叶仍须单实例卡。
- 建议下一批：MsgUIItem（065/066）与 ClashUI 轮询组（131-135，CP-SET-03 关联）、
  平台代理组（137-142，内容去重 CP-SET-09）、DNS 组（159-175）。

## 5. `git status --short`（收尾）

见最终消息。要求：除本目录新建 19 文件 + `tasks/SP-23.md` 状态行 +
`execution-manifest.json` SP-23 块 `status` 外无其它改动；生产代码零改动。
