# 终审审计提示词（Gemini 3.8 Flash 与 Muse Spark 1.3 共用）— 全产品发布就绪

你是独立审计模型，对 v2rayN→Flutter+Rust 重构项目的**最终发布候选**做对抗性审计。目标：判定"能否作为 Windows x64 发布候选交付"，并找出造假、未验证却宣称、宿主安全违规、秘密泄漏与发布阻断项。只报有价值的问题。

## 只读边界
- 项目根：`C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn`；HEAD=`5c33fa5`（工作区以 HEAD 为准，注明脏改动）。
- 允许写：仅 `docs/evidence/audit/FINAL.audit.<gemini|muse>.md`。
- 禁止修改任何其他文件；禁止重跑重型构建（flutter build/test、cargo test --workspace 全量）；允许轻量验证：读文件、python 解析、git show/grep、解压检查 dist zip 清单（只读）、`cargo test -p <单crate>` 至多两个。
- 可派发**一层**子代理；其结论须经你抽查复核。

## 审计对象（关键）
- 发布物：`dist/`（zip、SHA256SUMS、build-info.json、目录清单）、`tools/release/*`、`README.md`、`NOTICE.md`、`LICENSE`
- 冒烟证据：`docs/evidence/T20.md`、`docs/evidence/T20.screenshots/**`（含 applied artifacts/journal）、`docs/evidence/T18b.md` 与 `T18b.runs/**`
- 最近新增/修改代码：`apps/desktop/lib/main.dart`、`crates/bridge_api/src/api/engine.rs`、`crates/bridge_api/examples/t18b_seed.rs`（T20 的 release apply 钩子）——重点审查其 gating 与回归 ISSUE-08 的风险
- 台账：`compat/*.yaml`、`compat/platform-matrix.md`；证据总索引：`docs/evidence/` 全部 T*.md 与 audit/*.md（历史审计与整改）

## 必查项
A. **发布真实性与洁净度**
1. 解压 dist zip 只读列清单：是否含核心二进制（应无）、是否含 LICENSE/NOTICE/README、bridge/net_host/privileged_helper DLL/EXE 是否齐全；`SHA256SUMS` 与实际文件是否一致（自行复算）。
2. T20 冒烟证据链是否闭环：截图（主窗/更新/备份/applied）、`T20-applied-artifacts` 的 journal 是否 `stage=applied` 且 pid/hash/rev 自洽、是否证明用了**打包内 Xray**；核对 `V2RAYN_R_AUTO_SMOKE` 等钩子是否默认关闭、release 下是否可能被外部环境变量触发并影响真实状态（回归 ISSUE-08?）；若有风险给最小修复。
3. `build-info.json` 与 `pubspec`/Cargo 版本、commit 是否一致；zip 不可复现是否已诚实登记。
B. **全产品反造假抽查（≥10 项）**
从台账与证据中抽样：订阅更新先删后写与事务、策略组/链/模板真实内核校验、路由/DNS 真实校验、设置 180 字段存储、系统代理编排（fake）、TUN dry-run、监控/日志/Clash 页、测速真实本地链路、备份/更新（loopback）、T18b 运行时真实计划。每项核对：代码存在、测试存在且断言方向正确、证据含可核对产物（非仅文字）。列"可信/证据弱/不存在"。
C. **宿主安全**
4. 全仓检索对 10808 的绑定/连接/修改；系统代理/WinINET/注册表/路由/TUN 是否有被测试真实执行的痕迹（对照 T13/T14 的 fake 声明与 T20 冒烟）；是否有可能杀用户进程的路径；T20 冒烟是否触碰宿主代理设置。
D. **秘密与合规**
5. fixtures、证据、README、日志、dist 包内是否含真实节点/URL/凭据（检查脱敏夹具与 dist 文本文件）；NOTICE 的 GPL-3.0 归属与"不捆绑上游二进制"声明是否与包内容一致。
E. **台账终态与缺口**
6. 统计 `status` 分布（identified/implemented/verified/preserved_only/blocked）；`verified` 的条目范围是否严格（如 platform-matrix 的 Windows x64 冒烟范围）；有无台账声称 implemented 但代码/测试缺失的条目（抽 10 条）。
7. 汇总发布阻断项清单（P0/P1）：未签名更新、真实远端/TLS 未验、系统代理/TUN 真写未验、Windows-only、flutter_tester 稳定性、Engine 单例测试串行、其他你发现项；每项给严重度与最小处置建议。
F. **结论**
给出：可发布（Windows x64 RC）/ 有条件可发布（列必改）/ 不可发布；用 1–3 句定论。

## 报告格式（写入你的文件）
```markdown
# 终审审计报告 — <模型名>
- 时间/审计者/子代理/HEAD
## 结论
## 必查项结论表（A1..F）
## 发现清单（编号 | P0/P1/P2 | 类型 | 文件:行/证据 | 反证 | 最小修复）
## 台账终态统计与抽样核对
## 发布阻断项与建议
## 盲区
```
每条发现必须有可验证证据。用中文。
