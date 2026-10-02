# 终审审计报告 — Muse Spark 1.3

- 时间：2026-10-02（UTC+8）；审计者：Muse Spark 1.3（主审计，亲自复核）
- HEAD：`5c33fa5f08bc063e938c0c339d2a96bf01662313`；工作区状态：干净，仅未跟踪 `docs/evidence/audit/FINAL.audit.prompt.md`
- 子代理：5 路一层子代理（A1 发布物 / A2-A3 T20 冒烟与 gating / B 反造假 12 项 / C+D 安全秘密 / E 台账终态），结论均经主审计亲自抽查复核（下文 ★ 为亲自验证项）
- 只读边界：未修改任何项目文件，未运行 flutter build/test、未运行全量 workspace 测试；轻量验证仅读文件、`git show/grep`、`Get-FileHash`、zip 只读清单

## 结论

**有条件可发布（仅 Windows x64 portable 冒烟包 RC 口径；正式对外发布不可）。**

定论：包内洁净（无内核、无 10808 绑定、无秘密泄漏、无造假冒充），门禁日志齐全且诚实登记了全部已知未决项；但 `build-info.json` 记录的不是 HEAD 的干净构建、release 下环境变量可直接拉起内核（ISSUE-08 同模式回归，高风险）、决定性 apply 证据存在自洽缺口（journal `finalized`+summary `applied=false` vs 正文 `applied` 宣称、`core.log` 未归档）。上述三项必改后可作为受控 RC；签名/自替换实跑/远端 TLS/系统代理-TUN 真写四项 P0/P1 关闭前不得正式发布。

## 必查项结论表（A1..F）

| 项 | 结论 | 一句话依据 |
|---|---|---|
| A1 发布真实性与洁净度 | 部分通过（1 缺陷） | ★ zip 27 成员齐全、无内核，SHA256 复算一致；但 `build-info.json` commit=`54fcebe`+dirty ≠ HEAD=`5c33fa5`（F-01） |
| A2 T20 冒烟证据链 | 未闭环（弱链） | ★ journal `stage=finalized,pid=null`、summary `seed_apply.applied=false/port11808=false` 与 T20.md §2.2 `stage=applied pid=41560` 矛盾；`core.log` 未归档（F-02/F-03） |
| A3 版本/build-info/可复现 | 基本诚实（1 注记） | 版本号自洽，不可字节复现已诚实登记并给成员清单哈希；来源新鲜度为唯一缺陷（F-01） |
| B 反造假抽查 12 项 | 可信 8 / 证据弱 4 / 造假 0 | 代码+测试断言方向真实；弱项均为"缺机器产物/截图占位"，非虚构功能（详见台账节） |
| C 宿主安全 | 通过（附条件） | ★ 无 10808 绑定/连接；WinINET/注册表/路由/TUN/杀进程能力均编译存在、测试零调用，与 fake 声明一致；T20 未触碰宿主代理 |
| D 秘密与合规 | 通过 | ★ 仅协议前缀分支与 `example.com`/合成串；NOTICE/GPL/不捆绑声明与包内容一致 |
| E 台账终态 | 口径干净，有精度瑕疵 | ★ `verified`=0 全仓零命中；`implemented` 抽查 8/10 实存，2 条 location 缺位（CFG-002、fields 源分区） |
| F 总判 | 有条件 RC，正式发布不可 | P0×2（签名、自替换实跑）+ 必改×3（F-01/F-02 门控、F-04 ISSUE-08 门） |

## 发现清单（编号 | P0/P1/P2 | 类型 | 文件:行/证据 | 反证 | 最小修复）

- F-01 | P0 | 发布来源不一致 | ★ `dist/build-info.json:9-10` `git_commit=54fcebe…+git_dirty:true` ≠ HEAD `5c33fa5`；★ `git diff 54fcebe..5c33fa5 --name-only` 含功能源码（`main.dart`、`app.dart`、`engine.rs`、`t18b_seed.rs`）；T20.md:12 自认基线为 54fcebe 脏树 | 构建时间 02:29:52Z 距 T20 提交仅 ~2min，内容很可能同树，但只读无法证明二进制==HEAD | 在 HEAD 干净树重跑 `tools/release/build_windows.ps1`，更新 `build-info.json`/`SHA256SUMS`/zip 后重新提交 |
- F-02 | P0 | 证据自洽缺口（决定性 apply） | ★ `T20-applied-artifacts/s-…-2_journal.json`：`stage=finalized,pid=null,port=11808,rev=2,config_sha256=3c168c4d…`；★ `T20-smoke-summary.json` seed_apply：`applied=false,port_11808_listening=false,has_core_descendant=false`；对比 `T20.md:155-156` 声称 `stage=applied pid=41560` | `apply_evidence.log` 有 `state=running pid=42868`，config artifact 为真实 mixed 入站 11808+VLESS 出站；T20.md:128-130/159-162 已诚实登记探测节流偏差并保留双日志 | 二选一：(a) 归档 300ms 间隔重测的 journal（`stage=applied`）+ `core.log`，替换矛盾工件；(b) 若重测不可行，正文降级为"弱链：config 生成真实，applied 运行态单次探测"，summary 以 capture 脚本为准 |
- F-03 | P1 | 引用的 `core.log` 未归档 | ★ `T20.runs/` 仅 10 个文件（`apply_evidence.log` 304B 等），无 `core.log`；T20.md:153-154 引用 `core.log: Reading config → Xray 26.3.27 started` | 运行态截图 `T20-smoke-applied-running.png`（74KB）存在 | 将 capture 运行的 `core.log`（脱敏后）补入 `T20.runs/`，或删除对未归档文件的引用 |
- F-04 | P0 | ISSUE-08 同模式回归（高） | ★ `main.dart:16-19` `applyOnLaunch` 无条件读 `V2RAYN_R_AUTO_SMOKE`，无 `kDebugMode`/编译期守卫，release 生效；★ `main_shell.dart:120-123` 第二条独立路径直接 `applyActive()`，绕过 provider；★ `app.dart:90-95` 注释自称"未回退 ISSUE-08"错误（AUTOSTART 修复恰是 `kDebugMode` 门，新钩子无此门） | 触发需"环境变量+已有数据目录含 active 节点+有效计划"三条件；`engine.rs:689-704` 空目标走结构化错误无硬编码回退；apply 不写系统代理/不碰 10808 | 两处同时加编译期武装位：`bool.fromEnvironment('V2RAYN_R_SMOKE_ARMED', defaultValue:false)` 守卫，冒烟构建加 `--dart-define=V2RAYN_R_SMOKE_ARMED=true`；正式 release 默认忽略该变量（`T18Bench` 同理） |
- F-05 | P1 | T20 用 Xray 非包内（声明一致但易误读） | ★ summary seed_apply env：`V2RAYN_R_XRAY_BIN=…\tools\cores\xray\v26.3.27\xray.exe`；★ zip 清单 27 成员无 xray/sing-box/geo/cores | `cores_policy=not_bundled` + `CORE-NOTES.txt` + T20.md §1/§6 均已声明不捆绑，证据链自洽 | README 发布页加一句话："RC 冒烟用的 Xray 来自开发者本机 `tools/cores`，用户侧运行时下载；包内无内核"（防"打包内 Xray"误读） |
- F-06 | P1 | 历史截图大量 1 字节占位 | 子代理抽查：`screenshots/windows_flutter/` 下 T01~T15a 期 `t11_routing.png/t11_dns.png/t12a_*.png/t13_*.png/t15a_*.png` 等 `Length=1`；T16/T20 为真实尺寸（70~90KB） | T20 已回补 4 张真实截图；各 T*.md 对未验证项均有文字诚实登记 | 接受现状；后续审计以 `*.runs/` 日志+`results*.json`+断言方向为主要依据，截图降权（本报告 B 项已按此口径） |
- F-07 | P2 | 台账精度：CFG-002 空 location 却标 implemented | `compat/` CFG-002 条目 `implementation_location/test` 为空，仅上游 evidence | 功能缺失不成立（配置类型枚举代码存在），系台账回填遗漏 | 补 location/test 或降级为 identified |
- F-08 | P2 | 台账精度：fields 源分区 180 条 `impl=None/tests=None` | ★ `fields.settings.yaml` 源分区抽查代表为空；聚合 `fields.yaml` 侧有 location | 同上，口径未同步，非功能缺失 | 声明"以 `fields.yaml` 聚合条目为准"或逐条回填 |
- F-09 | P2 | 台账 `unresolved/contradicted_by_source` 非法 status 残留 | `compat/codegen-map.singbox.yaml` 6 条 unresolved、`codegen-map.xray.yaml` 4+1 | 非 SCHEMA 合法值，未冒充完成态 | 走台账降级/裁决（kcp-legacy 等），不占用发布门禁 |
- F-10 | P1 | T17 输入登记文档过时（代码已修、文档未更新） | `T17-inputs.md:22-23` 仍写 helper"词法而非规范化/仅 u64"，而 `helper.rs validate_elevated_core_canonical`+`configure_job_kill_on_close`+`pid+created_at_ms` 回归已修 | 文档滞后，非代码缺失 | 更新 T17-inputs §1 为 canonical 口径，或标注"已修、待文档同步" |

未升级项（诚实登记、维持原判）：更新签名缺失（见阻断 B1）、自替换/安装器未实跑（B2）、远端 TLS 未验（B3）、系统代理/自启真写未验（B4）、TUN 真实会话未验（B5）、Windows-only（B6）、flutter_tester 逐文件串行口径（B7）、Engine 单例测试持锁纪律（B8）、订阅有端点时代理失败回退直连（B9，部分修）、备份明文（B10）、复制后缀不一致（B11）。

## 台账终态统计与抽样核对

★ 亲自复核：`git grep -n "status: verified" -- compat/` 零命中；`implemented` 计数 features 105 / actions 90 / entities 152 / settings 180 / fields.yaml 81。

| status | 全文件 N=1214 | 说明 |
|---|---|---|
| identified | 584（48.1%） | 含 `layouts.yaml` 92/92 identified（与其头注一致） |
| implemented | 608（50.1%） | 台账最高宣称；`verified` 全仓为 0，口径干净 |
| preserved_only | 11（0.9%） | F-SUB-007 等口径正确 |
| unresolved / contradicted_by_source | 11 | 仅 codegen-map，非 SCHEMA 合法值，待裁决（F-09） |
| verified / blocked / not_applicable | 0 | yaml 内为 0；`blocked` 仅出现在 platform-matrix prose（ARM64） |

严格口径（features+actions+layouts+fields.yaml 去双计）：N=846，identified 559（66.1%）、implemented 276（32.6%）、preserved_only 11（1.3%）。`fields.yaml` 头注已声明为合并物，全文件 N 仅作过程量，不得作为完成率分母。

B 反造假 12 项抽样（子代理结论，主审计抽查了 #2/#3/#6 链路对应代码行，主审计另行全覆盖了端口/签名/NOTICE 点）：

| # | 项 | 判定 |
|---|---|---|
| 1 订阅先删后写+事务 | 证据弱 | `store_repo.rs:62-113` 事务结构真实；`subs_pipeline.rs:206-269` preserve 断言方向正确；缺机器产物归档，中段注入回滚测试未定位 |
| 2 策略组/链/模板内核校验 | 可信 | `T06b.runs/results.json` 36 例 35PASS/1FAIL（kcp-legacy 上游间隙诚实归因）；`T10.runs/results.sanitized.json` 逐例 `exit:0` |
| 3 路由真实校验 | 可信 | 28/28 内核 check + 4 个真实踩坑修复记录；截图占位不计 |
| 4 DNS 真实校验 | 证据弱→偏可信 | 内核 check 链真实；独立截图占位，无 DNS 流量差分对等项 |
| 5 设置 180 字段存储 | 证据弱 | 38 用例链真实（往返/冲突/原子写）；无产物归档、截图占位 |
| 6 系统代理编排 fake | 可信（声明属实） | 代码即 fake + 8 项真写明确"未验证"；零冒充 |
| 7 TUN dry-run | 可信（dry-run 属实） | 75 用例 + 真二进制 `--dry-run-tun` 冒烟；真实 TUN 明确未执行 |
| 8 监控/日志/Clash | 证据弱 | loopback+单元真实；"真实 Xray 冒烟"依赖 `#[ignore]` 需手动指定用例 |
| 9 测速真实本地链路 | 可信 | 真实 Xray socks→freedom + tiny_http，失败返回 -1 不伪造；UDP 不支持诚实登记 |
| 10 备份 loopback | 可信 | roundtrip/篡改拒绝断言方向正确；87KB 真实窗口截图 |
| 11 更新 loopback | 可信 | loopback≥11808 + 摘要不符不落盘；应用自更新仅构造 spec 不启动进程（诚实） |
| 12 T18b 真实计划 11/11 + e2e 7/7 | 可信 | `T18b.runs/` 三份 log 齐全；远端 TLS/在线重配/GUI/macOS 未验已明示 |

造假/冒充：0 项。所有未验证处（系统代理真机写入、TUN 真实会话、mihomo WS、远端 TLS、签名/安装器、ARM64/macOS/Linux）均在对应 T*.md 中明示未验证。

## 发布阻断项与建议

| # | 项 | 严重度 | 最小处置 |
|---|---|---|---|
| B1 | 更新签名缺失（`signature.rs:35 UnsupportedSignatureVerifier`，无公钥/未接 `.dgst`） | P0 | 选定 GPG/minisign、内置可信公钥、资产校验实跑通过前，禁止对外发布/自动更新 |
| B2 | 外部自替换/安装器/升级卸载未实跑（跨卷 rename 未实现） | P0 | 隔离机自替换实跑+跨卷路径后方可发布；当前仅 portable zip 冒烟包口径 |
| F-01 | `build-info.json` 非 HEAD 干净构建 | P0（RC 门） | HEAD 干净重建并更新三件套 |
| F-04 | ISSUE-08 回归（release 环境变量拉核） | P0（RC 门） | 两处编译期武装位（`main.dart` + `main_shell.dart`） |
| F-02/F-03 | apply 决定性证据自洽缺口 + `core.log` 缺失 | P0（RC 门） | 补归档 applied journal + `core.log`，或正文降级为弱链 |
| B3 | 真实远端/TLS 未验（e2e 全 loopback） | P1 | 补受控远端+TLS 握手证据，或发布说明明确"仅 loopback 验证" |
| B4 | 系统代理/自启真写未验 | P1 | 隔离环境+用户授权下首验 WinINET/HKCU/Run 回读；非 Windows 维持 unverified |
| B5 | TUN/提权 helper 真实会话未验 | P1 | 隔离提权实测路由/适配器/租约/回滚；UI 文案保持"未验证" |
| B6 | Windows-only | P1 | 发布页只列 Win x64；其余标注未构建未验证 |
| B7/B8 | flutter_tester 逐文件串行口径 / Engine 测试持锁纪律 | P1（测试域） | 维持现有门禁约定；新增引擎测试必须持 `engine_test_lock` |

## 盲区

1. 未重跑任何构建/测试，门禁结论依赖 `T20.runs/*.log`（`cargo_test.log` 74KB、`flutter_test.log` 70KB 仅抽查了存在性，未逐例复验）。
2. 未用 Process Monitor，不能证明包无开发路径依赖（T20.md §2.3 已如实登记）。
3. 图片证据仅核对文件大小与清单，未做像素级内容审查（`T20-smoke-applied-running.png` 74KB 等）。
4. `tools/cores` 本机 Xray 与 `cores.lock.json` pin 的一致性未逐字节核对。
5. 子代理结论经主审计 ★ 12 项亲自抽查复核，但子代理读取的大量证据正文细节仍为抽信级别。
