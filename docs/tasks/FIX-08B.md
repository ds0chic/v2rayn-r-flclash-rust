# FIX-08B — DNS 导入/应用完整流（FIX-08 拆分）

状态：`implemented`（UI→Rust→持久化→重开→生成配置断言均有测试覆盖；
真实 Windows 窗口逐事件对照与 release 构建未跑，故不写 `verified`）。

任务 ID：FIX-08B

本次唯一用户流程：DNS 窗口“导入默认（Xray/sing-box）→ 预览 → 保存/取消”
与“区域预设（默认/俄罗斯/伊朗）远端下载 → 应用 → 重开仍生效”，以及
双核自定义启用时普通 DNS 的禁用联动。失败不假成功，取消不留痕。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit
`7d6a967c18c697f28dc6917122ed3a4993fcf336`；FIX-08 已落地草稿/保存/合并
语义（`docs/tasks/FIX-08.md`、`docs/evidence/UX-PARITY-FIX-08/`）。
本卡只读基线 HEAD `04fcd93`。审查结论见
`docs/evidence/parity-review-2026-10-03/settings-report.md:65`（联动缺失）、
`repair-queue.md` 第 26 行。

对应 feature / field / action / layout ID：`ACT-DNS-001`、`ACT-DNS-002`、
`ACT-DNS-003`、`F-DNS-001`、`F-DNS-002`、`F-DNS-005`、`FLD-ENT-118/121/122`、
`LAY-DNSSET-001`。

必读上游文件、符号和固定 commit：
`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
`ServiceLib/ViewModels/DNSSettingViewModel.cs:38`（`IsSimpleDNSEnabled`）、
`:49-61`（两个导入命令只改窗口属性）、`:63-64`（双自定义变化触发通知）、
`:106-176`（全部文本先验后存）、`:178-196`（依次存双核行再 SaveConfig 关窗）、
`v2rayN/Views/DNSSettingWindow.xaml.cs:66-75`（`IsSimpleDNSEnabled` 绑定
`gridBasicDNSSettings.IsEnabled`/`gridAdvancedDNSSettings.IsEnabled`）、
`ServiceLib/Handler/ConfigHandler.cs:2684-2725`（`InitBuiltinDNS`：空则建双行，
NormalDNS 空且启用则禁用）、`:2733-2753`（`SaveDNSItems`：空 Id 赋 GUID，
ReplaceAsync）、`:2762-2795`（`GetExternalDNSItem`：下载模板，Normal/Tun 若为
URL 再下载，id/enabled/remarks 取当前行）、`:2801-2814`
（`InitBuiltinSimpleDNS`）、`:2816-2832`（`GetExternalSimpleDNSItem`）、
`:2894-2929`（`ApplyRegionalPreset`：默认重置；Russia/Iran 设 URL 下载
v2ray.json/sing_box.json/simple_dns.json，simple 为空则双核 enabled=true）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：各页控制器草稿（基础/高级/双核自定义）+ 区域预设下拉；导入默认只改
  窗口控制器，不落盘。
- 输出：保存走 `save_simple_dns`（整树，GlobalFakeIp 透传）+ 两行 `save_dns`
  （先验后写）；区域预设走 `apply_regional_preset`，远端三模板全部下载并解析
  成功后一次落盘（双核行 + SimpleDNS + geo/srs/routing 源 URL）。
- 错误：DNS 文本非法 → 就地提示、任一阶段不写、窗口不关；远端下载/解析失败
  → `ok=false`，存储不变，Dart 弹“区域预设下载失败，配置未变更”，不报成功。
- 取消：取消/关闭只关窗丢草稿；导入默认预览丢弃；预设刷新只更新未编辑字段。
- 权限：仅本机 UI + FRB/Rust/SQLite 与配置树；mock 端口 ≥11808；不启动内核、
  不写系统代理/TUN、不监听生产端口。
- 持久化：SQLite（`DNSItem` 行）+ `guiNConfig.json`（SimpleDNS + 源 URL）。
- 生效：保存后由应用入口 apply；预设成功后刷新下拉/字段。

允许修改的模块：`apps/desktop/lib/features/routing/{dns_window.dart,dns_controller.dart}`、
`apps/desktop/test/**`、`apps/desktop/integration_test/**`、
`crates/application/src/dns.rs`、`crates/bridge_api/src/api/dns.rs`、
`docs/evidence/UX-PARITY-FIX-08B/**`、本卡、`compat/*.yaml`（仅追加）。

禁止改变的已有行为：`main_shell`、两处 `frb_generated`、`features/settings`、
`features/profiles`、`features/subs`、`features/runtime`、`features/monitor`、
`features/update`、`features/backup`、`crates/application/src/engine.rs`、
`crates/updater`；不删入口或降分母；不伪造下载/落盘/应用结果；不跑全仓 fmt、
不跑 `flutter build windows`（根代理统一跑）。

测试夹具和原版预期：合成 Pascal/camel DNS 模板与 simple_dns 模板（本地
`bind_loopback` mock，从 11808 起探测）；合成 `119.29.29.29`/`8.8.8.8` 地址。
原版预期：导入只改窗口属性、缺行建行赋唯一 ID、双自定义同时启用即禁用普通区、
远端失败保留旧行、SimpleDNS 文本形状校验。

本次必须通过的命令/真实场景：
- `dart format <changed files>`；`flutter analyze`（No issues）；
  `flutter test test/fix08b_dns_apply_test.dart`；回归 `test/fix08_dns_draft_test.dart`。
- Rust：`cargo fmt -p application -p bridge_api -- --check`、
  `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`、
  `cargo test -p application --locked --lib`、
  `cargo test -p application --locked --test t11_routing_dns`、
  `cargo test -p bridge_api --locked`。
- 真实窗口集成测试与 release 构建未跑（根代理统一跑）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-08B/README.md`。

完成条件：导入默认取消不留痕、保存后重开一致；区域预设远端全成功才落盘、
失败保留旧数据且不假成功；双自定义启用禁用普通区；缺行建行唯一 ID；
生成配置含 DNS 且 GlobalFakeIp 保留；门禁通过。未做原版实机窗口逐事件对照，
故状态保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：无“区域预设下载进度/取消”通道。当前 `apply_regional_preset`
  为同步阻塞调用（内部 current-thread runtime），远端超时上限走
  `fetch_rules_text` 的 15s/请求，全链最长约 45s。若后续要求可取消/进度，需
  独立 job + 进度事件契约（本卡不造新 IPC、不改生成文件）。
- 有意偏离（记录）：上游 Russia/Iran 在 SimpleDNS 下载失败时仍落盘
  `InitBuiltinSimpleDNS` 并把双核 enabled=true（“离线兜底”）。本卡要求“失败不
  假成功”，故任一文件失败即整体不落盘并报错，更严格；如需完全复刻离线兜底，
  应作为可选“离线模式”另开，不与远端链混用。
- 接口缺口（登记）：Rust `RegionalPreset::{RussiaOffline,IranOffline}` 离线枚举
  在 FRB 路径已不再被调用（Dart 侧只走下载链）。建议后续收敛枚举命名/移除，
  非本卡阻塞。

本轮实际结果：
- `crates/application/src/dns.rs`：新增 `RegionalDnsPlan`、`PRESET_TEMPLATE_FILES`、
  `template_url`、`parse_dns_template`、`parse_simple_dns_template`、
  `load_dns_template`（嵌套 URL 解析 + per-core 校验）、`fetch_region_dns_plan`
  （三文件全取全解析）；新增单元测试。
- `crates/bridge_api/src/api/dns.rs`：`apply_regional_preset` 对 Russia/Iran 走
  下载链，`apply_downloaded_plan` 仅在成功后一次落盘（保留旧行 id/remarks/enabled）。
- `apps/desktop/lib/features/routing/dns_window.dart`：`_simpleDnsEnabled` +
  基础/高级 `IgnorePointer` 门 + 禁用提示；`_textBaseline`/`_boolBaseline` 与
  `_refreshPreservingDrafts`（预设刷新不覆盖其它页未保存草稿）；预设按钮
  成功/失败明确提示。
- 新增 `apps/desktop/test/fix08b_dns_apply_test.dart`（4 用例）。
- 门禁：Rust fmt/clippy/lib 170/t11 20/bridge 43 全过；Flutter format/analyze/
  fix08b 4/fix08_dns_draft 3 全过。release 构建与真实窗口未跑。
