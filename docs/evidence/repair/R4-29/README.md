# R4-29 — 本项目发行与升级（自身更新）

状态：`implemented`（自有发行源链路合成全链已验证；真实远端发行源未配置，
按产品决策 `blocked`，未拿上游 v2rayN 包冒充、未绕过验签）。

- 仓库 HEAD：`06f61f697415e3dda108b2c390c7b188cb71ba6f`
- armed=false；未联网下载真实发行资产；未真实安装/卸载/自替换；未重启宿主。
- 端口：测试仅用 OS 分配的 127.0.0.1 临时端口（tiny_http `:0`），从不使用
  10808；未改宿主系统代理/注册表/路由/TUN/自启。

## 本次唯一用户流程

检查本项目包（合成 release 元数据+资产）→ PGP 验签 → 退出交接外部 runner
（stage→替换→`.previous` 回滚→重启命令构造）或拒绝并保留现状。

## 前置（不回退）

RR-04（Dart 交接 runner/exit）与 FIX-12B（显式安装根、布局、
`v2rayN-upgrade.exe`、PGP `enforce_detached_signature`、拒 WPF 包）已实现，
本卡只在原处加固并补测，未改动其语义。

## 改动

| 文件 | 改动 |
|---|---|
| `apps/desktop/lib/features/update/update_controller.dart` | 新增 `_bridgeCall` 统一 catch：桥接抛异常时清零 busy 并报错，不伪造成功/退出（D23）。覆盖 `applyAppUpdate`/`checkOnly`/`checkAndApply`/`installCores`。 |
| `apps/desktop/test/r4_29_contract_test.dart` | 新增：未配置源显式 blocked（R3-08 不回退）、成功交接 CLI（plan/result/pid/restart-exe/restart-cwd）、验签失败/缺签/摘要不符/缺 helper 拒绝且不退出、抛异常不 latch busy。 |
| `apps/desktop/test/repair/r4_29_repro_test.dart` | 新增复现：抛异常须完成且 busy=false（修复前失败）。 |
| `crates/updater/tests/r4_29_release_chain.rs` | 新增合成全链：loopback 下载 → 合成 PGP 信任根验签（含篡改/错签拒绝）→ 解包 stage → flat overlay（保留 `app.previous` + 重启命令构造）→ rollback；stub runner 不执行。 |
| `tools/release/recheck_r4_29_release_asserts.ps1` | 新增合成 stage 发布断言：stage/zip 名与 pubspec 版本一致、两处 build-info 版本一致、runner 名与 `DEFAULT_RUNNER_NAME` 一致、扁平 exe 入包。 |
| `docs/repair/tasks/R4-29.md` | 状态更新与结果登记。 |

未改任何 FRB 生成文件、`main_shell/app`、profiles/subs/runtime/settings/
routing/monitor/backup 域、`engine.rs` 等禁改文件。

## 命令与结果

见 `commands.log`。关键：

- `flutter analyze` → No issues found
- `flutter test test/r4_29_contract_test.dart test/repair/r4_29_repro_test.dart` → 10 passed
- `flutter test test/t16_update_test.dart` → 7 passed；`r3_08` → 1 passed；`recheck_rr04` → 4 passed
- `cargo test -p updater --test r4_29_release_chain --locked` → 1 passed
- `cargo test -p updater --locked` / `cargo test -p application --test t16_update --locked` → 全通过
- `cargo test --workspace --locked` → exit 0
- `pwsh tools/release/recheck_r4_29_release_asserts.ps1` → `R4_29_RELEASE_ASSERTS ok=true`
- `flutter build windows --release` → Built `v2rayn_desktop.exe`

## 复现证据（先失败后通过）

`test/repair/r4_29_repro_test.dart` 在修复前失败（`update_controller.dart:393`
未捕获 spec 抛出的异常、busy latch）；修复后同文件全绿。原始失败输出见
`repro-before-fix.log`。

## blocked / 接口缺口

1. **真实发行源（产品决策 blocked）**：`UpdateService.app_repo` 默认 `None`，
   `check_app_update` → `error.update_app_source_unconfigured`。未伪造 GitHub
   端点。配置 `with_app_repo`/`V2RAYN_R_APP_REPO` + 自有公钥资产后即可接入，
   无需改布局/验签。
2. **接口缺口：`t16_apply_app_update_spec` 无参数**。窗口的
   `prerelease`/`viaProxy` 未随调用传入（当前依赖上一次
   `t16_check_updates` 的 `remember_update_flags`）。提供方：bridge_api t16；
   调用方：`update_controller.applyAppUpdate`；期望输入 `(prerelease, via_proxy)`；
   生效点：spec 的 check/stage。修复需重生成 FRB（本卡禁止），登记由整合者处理。
3. **接口缺口：`ExternalSpecDto` 不含重启命令/回滚入口**。Rust 侧
   `app_restart_command()` 已构造重启命令，`rollback_app_upgrade()` 可用，但 UI
   未暴露回滚按钮，DTO 未携带重启目标；需要新增 FRB 函数+DTO 后由根代理重生成。
4. **workspace 门禁被并行子代理干扰**：`cargo fmt --all -- --check` 与
   `cargo clippy --workspace ... -D warnings` 仅因另一并行卡（R4-34）正在编辑的
   `crates/application/src/engine.rs`（禁改文件）报差异/5 个 lint 而失败；本卡
   `-p updater` 的 fmt/clippy 全绿。待其收尾后重跑 workspace 门禁。
