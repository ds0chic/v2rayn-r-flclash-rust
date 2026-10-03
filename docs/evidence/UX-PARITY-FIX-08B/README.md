# UX-PARITY-FIX-08B — DNS 导入/应用完整流（证据）

任务卡：`docs/tasks/FIX-08B.md`（repair-queue 第 26 行“DNS导入/应用另卡”，
FIX-08 拆分；对应 `ACT-DNS-002`、`ACT-DNS-003`、`F-DNS-002`、`F-DNS-005`）。

## 本次唯一用户流程

DNS 窗口导入默认（Xray/sing-box）→ 预览 → 保存/取消，以及区域预设
（默认/俄罗斯/伊朗）远端下载 → 应用 → 重开仍生效；失败不假成功。

## 上游对照（冻结 7d6a967，只读）

| 行为 | 上游符号 | 本实现 |
|---|---|---|
| 导入默认只改窗口属性 | `DNSSettingViewModel.cs:49-61` | `_importDefault` 只改控制器；`import_default_dns` 在保存路径落盘 |
| 双自定义启用禁用普通区 | `:38` `IsSimpleDNSEnabled` + `DNSSettingWindow.xaml.cs:74-75` | `_simpleDnsEnabled`；基础/高级 Tab 包 `IgnorePointer(ignoring)` + 可见提示 |
| 文本先验后写 | `:106-176` | Flutter `_validateCustomTexts` 先于首次写入；Rust `validate_dns_profile` 二次拒绝 |
| 缺行建行/更新行 + 唯一 ID | `ConfigHandler.cs:2707/2740` `SaveDNSItems` | `import_default_dns` 缺行新建赋 id，已有核心行更新同一行；空 id 草稿 `normalize_dns` 赋唯一 id |
| 远端模板下载链 | `ConfigHandler.cs:2762-2795` `GetExternalDNSItem`（Normal/Tun 可为 URL，再下载） | `fetch_region_dns_plan` + `load_dns_template`；全部成功才返回 plan |
| 预设应用 | `ConfigHandler.cs:2894-2929` `ApplyRegionalPreset` | `apply_regional_preset`：远端全成功→一次落盘；失败→`ok=false`，存储不变 |

## 结果（本机 Windows，2026-10-04）

Rust（`crates/application`、`crates/bridge_api`）：

- `cargo fmt -p application -p bridge_api -- --check` → 通过（0 差异）。
- `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` → 通过。
- `cargo test -p application --locked --lib` → 170 passed。
- `cargo test -p application --locked --test t11_routing_dns` → 20 passed。
- `cargo test -p bridge_api --locked` → 43 passed。

新增 Rust 用例（`t11_routing_dns.rs`）：

- `regional_preset_remote_download_plan`：本地 loopback mock 返回三模板 →
  plan 解析出双核行 + SimpleDNS，端口 ≥11808，非 10808。
- `regional_preset_remote_failure_returns_error_before_write`：缺失
  `sing_box.json` → `fetch_region_dns_plan` 返回 Err，调用方无法落盘半成品。
- `regional_preset_resolves_nested_dns_url`：`NormalDNS` 为 URL 时按上游再下载。
- `dns_import_default_and_new_row_semantics`：内置两行；导入更新同一行；
  空 id 草稿赋唯一 id 且两行不冲突。
- `dns_config_feeds_generation_input`：DNS 行 + SimpleDNS 进入 `dns_to_codegen`
  生成模型，`global_fake_ip=false` 保留。
- `dns_save_reopen_keeps_global_fake_ip`：保存→重开后 global_fake_ip 仍 false。
- `dns.rs` 单元：模板 URL 拼接、Pascal/camel 解析、非法模板拒绝、
  simple_dns 字段解析。

Flutter（`apps/desktop`）：

- `dart format` 改动文件 → 已格式化。
- `flutter analyze` → No issues found。
- `flutter test test/fix08b_dns_apply_test.dart` → 4 passed。
- 回归 `test/fix08_dns_draft_test.dart` → 3 passed。

新增 Flutter 用例（`fix08b_dns_apply_test.dart`）：

- 导入默认预览→保存落盘；取消前存储未变。
- 预设刷新不丢其它页草稿（Xray 自定义页脏值在 apply preset 后保留；
  取消不落盘）。
- 双自定义启用→基础区出现禁用提示；仅单核启用→无提示。

## 约束遵守

- 未触碰 127.0.0.1:10808；mock 端口由 `bind_loopback` 从 11808 起探测。
- 未改宿主系统代理/注册表/路由/TUN；未启动/停止任何内核进程。
- 未读/写用户凭据；mock 模板为合成 JSON。
- `work/`、`outputs/` 只读；`compat/` 仅追加 evidence/notes。

## 覆盖边界与未完成

- 状态 `implemented`：UI→Rust→持久化→重开→生成配置断言均有测试覆盖；
  真实 Windows 窗口逐事件对照（`integration_test`）与 release 构建未跑
  （归根代理统一跑）。
- 本卡未新增 FRB 函数，故未触碰两处 `frb_generated`。
- Dart 侧 `applyRegionalPreset` 不再使用“离线 pending”文案：`ok=true` 即已落盘，
  `ok=false` 明确报错。Rust 侧仍保留 `RegionalPreset` 离线枚举供非 FRB 调用方，
  属未清理的 dead-ish API（建议后续收敛，不阻塞）。
