# SP-15 平台配置按内容生效 — 证据

状态：implemented（纯平台 fake 正确合同绿；授权隔离机真实 OS 效果未验，不写 verified）。
基线：`3635392`。不要 commit，根整合者验收提交。
合成数据专用；测试端口仅 loopback PAC 自选端口（≥11808，实测 11808 起）；10808 从未占用/修改；
宿主 WinINET/注册表零写入（全 fake/dry-run）；`work/`、`outputs/` 只读未动；`compat/` 未动。
并行兄弟卡（SP-24 等）同树作业中：`settings.rs`、`codegen.rs`、`app_theme.dart` 为他人文件，本卡未动。

唯一用户流程：用户同模式同端口修改 bypass/PAC 后，实际系统效果更新且失败可重试。

## 0. 缺陷复现（红）

- CP-13：平台内容去重忽略 bypass/PAC 等变化。基线实测：
  `applied_content_hash(mode, settings, session)` 存在但 `PlatformService::apply_proxy`
  从不调用它——每次无条件写 OS（相同内容重复写），且无 desired/applied 跟踪、无失败重试语义。
- 本卡红探针（基线 `3635392` 上实测，回复现错误即红成立）：
  - `crates/platform/tests/sp15_applied_hash.rs` 初版：
    `error[E0432]: unresolved import applied_content_hash_with_pac`——PAC 内容无身份段；
  - `crates/application/tests/sp15_platform_content.rs` 初版：
    `E0560 session_key / E0609 skipped_as_duplicate, desired_content_hash,
    applied_content_hash`——服务层无内容去重、无 desired≠applied、无失败可重试。

## 1. 改动文件

- `crates/platform/src/sysproxy/mod.rs`（仅本卡符号）：
  新增 `applied_content_hash_with_pac(mode, settings, session_key, pac_content)`；
  旧 `applied_content_hash` 原签名保留并委托（`None` 即 URL-only 身份；绝对值因归一化+
  PAC 空段与旧实现不同，但同/异关系保持——hash 仅内存态、不落盘，无持久兼容问题）；
  文本段归一化（trim、空==缺失；bypass 按 `;`/`,` 分 token 修整重接，`"<local>" ==
  "  <local> ; "`）；PAC 段为脚本 md5。
- `crates/application/src/platform_service.rs`（仅本卡符号）：
  `ProxyApplyOutcome` 新增 `desired_content_hash` / `applied_content_hash` /
  `skipped_as_duplicate`（只读旧字段不动，`bridge_api` 仍编译兼容；
  `ProxyApplyRequest` 零改动——`bridge_api` 按字面量构造，不可加字段）；
  服务新增 `desired_hash`（末次请求）/ `applied_hash`（仅成功推进）/
  `pac_content`（末次 `pac_start` 脚本+规则 md5）；
  `apply_proxy` 先记 desired→命中（desired==applied 且现值仍全为我方值）则零写跳过→
  OS 写失败直接返回 `Err` 且 applied 不动→成功才推进 applied；
  新增 `desired_hash_for`（纯函数，session_key 参数化，桥接默认 `""`）、
  `desired_content_hash()` / `applied_content_hash_state()` /
  `apply_status()` / `needs_retry()`。outcome 只报平台效果事实，
  无 revision/token（保存成功只能由 SP-12 提交路径报告）。
- 测试（本卡新增，随改动同步）：
  `crates/platform/tests/sp15_applied_hash.rs`（4）；
  `crates/application/tests/sp15_platform_content.rs`（6，`FlakyBackend` 故障注入）。

## 2. 真实命令与 exit（本机 dry-run/fake；真实 OS 写入未授权）

| 命令 | exit | 结果 |
|---|---|---|
| `cargo test -p platform --locked --test sp15_applied_hash`（红阶段） | 非 0（E0432） | 缺 PAC 内容身份，得证 |
| `cargo test -p application --locked --test sp15_platform_content`（红阶段） | 非 0（E0560/E0609） | 无去重/无 desired-applied/无重试，得证 |
| `cargo test -p platform --locked --test sp15_applied_hash` | 0 | 4 passed |
| `cargo test -p application --locked --test sp15_platform_content` | 0 | 6 passed |
| `cargo test -p application --locked --test t13_platform` | 0 | 15 passed（回归） |
| `cargo test -p application --locked --lib platform_service` | 0 | 14 passed（回归） |
| `cargo test -p platform --locked`（全包） | 0 | 66 passed / 3 ignored（详见 commands.log） |
| `cargo fmt -p platform -p application -- --check` | 1（非本卡） | 剩余 diff 全在兄弟文件（`codegen.rs`/`monitor.rs`）；本卡 4 文件零 hunk |
| `cargo clippy -p platform --all-targets --locked -- -D warnings` | 0 | 干净 |
| `cargo clippy -p platform -p application --all-targets --locked -- -D warnings` | 101（阻塞，非本卡） | `codegen.rs` 并行改动中：`t11_codegen_matrix` 缺 `local_srs_files`（他人文件，未动，待其合入后由整合者复验） |
| 授权隔离机 bypass/PAC/自启成功失败/外部变化重开 | 未运行 | 无授权环境；本机一律 fake，不写宿主 |

## 3. 正确合同摘要（合成数据）

- 同内容重复 apply：零新写 + `skipped_as_duplicate=true` + `needs_retry=false`。
- 同模式同端口改 bypass（`<local>`→`<local>;192.0.2.0/24`）：hash 变→重写→applied 追 desired。
- 同模式同端口同 PAC-URL 换服务脚本（v1→v2，经 `pac_start`）：重写；纯空白 bypass 差不重写。
- OS 写失败（注入 `Backend`）：`Err` 如实上浮，desired 留存、applied 保持（`None`）、
  `needs_retry=true`；同请求重试成功后 applied==desired、`needs_retry=false`。
- 外部改动某已拥有字段后同内容再请求：不跳过，重写恢复期望效果（去重不掩盖漂移）。
- `restore_proxy` 不改写 applied（applied=末次成功，現值走 snapshot/ownership 观察）。
- 应用成功≠保存成功：outcome 无 revision/token；`platformApply` 由调用方按 outcome 填充，
  `save` 相只能来自提交路径（SP-12）。

## 4. 兼容与限制（如实）

- 旧 `applied_content_hash` 签名保留：绝对值因归一化（trim/空处理）与新增 PAC 空段
  与旧实现不同；正常输入的同/异关系（同则同、bypass 变则变）保持，
  旧单测 `applied_hash_changes_on_bypass_only_edit` 仍绿。applied hash 仅内存态、不落盘。
- `desired_hash_for` 的 `session_key` 在桥接调用中为 `""`（进程单服务实例下稳定）；
  运行时 session 接线（`RuntimeIntent`/SP-00 合同）由根整合者接入，届时调用方传真 session。
- 远端 PAC（非本服务 serve 的 URL）正文变化不跟踪（只跟踪 URL）；本地 serve 脚本跟踪全文。
- 真实 OS 效果（WinINET/注册表/PAC 落盘/自启 Run-key/外部变化重开）未验：
  需授权隔离环境，由 SP-32 承接；本卡保持 implemented，不标 verified。

## 5. `git status --short`（收尾，见最终消息）

本卡改动：`crates/platform/src/sysproxy/mod.rs`、
`crates/application/src/platform_service.rs`、
新增两测试文件与本证据目录；另更新 `tasks/SP-15.md` 状态行与
`execution-manifest.json` SP-15 块 `status` 为 `implemented`。
其余改动（`settings.rs`、`codegen.rs`、dart/证据等）属并行兄弟卡，未动。
