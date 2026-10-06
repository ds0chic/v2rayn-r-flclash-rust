# SP-01 证据：损坏配置读取与恢复

- 基线：`a52c846`（SP-00 已完成；开始前 `git status` 干净，`git rev-parse HEAD` =
  `a52c846993d0abf056ae1378ebd0ffd351249045`）。
- 状态：implemented（正确回归 + 最小修复 + 真实合成磁盘/SQLite/重开证据；真实 OS
  验收与正式 GUI 入口未运行，故不写 verified）。
- 原版：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
- 未 commit（根整合者验收与提交）。

## 本次唯一用户流程

打开存在坏类型或截断内容的配置后，用户看到恢复入口而原件不被覆盖。

## 原版预期（只读 `work/` 核对）

- `ServiceLib/Handler/ConfigHandler.cs:19-36`（`LoadConfig`）：文件不存在 → 新建
  `Config` 默认；文件存在但内容为空 → 记日志并返回 `null`（失败）；非空 →
  `JsonUtils.Deserialize<Config>`。
- `ServiceLib/Common/JsonUtils.cs:67-81`（`Deserialize`）：空/空白返回 default；
  **任何异常一律吞掉返回 default**，随后 `config ??= new Config()` 整树默认化。
  即原版对类型错误/截断也是静默默认（会丢掉其它合法字段）。
- `ServiceLib/Models/Configs/ConfigItems.cs:67-74`：`TrayMenuServersLimit` 为
  `int`，默认 `20`。
- 稳定移植目标（IMPLEMENTATION_PLAN §5.1）严于原版：missing 初始化与
  empty/null/type error 区分；未知 raw 字段保留；损坏内容进入只读恢复流程，
  不启动默认、不覆盖源文件、日志不记凭据。

## 正确合同（先红后绿）

新错误合同：`E_FIELD_FORMAT / error.config_corrupt`（`retryable=true`；重读无副
作用，修复后重开可重试；写路径保持阻塞）。`detail` 只带脱敏后的 serde 诊断
（双引号标量替换为 `"?"`，不断言位置外不回显原值）。

红回归（修复前运行，正确预期失败）：

| 用例 | 修复前 |
|---|---|
| `present_empty_file_is_corrupt_not_default` | 空文件被当默认加载成功（FAIL） |
| `whitespace_only_file_is_corrupt` | 同上（FAIL） |
| `truncated_json_is_corrupt_and_preserved` | 失败形式非结构化 corrupt（FAIL） |
| `bad_tray_limit_type_fails_without_defaulting_rest` | 坏字段整树默认成功（FAIL） |
| `corrupt_file_can_be_repaired_and_reopened` | 同上（FAIL） |

## 改动文件

1. `crates/domain/src/settings.rs`：新增 `AppSettings::parse_strict`（已知字段类
   型错误 → `error.config_corrupt`，不再 `unwrap_or_default` 整树默认）+
   `sanitize_serde_detail`（诊断脱敏）+ 2 个单测；其余缺省/`??=`/未知键语义不
   变（`apply_load_defaults` 仅成功后执行）。
2. `crates/persistence/src/upstream_config.rs`：新增 `parse_config_text`
  （present-but-empty 与截断 JSON → `PersistenceError::Corrupt`）。
3. `crates/persistence/src/lib.rs`：导出 `parse_config_text`。
4. `crates/application/src/engine.rs`：`read_config` 缺失→空对象初建（保留），
   现存空/语法错 → `error.config_corrupt`；新增 `config_corrupt_error` 映射；
   `read_settings_state` 返回 `Result`（严格类型解析），`open_with_runtime` 与
   `reopen` 用 `?` 向上传播 → 打不开即 fail-closed（桥接层已有
   `storage_unavailable` 失败闭合引擎），损坏源文件永不被重写。
5. `apps/desktop/lib/features/settings/option_setting_window.dart`：进程内设置
   对话框在 `loadFailed` 时显示恢复入口（损坏状态文案 +
   `settings-load-retry-inline` 重试按钮，直调控制器 `load()`）；保存本就被
   控制器 `loadFailed` 守卫拦截，不新增写路径。
6. 新增测试：
   - `crates/application/tests/sp01_corrupt_config.rs`（10 项，真实合成磁盘 +
     `open_with_runtime` + `NullRuntimeClient`，端口仅写 `11808` 常量、无
     socket/内核/OS 操作）；
   - `crates/persistence/tests/sp01_config_text.rs`（5 项）；
   - `apps/desktop/test/repair/sp_01_recovery_contract_test.dart`（3 控制器契
     约 + 1 部件恢复入口回归；损坏注入仅用内存 fake 桥）。

## 正式入口与重开（真实链路）

- 应用层读路径：`AppEngine::open_with_runtime` → `read_config` →
  `read_settings_state` → `load_settings`。损坏时 `open` 直接返回
  `Err(error.config_corrupt)`，桥接 `engine()` 切 fail-closed 引擎，
  `get_settings` 返回 `ok:false`（既有 DTO，无 FRB 变更）。
- 独立重开：`unknown_keys_survive_save_and_independent_reopen` 用同一目录先后
  创建两个独立 `AppEngine`（`drop` 后重开），语言 `en`/去重 `true`/端口
  `11808`/未知根键与组内未知键全部保留；`corrupt_file_can_be_repaired_and_reopened`
  证明损坏→外部修复→ fresh open 重试成功。
- 最终消费者：损坏时无任何设置到达 codegen/运行时（`open` 失败，无核心启
  动）；合法文档仍走原保存→codegen 链（t18 等既有链路测试全绿，未动）。
- Dart 可见性：控制器 `loadFailed=true` + `status=error.config_corrupt`（恢复
  入口），`saveDocument`/`saveGroup` 被拒且 fake 桥写计数为 0；部件测试证明
  对话框出现内联重试键，修复后同窗恢复。

## 实际命令与结果（本次运行，真实 exit）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | exit 0（先修 3 处格式后复核通过） |
| `cargo clippy -p persistence -p application --all-targets --locked -- -D warnings` | exit 0（中途修 1 处测试 `result_large_err` 后通过） |
| `cargo clippy -p domain --all-targets --locked -- -D warnings` | exit 0（额外，settings.rs 有改动） |
| `cargo test -p persistence --locked` | 全绿：68+5+8+5+8=94 passed，0 failed |
| `cargo test -p application --locked` | 全绿：lib 278 passed；集成含新 `sp01_corrupt_config` 10/10；其余全部 0 failed |
| `cargo test -p domain --locked` | 49 passed，0 failed（含新 `parse_strict_*` 2 项） |
| `dart format --output=none --set-exit-if-changed <改动的2个dart文件>` | exit 0（先格式化测试文件后通过） |
| `flutter analyze`（apps/desktop） | No issues found |
| `flutter test test/repair/sp_01_recovery_contract_test.dart` | 4/4 通过 |
| 邻接回归（单文件）：`fix08_option_error`、`fix16b`、`t12a_option_window`、`r4_12_contract` | 全部通过（r4_12 为 8/8） |

已知测试设施抖动（非本卡回归）：多文件同跑 `flutter test a b c` 时偶发
`did not complete`（先 `fix16b`、后 `t12a` 各一次）；同文件单跑均通过，
重跑同组合 6/6 通过。与审计基线记录的 Flutter 批跑加载异常一致，已如实记
录，未改测试预期。

## owned 资源回收

- Rust 回归只用 `tempfile::tempdir()`（进程退出自动清理）与
  `NullRuntimeClient`；无 socket 监听（`11808` 仅为文件常量）、无 10808 接
  触、无内核/helper/系统代理/路由/TUN/DNS/Run-key 操作。
- Dart 回归只用内存 fake 桥与 `ProviderContainer`（`addTearDown` 释放）；
  无真实用户配置、无凭据（合成 `FutureRoot`/`oops-not-a-number` 哨兵）。
- `work/`、`outputs/` 只读（仅读取冻结源码）；`compat/` 未动。

## 未验证与下一前置（含需根整合者处理的共享接口）

1. 真实 OS 验收未做：损坏配置下的正式 GUI 对话框截图、未武装包入口、其它
   OS/架构——留待 SP-30/SP-32/SP-33；故本卡为 implemented 非 verified。
2. 需要根整合者决策/生成的共享接口（本卡**未动** FRB/共享 DTO，已登记阻
   塞）：一键“备份损坏文件并恢复默认”的应用内恢复动作目前无 FRB 入口——建
   议新增 `recover_config_with_backup()`（Rust 先备
   `guiNConfig.json.corrupt-<ts>.bak` 再写默认并重载，返回结构化 receipt），
   由整合者定 DTO 并统一生成三处 2.13.0。本卡恢复入口 = 可见损坏状态 + 重
   试读取 + 保存阻塞 + 源文件保留，已满足 SP-01 完成条件。
3. `error.config_corrupt` 为新增 messageKey，`lib/shared/l10n` 不在允许模块
   内未动——UI 回退显示 key 本身；本地化映射请整合者在 SP-12/文案卡统一补。
4. 引擎自有计数器（`settings_revision`/`settings_group_revisions`）类型损坏
   时仍回退 0（可再生派生数据，非用户字段；证据留痕，不视为损坏豁免）。
5. 下一前置：SP-02（可恢复提交/journal）可直接在本卡 fail-closed 语义上继续。
