# SP-18 多选/右键/子菜单 Esc/立即 Enter 命令集合 — 证据

状态：implemented（独立接线完成；完整卡未完成，不标 verified）。
基线：`3635392`。工作树为多卡并行共享脏树，本卡仅动文件级写锁内文件。
合成数据专用；无宿主网络/10808/系统代理/路由/TUN/DNS/Run-key 操作；
不读用户秘密。不 commit。

## 1. 本次范围（做 / 不做）

做（可独立）：纯命令校验 `isCommandContextLive`（command_context.dart 新增，
与 Rust/Dart 双镜像一致）；右键菜单执行前统一走该纯门控再做选择重绑
（profiles_table.dart `_onContextAction` 接线，行为与之前逐字等价）；
空白组拼写归一为 All（selection.rs + sub_entry.dart，与 All 视图互通）；
立即 Enter/快捷键命令集回归（新增 sp_18_command_set_test.dart，8 项）。
不做（等前置/他锁）：profiles_controller 内右键保留多选/重绑逻辑
（文件锁外，仅复用不断言改动）；子菜单逐级 Esc（未真机对照，保持整链
关闭诚实合同）；WPF 逐场景 trace/截图、DPI 跨屏、全 ACT-PROF 矩阵
（未验证，如实登记）。

## 2. 原版对照（冻结 7d6a967，只读 work/，本次实读）

- `ProfilesView.xaml.cs:220-294 LstProfiles_PreviewKeyDown`：Enter 即时
  `SetDefaultServer()`；Esc 仅 `ServerSpeedtestStop()`（从不清选择）；
  Ctrl+A 全选、Ctrl+C 导出分享、Ctrl+D 编辑、Ctrl+F 分享、Ctrl+O/R/T/E
  测速。`actionForKey` 映射与之一致；Esc 保选择已有
  `emitAction(escape)` 实现（R3-PROF-10 回归覆盖）。
- `ProfilesViewModel.cs:862-883 EditSubAsync`、`334-341/361-391`
  组/选择语义：同 SP-16 证据，不重复。
- 右键：WPF DataGrid 右键行即目标并保留既有多选；本项目
  `handleRightTap` + `CommandContext` 冻结 + `_onContextAction` 双门控
  （纯校验 + 可见性重绑）与之对应；失效一律提示重选，不回落首行。

## 3. 改动文件（写锁内，本卡）

- `apps/desktop/lib/features/profiles/command_context.dart`（+纯函数）：
  `normalizeGroupSubId`（空白→All）/`isCommandContextLive`
  （同组 + 非空 + 全可见；false=拒绝重选，无首行回落）。
- `apps/desktop/lib/features/profiles/profiles_table.dart`
 （`_onContextAction` 门控改走纯函数 + 既有重绑，行为等价）。
- `apps/desktop/lib/features/profiles/sub_entry.dart`：
  `restoreCommandTargets` 组归一（与 Rust 一致）。
- `crates/application/src/selection.rs`：
  `restore_command_targets` 改 `present()` 归一比较 +1 单测
  `blank_group_normalizes_to_all_view`。
- `apps/desktop/test/repair/sp_18_command_set_test.dart`（新增，8 项）：
  Enter/numpadEnter→activate、Ctrl+D/A/Esc 映射、冻结 primary 优先、
  多选主行不断言拒绝、纯门控三拒收、空白组互通、primary 隐藏拒绝。
- `crates/application/src/groups.rs`：未动（订阅组 G 与 PolicyGroup 无关，
  不硬凑改动）。

## 4. 先红后修

红：新测试引用不存在的 `isCommandContextLive`，flutter test 报方法缺失
（exit!=0）。Rust 新单测先按旧语义失败（空白组被拒）。
修：实现后 Dart 本卡 8/8 绿；selection 13/13 绿。

## 5. 定向检查（实际结果，未跑全量门禁）

- `rustfmt --check crates/application/src/selection.rs`：exit 0（自有文件；
  修后已格式化）。
- `cargo fmt --all -- --check`：仍报他卡 codegen.rs 等格式差（非本卡文件，
  不动）。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：
  exit!=0，唯一错误在 `tests/t11_codegen_matrix.rs:110`
  缺字段 `local_srs_files`（他卡在途文件，写锁外，不动）。
  `cargo clippy -p application --locked --lib -- -D warnings`：exit 0，
  含本卡 selection.rs 干净。
- `cargo test -p application --locked`：被上述 t11 基线错误阻塞编译，
  exit!=0；`--lib`：330 passed / 0 failed，其中 selection 13/13。
- `flutter analyze`：3 issues，全在他卡文件（sp_20×2、sp_19 1 为他卡在途；
  本轮复查时他卡已部分修复）；本卡 4 文件零命中。
- `flutter test`（7 文件同跑）：51/51 passed。其中 sp_18 8/8、
  sp_16 9/9、table_actions 6/6；受影响既有 profiles 键鼠/右键回归
  （ux_parity_fix01_command_context、recheck_r3_prof_controller、
  recheck_r3_prof10、r4_02_contract）28 项全绿。
- `dart format --set-exit-if-changed`（本卡 4 文件）：修后 exit 0。

## 6. 正式入口 / 重开 / 清理

- 正式入口：节点表右键菜单 → 冻结 `CommandContext` → 纯门控 → 重绑 →
  原有命令分发（编辑/设为活动/分享/导出/测速/移动分组/策略组生成）；
  键盘 Enter 即时设为活动（走 `resolveSingleTarget` 冻结优先）。
- 独立重开（group_reopen）：未运行——组持久化在 profiles_controller
 （A07 独占锁）+ SP-12 收尾，本卡未碰该文件。
- owned 清理：纯内存/单测，无窗口/端口/进程/文件副作用；合成 id
  （g-syn-*/n*）无秘密。

## 7. 阻塞 / 未验证（诚实登记）

- profiles_controller.dart（A07 独占）：右键保留多选/重绑与组持久化在该
  文件，本卡只复用其公开语义，未改一行。
- 子菜单逐级 Esc：未真机对照，保持整链关闭（`menuEscClosesWholeChain`），
  不猜语义。
- 同合成数据 WPF 逐场景 trace/截图、全 ACT-PROF 菜单条件/作用/取消核对、
  DPI 跨屏无错 target：未运行，标未验证。
- FRB/桥/引擎真实接线、正式包入口：未运行，标未验证。

## 8. 卡状态

SP-18：implemented（独立命令集接线完成；完整验收 blocked by 真机
trace/DPI + 组持久化前置）。
SP-16：implemented（见 SP-16/README 第 8 节；完整接线 blocked by SP-12
+ A07 文件锁）。
