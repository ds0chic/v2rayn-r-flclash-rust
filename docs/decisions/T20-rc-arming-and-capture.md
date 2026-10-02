# T20-02 — RC 冒烟编译期武装位与决定性 apply 捕获

- 状态：accepted
- 日期：2026-10-02
- 关联：`docs/evidence/T20.md` §2.2/§5.2/§9、
  `docs/evidence/audit/FINAL.audit.muse.md`（F-01/F-02/F-03/F-04/F-05）、
  `tools/release/build_windows.ps1`、`tools/release/capture_rc_apply.ps1`、
  `tools/release/negative_unarmed.ps1`

## 背景

终审指出：release 构建下 `V2RAYN_R_AUTO_SMOKE` 可无条件拉起内核（F-04/ISSUE-08），
apply 决定性证据存在自洽缺口（运行中 `stage=applied` journal 与 `core.log` 未
归档，F-02/F-03），正式包来源非 HEAD 干净构建（F-01）。需要在不放松安全边界
的前提下既能发布不响应环境变量的正式包，又能对同一源码产出可验证的冒烟包。

## 决策

1. **编译期武装位**：所有可变更运行状态的 automation 钩子以
   `bool.fromEnvironment('V2RAYN_R_SMOKE_ARMED', defaultValue:false)` 守卫
   （`main.dart`、`main_shell.dart`、`t18_bench.dart`）。正式包默认 false，
   忽略 `V2RAYN_R_AUTO_SMOKE`/`V2RAYN_R_T18_BENCH`；冒烟包显式
   `--dart-define=V2RAYN_R_SMOKE_ARMED=true`。运行期环境变量仅在编译期武装后
   才有效——避免 release 二进制被环境变量拉核。

2. **产物隔离**：`build_windows.ps1 -SmokeArmed` 将 stage/zip/build-info/
   SHA256SUMS 输出到 `dist/evidence-armed/`，绝不覆盖正式 `dist/` 包。

3. **运行中捕获**：`capture_rc_apply.ps1` 必须观测到 11808 监听且 journal
   `stage=applied`（并稳定复检）后，在**停止前**拷贝 journal/config/core.log 与
   进程 PID/创建时间；停止后再抓 `finalized` journal 作对照，验证明文
   config/core.log 已删除。旧的事后探测记录不作为决定性证据。

4. **负向门禁**：`negative_unarmed.ps1` 用正式包 + 全套 automation 环境变量运行，
   断言无内核子进程、11808 不监听、无 applied journal、无 core.log/bench 产物。
   注记：`net_host`（自身状态宿主，非内核）在所有运行都会由 `getSnapshot`
   自动启动，不计入该违约判定。

## 影响

- 正式包 `build-info.json` 的 `smoke_armed=false`；arm 包为 true（并因工作树
  含本次工具改动而 `git_dirty=true`，属证据包预期）。
- `capture_apply_evidence.ps1`（旧）保留，但决定性证据改用
  `capture_rc_apply.ps1`。
- 未改功能代码；`dart format`/`flutter analyze` 通过。

## 未决/不做

- 签名、自替换/安装器实跑、远端 TLS、系统代理/自启真写、TUN 真实会话、非
  Windows 平台：仍为未验证/未构建，状态不变。
