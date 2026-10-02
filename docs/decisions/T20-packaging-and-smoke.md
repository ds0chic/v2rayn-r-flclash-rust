# T20-01 — Windows 打包、版本元信息与干净环境冒烟

- 状态：accepted
- 日期：2026-10-02
- 关联：`docs/evidence/T20.md`、`compat/platform-matrix.md` §6、
  `tools/release/build_windows.ps1`、`tools/release/smoke_windows.ps1`、
  `tools/release/README-platforms.md`

## 背景

T20 需要产出 Windows x64 发布候选（portable zip）、干净环境冒烟、GPL/归属
文档、版本信息，并且全部门禁真实通过。约束：不捆绑外部内核二进制、不触碰
10808/系统代理/TUN、不 commit。

## 决策

1. **打包布局采用 Flutter Release 目录为骨架**。`flutter build windows`
   的 Release 目录已包含 exe、`bridge_api.dll`、插件 DLL、`flutter_windows.dll`
   与 `data/`。我们额外加入工作区自产的 `net_host.exe` 与
   `privileged_helper.exe`（`target/release`），因为它们不在 Flutter 产物中，
   需与主 exe 同级供运行时定位（`net_host_client.rs` 的同级查找）。
   `ui_state.json`（运行期草稿）被显式排除。

2. **不捆绑内核**。Xray/sing-box 由运行时 updater 下载，锁定于
   `tools/cores/cores.lock.json`。包内附 `CORE-NOTES.txt` 说明。

3. **版本信息多来源**：`dist/build-info.json` 记录 pubspec version
   (`1.0.0+1`)、Rust workspace version、git commit/branch/dirty、目标平台/三元组、
   构建时间与工具链版本。UI 侧既有“检查更新”窗口展示应用版本语义；应用内
   展示 commit 的“关于”面板未新增（避免功能代码改动），改由 build-info 与
   README/NOTICE 承载。

4. **干净环境冒烟 = 真实执行**。解压到独立临时目录，`V2RAYN_R_DATA_DIR`
   指向新建临时数据目录，后台启动并记录 PID；验证进程/窗口/首运行数据文件；
   用既有 `V2RAYN_R_OPEN_*` 钩子打开备份/更新窗口并截图；退出后清理自有进程树。
   方法学限制（无 Process Monitor，未做路径追踪）如实登记。

5. **release 构建下真实 apply 的驱动方式**。历史 hook `V2RAYN_R_AUTOSTART`
   仅在 `kDebugMode` 生效（ISSUE-08 的安全修复）。为在 **release** 打包产物上
   验证真实 apply 链，新增窄接口 `applyPlanOnLaunchProvider`
   （默认 `false`），仅在 `main()` 检测到 `V2RAYN_R_AUTO_SMOKE` 时 override 为
   `true`。默认值保证正常 release 运行不会因环境变量自动拉起内核，未回退
   ISSUE-08 的安全结论。

## 影响

- 打包脚本幂等：重建 stage 目录与 zip，覆写 `SHA256SUMS` 与 `build-info.json`。
- zip 的 SHA-256 会因 `Compress-Archive` 写入文件时间戳而在不同次构建间变化；
  因此以“成员内容清单哈希”作为可复现性度量（见 T20 证据）。
- 新增/修改文件集中在 `tools/release/**`、`dist/**`、文档与
  `apps/desktop/lib/main.dart`、`apps/desktop/lib/app/app.dart`（窄接口）。
  未改 Rust 功能代码（仅 `cargo fmt` 格式化既有的两处未格式化文件）。

## 未决/不做

- 代码签名、安装器、自动更新真实执行、升级/卸载：未做。
- Windows ARM64 / macOS / Linux：不支持/未验证。
- GUI 鼠标点击驱动的 apply 时序：未自动化，改用等价环境钩子驱动。
