# T21 — 安装器与外部自更新实跑证据

状态：`verified`（Windows 11 25H2 x64；Inno Setup 6.7.3；Rust 1.98.1）
范围：`tools/release/v2rayn-r.iss`、`tools/release/install_test.ps1`、
`tools/release/selfupdate_test.ps1`、`services/upgrade_runner/**`、
`crates/updater/src/install.rs`（最小外部升级缺口）、
`crates/application/src/update_service.rs`（仅测试基址覆盖）。
证据目录：`docs/evidence/T21-install-update.runs/`。
未 commit。

## 1. 安装器（Inno Setup）

### 产物

| 项 | 值 |
|---|---|
| 编译器 | Inno Setup 6.7.3（`winget install --id JRSoftware.InnoSetup -e --silent --accept-*` 成功） |
| ISCC | `%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe` |
| 脚本 | `tools/release/v2rayn-r.iss` |
| 输出 | `dist/v2rayN-R-1.0.0+1-windows-x64-setup.exe` |
| 大小 | 16,143,523 字节 |
| SHA-256 | `da87ac4c2d73d75df479f1a234a816be6f1da4f7a246c0f47a7a67fcff87e51f` |
| 版本 | `1.0.0+1`（`AppVersion`），`VersionInfoVersion=1.0.0.1` |
| AppId | `{7A1E4C2D-9B3F-4E6A-8D2C-5F0B7A9E1C34}` |
| 权限 | `PrivilegesRequired=lowest`（每用户安装；`/DIR` 可覆盖） |

打包来源：`dist/v2rayN-R-1.0.0+1-windows-x64/`（portable 布局，整目录
`recursesubdirs createallsubdirs` 逐字节保留）。
`LicenseFile=LICENSE`、`InfoBeforeFile=NOTICE.md`（GPL-3.0 归属展示）。
快捷方式为可选任务：`startmenuicon`（默认勾选）、`desktopicon`（默认不勾选）。
静默契约：`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=<dir> /MERGETASKS=...`。

### 实测：安装 → 启动 → 卸载

命令：
```
installer.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=<temp\app>
              /MERGETASKS="startmenuicon,desktopicon"
```

| 校验 | 结果 |
|---|---|
| 安装退出码 | 0（约 2.1s） |
| 文件布局 | 14/14 期望文件存在（`v2rayn_desktop.exe`、`bridge_api.dll`、`net_host.exe`、`privileged_helper.exe`、`LICENSE`、`NOTICE.md`、`data\app.so`…）；缺失 0 |
| 开始菜单快捷方式 | `%APPDATA%\...\Start Menu\Programs\v2rayN-R\v2rayN-R.lnk` 存在 |
| 桌面快捷方式 | `%USERPROFILE%\Desktop\v2rayN-R.lnk` 存在 |
| 卸载注册项 | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{7A1E...}_is1`，`DisplayVersion=1.0.0+1` |
| 启动安装后的 exe | PID 记录；窗口出现（≤90s）；数据目录生成 `guiNDB.db`（+wal/shm） |
| 卸载退出码 | 0；后台自拷贝进程完成后轮询 |
| 卸载清理 | 安装目录已删除；两个快捷方式已删除；Uninstall 注册项已删除；残留文件 0 |

`guiNConfig.json` 未生成属预期：该文件仅在用户保存设置时写入
（见 `docs/evidence/T20.md`），故只记录、不要求。首启数据库为 `guiNDB.db`。

证据文件：
- `T21-install-update.runs/install-20261002T050303Z.json`（成功，含 ISCC 编译）
- `T21-install-update.runs/install-20261002T050108Z.json`（成功，幂等复跑）
- `T21-install-update.runs/install-20261002T045123Z.json`（成功，首跑）
- `T21-install-update.runs/install-20261002T044946Z.json`（诚实记录：首版脚本把
  首启数据文件名误写为 `guiNNDB.db`，误判为失败；已修正，保留原日志）
- 对应 `.log`

## 2. 外部自更新实跑

### 组成

`services/upgrade_runner`（新二进制 crate，已加入根 `Cargo.toml` members）：
等待目标 PID 退出（Windows `WaitForSingleObject`，有超时）→ 读取
`InstallPlan` JSON → `updater::apply_atomic` 原子替换并保留旧版本为
`<root>/<keep_name>` → 失败时 `updater::restore_previous` 回滚 →
以 `DETACHED_PROCESS|CREATE_NEW_PROCESS_GROUP` 重启新 exe → 写结果 JSON。

`crates/updater/src/install.rs` 增量（最小缺口）：
- `InstallPlan` 增加 `Serialize/Deserialize`（供外部进程加载同一计划）；
- 新增 `pub fn restore_previous(&InstallPlan) -> Result<bool, UpdateError>`
  （current 缺失且 keep 存在时回滚）。

`crates/application/src/update_service.rs` 增量（仅测试覆盖）：
- `pub const GITHUB_API_BASE`（生产默认 GitHub）；
- `test_api_base_override()` 从环境变量 `V2RAYN_R_UPDATE_API_BASE` 读取，
  **仅 `#[cfg(debug_assertions)]` 生效**，release 构建编译期返回 `None`；
- `UpdateService::with_api_base()` 显式覆盖；
- 仅接受 `http(s)://` 前缀；摘要/签名校验不放宽；生产默认端点不变。

### 成功用例

流程：临时根 A 放旧包（`version.txt=1.0.0`、`v2rayn_desktop.exe`、
`bridge_api.dll`）→ loopback HTTP（OS 分配临时端口，绝不为 10808/11808）
提供合成 release JSON（新版 1.0.1，资产 zip = 旧包副本 + `NEW-VERSION.txt`，
`digest=sha256:<正确哈希>`）→ `check_core` → `app_update_spec`（下载+校验+
安全解包）→ 落 `InstallPlan` JSON → 启动一个“假 app”进程并让 runner 以
`--pid` 等待它 → runner 原子替换、备份 `v2rayN-R.previous`、重启 stub、写
结果 JSON。

断言全部通过：
- `NEW-VERSION.txt` 存在；`version.txt == 1.0.1`；
- 备份目录 `v2rayN-R.previous/` 存在且 `version.txt == 1.0.0`、
  `v2rayn_desktop.exe` 存在；
- `install-manifest.json` 写入；
- 结果 JSON `ok=true`、`stage=done`、`restarted=true`；重启 stub marker 出现。
- 关键时序：runner 启动后 400ms 内“假 app”仍存活时，安装目录未被改动。

### 失败/回滚用例

1. `digest_mismatch_aborts_without_touching_install`：release 元数据声明的
   sha256 与服务器实际字节（另一个合法 zip）不一致 → `DigestMismatch` 映射为
   `E_CONFLICT`，替换中止。断言旧包三个文件逐字节不变、无 `NEW-VERSION.txt`、
   无备份目录、无 staging 泄漏。
2. `commit_rename_failure_leaves_install_usable`：runner `--fail-inject commit`
   注入最终 rename 失败 → `apply_atomic` 自行回滚 stage rename → runner 退出
   非 0，旧安装逐字节可用，无 keep 目录残留。
3. `crash_between_renames_is_recovered`：模拟半完成交换（current 缺失、keep
   存在）→ `updater::install::restore_previous` 将 keep 改回 current，旧版本
   逐字节恢复。

结果 JSON：`T21-install-update.runs/selfupdate-20261002T050059Z.json`（4/4 通过），
日志同名 `.log`。全部进程为测试自身 PID 的后台/短命子进程，只按 PID 终止。

## 3. 门禁

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test --workspace --locked`：**未整体通过**，但失败项与本任务无关：
  `crates/updater/src/signature.rs` 的两个 bundled PGP key 测试
  (`bundled_upstream_key_parses_and_is_available`、
  `bundled_upstream_key_rejects_unrelated_signature`) 失败。这些改动是工作区中
  **另一并发子代理未提交的改动**（`signature.rs`/`dgst.rs`/`fixtures/keys/`/
  `docs/evidence/T21-kcp-cross.md` 等），非本子代理所写，按项目规则未回退。
- 本任务相关 crate 定向通过：
  - `cargo test -p upgrade_runner --locked`：4/4（自更新）+ 2/2（单元）。
  - `cargo test -p updater --locked --lib install`：4/4。
  - `cargo test -p application --locked --test t16_update`：3/3。
- 安装器脚本幂等复跑：第二次 `install_test.ps1 -SkipCompile` 仍 `ok=True`，
  卸载后无残留。

## 4. 未决项

- **代码签名证书**：安装器与主程序均未签名。首次运行会触发 SmartScreen；
  未做 Authenticode 签名与时间戳。
- **真实 GitHub 端点实跑**：自更新在 loopback mock 上验证；未对
  `api.github.com` 做真实下载 + TLS 握手 + 外部替换的端到端跑（生产默认
  端点与生产校验未放宽，但未实跑）。
- **跨卷替换**：`apply_atomic` 依赖同卷 rename；安装根跨卷（staging 与
  current 不在同一卷）未验证，存在需要 copy+swap 的场景。
- **runner 的真实 GUI 重启**：测试用写 marker 的 stub 代替真实
  `v2rayn_desktop.exe` 重启，以保持无头；真实 GUI 重启未跑。
- **恢复场景**：`restore_previous` 仅覆盖“current 缺失、keep 存在”；更复杂
  的中断（keep 自身缺失/损坏）未覆盖。
- **失败证据**：`install-20261002T044946Z.json` 记录了首版脚本的
  `guiNNDB.db` 命名错误导致的误判，作为过程记录保留。

## 5. 防卡死约束遵守

- 所有后台进程记录 PID，等待 ≤120s；只终止本脚本启动的 PID 树。
- 端口：mock 用 OS 分配临时端口（≥49152），`t16_update` 与 installer 不监听；
  未使用 10808，未触碰系统代理/TUN。
- 安装测试结束即静默卸载并确认目录/快捷方式/注册项清理，无残留。
