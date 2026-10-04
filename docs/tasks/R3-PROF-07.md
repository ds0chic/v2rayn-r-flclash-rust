# R3-PROF-07 — 宽容导入的无效叶子被订阅策略组包含

状态：`implemented`（Rust `resolve_sub_children` 与 Dart `resolveGroupPreview` 同步只保留 `is_valid` 叶子，合成有效/缺UUID/缺地址三叶子测试通过；未运行真实 UI/持久化整链，故不写 `verified`）。

任务 ID：R3-PROF-07

本次唯一用户流程：对含宽容导入无效节点的订阅生成/预览策略组时，自动订阅子项只包含有效叶子；缺 UUID / 缺地址的叶子被跳过。

前置任务及已验证证据：冻结 v2rayN 7.25.4 `7d6a967...`；上游 `GroupProfileManager.cs:118-122`（订阅子项必须 `p.IsValid()`）；域实体 `domain/profile.rs:305-359` `Profile::is_valid`。复核来源 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` RE-PROF-10 / R3-PROF-07（HEAD `7edf1ee`）。RE-PROF-10 草稿预览主体本卡不改。

对应 feature / field / action / layout ID：`CFG-014`(PolicyGroup)、`ACT-PROF-007/008`、`FLG-ENT-005`(Subid)。

必读上游文件、符号和固定 commit：`work/.../ServiceLib/Handler/GroupProfileManager.cs:118-122`；应用侧 `crates/application/src/groups.rs:251-270`、`apps/desktop/lib/features/profiles/group_editor_dialog.dart:48-77`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：组草稿的 `SubChildItems` / `Filter` / owner `Subid` + 候选 `Profile` 集合。
- 输出：过滤后的有序子项列表（Rust 持久生成路径 + Dart 预览路径一致）。
- 错误：`Filter` 不可编译时按既有回退（生成期视为全匹配；Dart 预览同）。
- 权限：纯解析，无内核/系统写入。
- 持久化：真实解析走 Rust `resolve_sub_children`，影响自动策略组生成结果。

允许修改的模块：`crates/application/src/groups.rs`、`crates/domain/src/profile.rs`（未改）、`apps/desktop/lib/features/profiles/group_editor_dialog.dart`（仅无效叶子过滤）、`apps/desktop/test/**`、`docs/evidence/recheck-fixes/R3-06-PROF-05-07/**`、本卡。

禁止改变的已有行为：显式 `ChildItems` 叶子路径不新增有效性过滤（保持上游显式选择语义）；RE-PROF-10 草稿预览主体、去重/顺序语义不变；不删入口或降分母。

测试夹具和原版预期：合成同组三叶子——有效（有效凭据+地址）、缺 UUID（Vmess 空密码）、缺地址；`resolve_sub_children` 与 `resolveGroupPreview` 均只留有效项。原版预期为 `GroupProfileManager` 跳过无效叶子。

本次必须通过的命令/真实场景：
- `cargo test -p application --lib --locked`（`groups::tests::sub_children_drop_invalid_leaves`）
- `flutter test test/reprof10_group_preview_test.dart`（`subscription children drop invalid leaves (RE-PROF-07)`）
- `cargo clippy -p application --all-targets --locked -- -D warnings`、`dart analyze`（两文件）

证据文件位置：`docs/evidence/recheck-fixes/R3-06-PROF-05-07/`。

完成条件：Rust 与 Dart 订阅子项解析同步排除无效叶子；合成三叶子断言；门禁通过。未运行真实 UI/重开整链，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：Dart `_isProfileValid` 对 Shadowsocks 只校验密码非空，未复制 Rust 的 `SS_METHODS_SINGBOX` 方法白名单（预览为便利路径，持久生成以 Rust 为准）；如需字节级一致，应把方法白名单抽为共享常量。

本轮实际结果：Rust `resolve_sub_children` 过滤链增加 `p.is_valid()`；`groups` 测试 `leaf` 夹具补有效凭据并新增 `sub_children_drop_invalid_leaves`。Dart 新增 `_isProfileValid` 并在订阅匹配过滤中应用；`_node` 夹具补有效默认凭据/地址并新增三叶断言。`application` 224 个 lib 测试、`bridge_api` 58 个、Dart 9 个 group-preview 测试通过；clippy/fmt/analyze 干净。
