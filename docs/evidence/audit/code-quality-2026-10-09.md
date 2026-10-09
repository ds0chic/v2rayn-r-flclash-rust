# 代码质量审计 2026-10-09 — 证据

范围：Rust workspace（全部 crate 与 services）+ Flutter `apps/desktop/lib` 静态检查。
原则：只改值得改/必须改的地方；不改 FRB 公开签名（`frb_generated` 未重新生成），
不改行为与持久化格式，不动 `work/`、`outputs/`、`compat/`。

## 1. 改动

### 重复实现合并
- `runtime/src/sha256.rs`：手写 SHA-256 换成 `sha2`（已在锁文件中，
  `persistence::hash` 同用）。此前 `application` 同时使用两套实现；手写版对输入
  整体 `to_vec()`（Geo 文件哈希时多拷贝一份），且依赖仅 1.98 才有的 clippy lint
  名。删除未使用的原始摘要导出 `runtime::sha256`，保留 `sha256_hex` 与 FIPS 向量测试。
- `subscriptions/src/tls.rs`、`updater/src/tls.rs`：两份逐字相同的 `HttpsTrust`
  契约改为 re-export `platform::http`（Wave B 已建成的统一实现）。调用点
  （`subscriptions::download`、`updater::{download,fetch}`、`application::webdav`）
  的逐项 `match` 类型转换随之删除；`CoreReleaseApi` 不再重复保存 `trust`，
  改由共享客户端的 policy 提供。分类器单测并入 `platform::http`。
- `privileged_helper/src/server.rs`：删除与 `audit::now_ms` 相同的私有副本。
- `bridge_api/src/api/t16.rs`：`backup_local/restore/recognize`、
  `backup_import_upstream(_merge)`、`webdav_restore` 中重复 6～8 次的失败 DTO
  合并为单一映射（`restore_failed` / `import_failed` / `?` 传播）。输出字段值不变，
  导入未开始时 `errors=0`、执行失败时 `errors=1` 的区分保留。新增函数均为私有，
  FRB 生成代码不受影响。
- `application/src/update_service.rs`：`check_core_inner` 两个结果相同的
  “手动/不支持”分支合并；与 `app_source_unconfigured_check` 共用
  `CoreUpdateCheck::unsupported`。

### 无用代码删除
- 仅为“保活/锚定”而存在的函数：`application::routing::{non_empty, opt_json,
  _opt_json_keepalive}`、`config_codegen::{singbox::config::empty_map,
  xray::stat::empty_object}`、`net_host::managed_process::_codes_anchor`。
- 无调用者：`net_host::session::identity_alive`、
  `net_host::lifecycle::active_lease_survives_ui_idle`（恒返回 `true`；规则本身已写在
  `lease_expired` / `lease_ownership_key` 文档中）、`updater::arch::within_range`
  （注释声称被 channel 规则使用，实际无调用）、`updater::unpack::read_capped`、
  `persistence::backup::bundle_digest`。
- `domain::routing::parse_imported_rules`：已被
  `application::routing::parse_imported_rules_compat`（兼容 camelCase/序号等上游形态）
  取代，无调用者；保留会诱导走错解析器。
- `apps/desktop/lib/shared/widgets/app_feedback.dart`：全仓无 import
  （各处直接用 `SnackBar`）。T17 证据中登记的该文件未接线。
- 标注为 staged（SP-08/09/10，有单测与证据登记、等待 SP-00 接线）的
  `#[allow(dead_code)]` 项**保留未动**。

### 依赖清理
- 删除未使用依赖：`core_adapters: percent-encoding`（实际用 `urlencoding`）、
  `subscriptions: serde_yaml, flate2, dev tempfile`、`updater: futures-util`、
  `upgrade_runner: domain`。
- 仅测试使用的依赖移入 `[dev-dependencies]`：`platform: serde_json`；
  `upgrade_runner: application, tokio, sha2, hex`。`upgrade_runner.exe` 本体不再编译
  `application`（含 bundled SQLite）。`Cargo.lock` 仅随之删减条目、新增
  `runtime -> sha2`。

### 小幅实现改进
- `domain::runtime_plan::ProcessGraph::start_order`：Kahn 排序的就绪集由
  “`Vec` + 每步 `sort` + `remove(0)`”改为 `BTreeSet::pop_first`，顺序语义不变。
- `platform::http::fetch_text`：新增 `Fetched::into_text`，不再整份克隆响应体；
  `follow`（6 个参数）上多余的 `too_many_arguments` 允许项删除。
- `subscriptions::fmt::shadowsocks`、`application::custom`：`contains` 后 `unwrap`
  改为 `split_once`/`Option::filter` 直接表达。

## 2. 实际运行的命令与结果（Linux 容器，Rust 1.98.1）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --exclude net_host --all-targets --locked -- -D warnings` | 通过（改动前后均通过） |
| `cargo clippy -p net_host -p privileged_helper -p runtime --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings` | 通过（Windows 目标类型检查，未链接、未运行） |
| `cargo test --workspace --exclude net_host --locked --no-fail-fast` | 见下 |

| 阶段 | passed | failed | ignored |
|---|---|---|---|
| 改动前（基线） | 1707 | 16 | 1 |
| 改动后 | 1704 | 16 | 1 |

- 失败集合改动前后逐条一致（16 项，均为环境原因：缺 `tools/cores/**.exe` 内核、
  Linux 上重启桩无执行权限），见 §3。
- passed 少 3：`subscriptions`/`updater` 两份重复的 `tls::tests`（各 2 项，共 4 项）
  合并为 `platform::http::tests::classifier_marks_only_tls_strings`（+1），
  `provider_mapping` 断言并入 `platform::http` 既有测试。

## 3. 未运行 / 未验证
- Flutter：容器内无 Flutter 工具链，`dart format` / `flutter analyze` /
  `flutter test` / `flutter build windows` **未运行**。Dart 侧仅删除一个无引用文件。
- `net_host` 为 Windows 专属（命名管道），Linux 上不能编译/运行测试；
  仅做了 msvc 目标 clippy。`upgrade_runner` 与 `platform` 的 Windows 分支因
  `ring` 的 C 构建无法交叉检查，**未验证**。
- 需要真实内核（`tools/cores/**.exe`）或 Windows 执行语义的测试在 Linux 上失败，
  改动前后集合一致（见上表）。

## 4. 建议的后续（未做）
- `net_host/src/dacl.rs` 与 `privileged_helper/src/pipe_security.rs` 约 20 段重复的
  DACL/安全描述符代码，可抽到共享模块；需 Windows 实机验证。
- `platform/src/{autostart,sysproxy}/windows.rs` 重复的注册表 FFI 包装，可合并为
  一个 `winreg` 小模块；需 Windows 构建验证。
- `updater::semver`：数字前导零的预发布标识（`rc.01` vs `rc.1`）下 `Eq` 与 `Ord`
  不一致；SemVer 本身禁止前导零，实际影响很小。

---

# 第二轮：流程 / 前端逻辑 / bug（2026-10-09）

## 5. 改动

### Rust
- `application/codegen.rs`：`RulesItem.RuleType` 为 null 时按 `All` 处理（上游可空，
  null 同时进入路由与 DNS）；此前映射为 `Routing`，内置白/黑名单模板（无 `ruleType`）
  的规则全部被排除出 DNS 生成。新增测试 `rules_without_rule_type_apply_to_routing_and_dns`。
- `domain/routing.rs`：删除无引用且取值错误（混入 `DomainStrategy4Freedoms`）的
  `DOMAIN_STRATEGIES`。
- `persistence/candidate.rs`：幂等短路仅用于 Merge；Replace 恢复同一备份时重建，
  此前本地改动后再恢复同一备份为空操作并报成功。新增测试
  `restoring_the_same_source_again_replaces_local_changes`。
- `application/backup_service.rs` + `bridge_api/t16.rs`：`list` 返回备份根路径，
  列表项可直接恢复（此前 `root` 恒为空）。
- `platform/pac.rs` + `application/platform_service.rs`：PAC 刷新使用新的代理规则
  （此前运行中刷新保留旧端口）。新增测试 `refresh_renders_the_updated_proxy_rule`。
- `application/subs.rs`：定时 tick 每次尝试后写 `UpdateTime`（取消除外），对齐
  `TaskManager`；此前失败订阅每 60s 重复下载。见 `docs/tasks/FIX-09D.md`。
- `application/update_service.rs`：托管安装的内核不再列出两次。

### Flutter
- `profiles_controller.dart`：
  - `newDraft` 设 `isSub = false`（上游 `AddServerAsync`）；此前手动新增节点在
    下次更新同组订阅时被删除。
  - `moveSelected` 基于当前显示列表（上游 `MoveServer` 作用于 `_lstProfile`），
    分组内/排序状态下不再移错。
  - `sortByResult` 持久化整组顺序（同表头排序）；`_recompute`/`sortBy` 用全部列定义，
    隐藏“延迟”列时按结果排序不再退回首列。
  - `moveProfilesToGroup` 批量保存后只重载一次，删除未用参数 `subRemarks`。
- `subs_controller.dart` / `subs_actions.dart`：
  - `update()` 结束后刷新节点表（订阅窗口单条更新此前不刷新）；桥接异常不再锁死 busy。
  - 定时更新运行时每 60s 比较 `UpdateTime`，有变化则刷新订阅列表与节点表。
  - 粘贴添加订阅只更新新增项；`preserved_error` 不再同时计入“保留”。
- `status_bar_view.dart`：本地/局域网按上游 `InboundDisplayStatus` 显示
  （此前恒为 `--`）；`ui_shell_controller.dart` 删除无用字段/方法。
- `settings_controller.dart`：删除无效 `clearStatus`；三份相同状态函数、两份
  AutoRun 读取合并；草稿修订号改用 `Expando`；规范化哈希去掉 O(n²) 查找。
- 测试：新增 `test/audit_r2_flows_test.dart`；`fix10b` 一条断言改为整组排序语义。

## 6. 实际运行的命令与结果（Linux 容器）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --exclude net_host --all-targets --locked -- -D warnings` | 通过 |
| `cargo test --workspace --exclude net_host --locked --no-fail-fast` | 1706 通过 / 17 失败 / 1 忽略（见下） |
| `dart format --output=none --set-exit-if-changed lib test` | 通过（450 文件，0 改动） |
| `flutter analyze` | 无问题 |
| `flutter test`（除 `scan_image_qr_test`、`scan_screen_qr_test`） | 1219 通过 / 10 跳过 / 14 失败 |

- Rust 失败 17 项 = 第一轮同一组 16 项环境失败（缺 `tools/cores/**.exe`、Linux 重启桩）
  + `bridge_api::api::subs::tests::sp14_preview_custom_leaves_no_files`：与提交类测试共用
  临时目录的并发竞争，单独重跑 3 次 2 次通过；相关文件本轮未改。
- 新增 Rust 测试均通过：`rules_without_rule_type_apply_to_routing_and_dns`、
  `restoring_the_same_source_again_replaces_local_changes`、
  `refresh_renders_the_updated_proxy_rule`、`scheduler_pass_reports_unavailable_endpoint_not_fake_success`。
- Flutter 失败 14 项与改动前基线（1212 通过 / 10 跳过 / 14 失败）逐条一致，均非本轮引入：
  - 环境：`t21e_import_forms` ×5（测试未加载 Rust 原生库）、`fix03b_custom_browse` ×2、
    `r3_root_03_actions` ×2（Windows 路径/`cmd`）。
  - 测试过时：`r4_11`（未注入假平台桥，开机自启写入失败致窗口不关）、`r4_23`/`r4_31`
    （日志合并刷新后未等待合并窗口）、`t18b`（错误码不再以文本显示）、`t11`（状态栏不再显示
    “路由模式:” 字样）。
- `scan_image_qr_test`、`scan_screen_qr_test`（9 项）在基线即卡死：测试未确认 SP-14 预览框；
  本轮排除未运行。

## 7. 未运行 / 未验证
- `flutter build windows`、`net_host` 测试：未运行。Windows 平台行为：未验证。

## 8. 已确认未修（按用户要求停止修复）
- 路由设置 DomainStrategy 下拉为 `DomainStrategy4Freedoms` 列表（`routing_windows.dart:18`），
  应为 `AsIs / IPIfNonMatch / IPOnDemand`（FLD-CFG-114）。
- 备份恢复：失败时 `resyncAfterRestore` 会启动原本停止的内核；导入被拒绝时详情显示
  `unknown`（应显示 `message`）；恢复后 WebDAV 输入框与更新窗口未刷新；压缩包恢复/导入
  抛异常时未做失败后的状态恢复。
- 节点编辑器：生成 UUID / 获取证书后输入框不刷新；VLESS UUID 校验比后端严格。
- 日志面板 `_reloadLogs` 未清空待刷新缓冲，重载后出现重复行。
- 上述过时测试与卡死的扫码测试。
