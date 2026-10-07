# SP-35 24 小时 Soak 协议（准备文档；未经明确授权不得执行）

状态：identified（仅协议文档；soak 未运行；`docs/evidence/stable-port/SP-35/runs/` 尚无数据）。
基线：以 SP-34 固定提交干净重建的候选包为准；历史 ZIP（含 `dist/v2rayN-R-1.0.0+1-windows-x64.zip`，
`git_commit=672e666`/`git_dirty=true`，见 `docs/evidence/stable-port/SP-34/prep-note.md`）**不得复用**。
前置：SP-34 放行条件（`tools/release/build_windows.ps1` 干净重建 → armed=false/dirty=false →
`tools/acceptance/sp30_package_identity.ps1 -Zip <新包>` exit 0 → `tools/gates/run_all.ps1` 门禁）。

> 授权门：本协议任何启动/采样命令在执行前须得到用户明确授权。默认在日常宿主机上**不执行**；
> 真实 OS 副作用仅授权隔离机。授权前只允许 `-DryRun` 类只读探针。

安全包络（硬约束，AGENTS.md）：
- 回环合成 only：订阅夹具只用 `fixtures/acceptance/sp33/synthetic-sub.txt`
  （`192.0.2.0/24` 文档地址，端口 ≥11808）；soak 回环端口先探测、全部 ≥11808。
- 永不触碰 127.0.0.1:10808（用户在用代理端口）；soak 前后各做一次 10808 未占用快照（只 connect 探测，不 bind）。
- 系统代理 / 路由 / TUN / DNS / Run-key 零写入；自启、签名、自动更新真实执行不在 soak 范围。
- 只停本协议启动且持有 PID 记录的进程（app + 其拉起的 net_host/core）；不得按进程名批量终止。
- 隔离数据目录：`V2RAYN_R_DATA_DIR=%TEMP%\sp35_soak_<guid>` 空目录（范式见
  `tools/acceptance/sp30_prepare_datadir.ps1`）；用户真实数据目录零触碰。
- 清理回收：T+24h 后按“拆除”节停止 owned 进程树、确认端口释放、删除隔离目录树（失败则保留现场并记录路径）。

## 启动命令（二选一，按授权时派单；包路径与端口在执行时填实）

路径 A — armed 证据构建（合成 apply 常驻，推荐用于 soak；env 合同与
`tools/release/capture_apply_evidence.ps1` 第 64–78 行一致：`t18b_seed` 播种 →
`V2RAYN_R_DATA_DIR`/`V2RAYN_R_AUTO_SMOKE=1`/`V2RAYN_R_XRAY_BIN`/`V2RAYN_R_NET_HOST`/
`V2RAYN_R_RUN_ROOT`/`V2RAYN_R_PIPE` → 启动包内 `v2rayn_desktop.exe`；就绪判定沿用该脚本
第 83–90 行的 `Get-NetTCPConnection -LocalPort <soak-port> -State Listen` 轮询）：
```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/release/capture_apply_evidence.ps1 -Zip <候选包全路径> -XrayBin tools/cores/xray/v26.3.27/xray.exe -EvidenceDir docs/evidence/stable-port/SP-35/runs/<date>/
```
启动后保持进程常驻 24h（该脚本自带的截图后即停仅用于 T20 取证；soak 时**不执行其拆除段**，
改为本协议的检查点采样与 T+24h 拆除；改动点须在 `launch.json` 中如实记录）。
soak 回环端口：以 `-ProbePorts`（`tools/acceptance/sp33_launch_prep.ps1`）确认空闲的 ≥11808 端口，
全程固定（中途不得换端口；换端口即记一次非计划 core restart）。

路径 B — official 构建（`smoke_armed=false` 普通入口 GUI idle soak；入口合同与
`tools/release/r4_32_real_entry.ps1` 一致：仅设 `V2RAYN_R_DATA_DIR`，无 AUTO_SMOKE、无预置 active）：
```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/release/r4_32_real_entry.ps1 -Zip <候选包全路径> -EvidenceDir docs/evidence/stable-port/SP-35/runs/<date>/
```
（该脚本 `param()` 仅接受 `-Zip`/`-EvidenceDir`/`-WindowWaitSec`；隔离数据目录由脚本自建，
其路径须转记入 `launch.json`。备选：
`tools/release/negative_unarmed.ps1` 的 unarmed 启动段，见该脚本第 89–97 行。）
路径 B 不触发真实 apply，对比的是 GUI idle 基线；journal 无 `stage=applied` 属预期，在
`launch.json` 注明 `apply=none(idle)`。

无论哪条路径，启动前先跑（只读，exit 须 0 方可继续）：
```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp33_launch_prep.ps1 -VerifyFixture -ProbePorts
powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp30_package_identity.ps1 -Zip <候选包全路径>
```

## 采样项与采样时机

| # | 指标 | 来源/方法（只读，不注入被测进程） | 时机 |
|---|---|---|---|
| M1 | 内存 RSS（app / net_host / core 三进程分别记录） | `(Get-Process -Id <pid>).WorkingSet64`（PrivateMemorySize64 同记） | T+0/1h/6h/12h/24h + 异常即采 |
| M2 | 句柄数 / 线程数 | `(Get-Process -Id <pid>).HandleCount` / `.Threads.Count` | 同 M1 |
| M3 | 日志 ring 计数 | journal/ring 计数器 `dropped_lines`/`dropped_bytes`/`accepted`/`rejected`（SP-22 定义；若运行时未暴露则记 `not-exposed` 缺口，不编造） | 同 M1；另每次 checkpoint 记录 core.log 体积 |
| M4 | monitor 轮询延迟 | 沿用 SP-22 实测口径：10k 规模下 5 次 controller poll 耗时（ms）；soak 中做同规模轻量 poll 计时 | T+0/6h/12h/24h |
| M5 | core 重启次数 | owned core PID 存活核对 + journal `stage=applied`（pid/config_sha256/rev）连续性；任何非计划 PID 变更记 1 次 restart 即停表取证 | 每次 checkpoint；变更即采 |
| M6 | 存活/就绪 | `Get-NetTCPConnection -LocalPort <soak-port> -State Listen`（沿用 `capture_apply_evidence.ps1` 第 85 行模式） | 每 checkpoint；失败即 fail |
| M7 | 性能微基准对照 | `dart run tools/perf/synth_node_bench.dart --out <run-dir>`（SP-31 方法，`benchmarks/sp31_method.md`） | T+0 与 T+24h 各一次（环境对照，非被测进程内） |

采样统一追加写入 `<run-dir>/samples.csv`（列：`t_iso,elapsed_h,pid,role,rss_mb,handles,threads,poll_ms,ring_dropped_lines,ring_dropped_bytes,corelog_bytes,core_restarts,journal_rev,note`）；
每次 checkpoint 另存 `checkpoint-T<n>.json`（原始 `Get-Process` 快照 + journal 尾部 50 行哈希）。

## 检查点表

| 检查点 | 动作 | 记录物 |
|---|---|---|
| T+0（启动后就绪） | launch.json 定稿（包 sha/commit/dirty/armed、数据目录、soak 端口、路径 A/B、seed 方式）；M1–M7 全采第一行 | `launch.json`、`samples.csv` 首行、`checkpoint-T0.json`、窗口截图 |
| T+1h | M1/M2/M3/M5/M6；确认三进程存活且 PID 未变 | `checkpoint-T1.json` + samples 行 |
| T+6h | M1–M6 全采；core.log 体积对照 | `checkpoint-T6.json` + samples 行 |
| T+12h | M1–M6 全采；journal rev 连续性核对（rev 只增不跳；跳变即取证） | `checkpoint-T12.json` + samples 行 |
| T+24h | M1–M7 全采；`sp31` 微基准第二次；按“拆除”节 teardown； verdict 判定 | `checkpoint-T24.json`、拆除记录、`verdict.md` |

两次 checkpoint 之间不做任何写操作（不点 UI、不重开、不换端口）；任何偏离先记 `note` 再继续，
属 fail 条件的按“通过/失败”节停表。

## 通过 / 失败判定（引用实测值作参照，不新设数值阈值）

以下均为 SP-31/SP-22 已实测参照值（reference band，不是通过阈值；soak 不做数值门限断言，
长稳 verdict 看**趋势与事件**）：
- SP-31 微基准（30 样本，`benchmarks/sp31_baseline_2026-10-06T17-50-29Z.json`）：
  filter_10k p50 0.73 / p95 1.48 / p99 3.65 ms；sort_10k p50 4.43 / p95 5.73 / p99 12.33 ms；
  logbatch_1k p50 0.20 / p95 0.49 / p99 1.06 ms。对照门槛原文：10k 搜索排序 p95 ≪ 200ms。
- SP-31 扩展（`docs/evidence/stable-port/SP-31/EXTENSION-2026-10-07.md`）：
  pagewalk_10k p95 4.81ms；overlay_10k p95 0.62ms；snapshot p95 1.31ms；logtail p95 42ms
 （Dart 侧 mirror  artifact，生产用 ring + 2000 cap，Rust 实测 50k 洪水 7.3ms）。
- SP-22 真实核 10k（`docs/evidence/stable-port/SP-22/logs/sp22-realcore-10k-2026-10-07.log`，
  12.96s pass）：10k/10k 建连（2.93s，dial fail 0）；5 次 controller poll 217–244ms（10k 存活连接）；
  core 日志洪水 20,008 行 → ring 保留 10,000，`dropped_lines=10,008`/`dropped_bytes=1,055,794`/`rejected=0`；
  mihomo RSS 26.9MB → 432.8MB（10k 连接峰值快照，非泄漏结论）；2k 规模 poll 37–54ms、
  7.70s pass（同目录 `sp22-realcore-rerun-2026-10-07.log`：50/50 回显 6.56s）。
- SP-22 环约束（`docs/evidence/stable-port/SP-22/README.md`）：默认 ring 10k 行/10MiB；
  单 burst 截尾上限 `MAX_LOG_BATCH_LINES=2000`；`accepted == total + dropped_lines` 闭环；
  group 并发上限 `MAX_GROUP_DELAY_CONCURRENCY=8`。

Fail（任一即停表、保留现场、记 fail；不重放非幂等写）：
- F1 进程崩溃/非计划 core restart（M5 计数 ≥1）或 soak 端口监听消失（M6 失败）。
- F2 RSS/句柄跨检查点单调发散且 T+24h 未回落（对照参照：10k 峰值 RSS 432.8MB 是规模快照，
  soak 看 24h 趋势而非绝对值；趋势判定须贴 samples.csv 全序列，不单点断言）。
- F3 ring 计数器不闭环（`accepted != total + dropped_lines`）或 journal rev 跳变/半写。
- F4 monitor poll 延迟相对 T+0 同规模 regress 一个数量级以上（参照：10k 下 217–244ms；
  只做同条件对照，不跨规模比较）。
- F5 任何安全包络触碰（10808 被占用/修改、系统代理/路由/TUN/DNS/Run-key 写入、非 owned 进程被停）。
- F6 隔离目录/证据写入用户真实数据目录。

Pass（须同时满足）：24h 内无 F1–F6；三进程 PID 全程不变；journal rev 单调连续；
M7 两次微基准落在 SP-31 参照同一量级（贴原始 JSON，不 means-pluck）；拆除后无残留
（owned PID 全清、端口释放、隔离目录已删或已登记保留路径）。verdict 只写 pass/fail + 证据路径，
不写数值门限、不借用其它卡证据。

## 证据布局

```
docs/evidence/stable-port/SP-35/runs/<date>/   # <date> 形如 2026-10-08（UTC 日）
  launch.json            # 包 sha/commit/dirty/armed、基线、数据目录、soak 端口、路径 A/B、启动命令原文、授权记录
  samples.csv            # 全序列采样（表头见“采样”节）
  checkpoint-T0.json checkpoint-T1.json checkpoint-T6.json checkpoint-T12.json checkpoint-T24.json
  window-T0.png window-T24.png
  journal-tail-sha256.txt  # journal/artifact 拷贝的哈希清单（不含任何秘密；订阅/凭据永不落盘）
  cleanup.json           # owned PID 清理、端口释放、目录删除确认
  verdict.md             # pass/fail + 条件编号（F1–F6）+ 未验证项
```

`launch.json` 须包含 `authorization: {by, at_iso, scope}`（用户明确授权记录），缺失则 verdict 无效。

## 拆除（T+24h 或 fail 停表时）

1. 停止 owned 进程树（仅 launch.json 记录的 PID；先 app，后 net_host/core；`Stop-Process -Id <owned>`，
   不得按名批量停）。
2. 确认：`Get-Process -Id <pid>` 全部不存在；`Get-NetTCPConnection -LocalPort <soak-port>` 无 Listen；
   10808 快照仍干净。
3. 删除隔离目录树 `%TEMP%\sp35_soak_<guid>`（fail 保留现场时跳过删除并在 cleanup.json 登记路径）。
4. 写 `verdict.md` + `cleanup.json`；本协议文档与任务卡状态由整合者按证据更新（本文件不改卡）。
