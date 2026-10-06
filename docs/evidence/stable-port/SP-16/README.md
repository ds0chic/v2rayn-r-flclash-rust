# SP-16 准备与正确合同（可独立部分）— 证据

状态：identified（准备范围已实现；完整卡未完成，不标 verified）。
基线：`92d46dd`。工作树为多卡并行共享脏树，本卡仅动文件级写锁内文件。
合成数据专用；无宿主网络/10808/系统代理/路由/TUN/DNS/Run-key 操作；
不读用户秘密。不 commit。

## 1. 准备范围（本次做 / 不做）

做（可独立）：原版键鼠/右键行为对照；选择集合与当前组身份纯合同；
故障夹具与最小实现；定向检查。
不做（等前置）：SubIndexId 持久化读写与重开恢复接线（profiles_controller
A07 独占 + SP-12 A04 在途）；FRB/桥/引擎/设置-运行时重试收尾；
真实 FRB/SQLite 重开、正式包入口、DPI 真机对照。不伪造完整卡完成。

## 2. 原版对照（冻结 7d6a967，只读 work/）

- `ServiceLib/ViewModels/ProfilesViewModel.cs:862-883` EditSubAsync：
  blNew=true 新建空 SubItem；false 取 GetSubItem(_config.SubIndexId)，
  null 直接 return（All/已删门控）。修后 RefreshSubscriptions +
  SubSelectedChangedAsync。当时本项目 `profiles_page.dart:118-129`
  两按钮同调 openSubSettings（总列表），违反直达（UI-07）。
- `ProfilesViewModel.cs:334-341` SubSelectedChangedAsync 写
  `_config.SubIndexId`；`:381-391` RefreshSubscriptions 按 SubIndexId
  命中恢复否则 All；`:361-375` RefreshServersBiz pending > IndexId > 首行；
  单击只改内存 SelectedProfile，从不写默认；SetDefaultServer 显式才写。
- 键鼠/右键：单击即时选择、pointer-down 捕获 Ctrl/Shift、右键已选保留多选、
  target 冻结、滚动/失焦关闭已有实现保留；子菜单 Esc 逐级语义未真机对照，
  当前实现为整链关闭，本卡如实锁定该行为（见 sub_entry.dart 注记）。

## 3. 改动文件（写锁内）

- `crates/application/src/selection.rs`（+2 纯合同 +2 单测）：
  SubEditTarget/resolve_sub_edit_target（G 命中直达，否则 GatedAll）；
  FrozenCommandTargets/restore_command_targets/is_primary_target_live
  （组变更/不可见即失效，不回落首行）。groups.rs 未动（订阅组 G 与
  PolicyGroup 无关，不硬凑改动）。
- `apps/desktop/lib/features/profiles/sub_entry.dart`（新增，纯）：
  SubEntryKind/resolveSubEntry/resolveCreateEntry + frozen 命令恢复 +
  menuEscClosesWholeChain。如实复刻 All/缺失门控。
- `apps/desktop/lib/features/subs/sub_direct_edit.dart`（新增）：
  openEditCurrentSub（直达 G；门控/取消/保存拒绝均提示且不开总列表）/
  openAddSub（空白新建，取消零写）。保存走已有 subs_controller.save；
  跨窗 revision 重试对账待 SP-12。
- `apps/desktop/lib/features/profiles/profiles_page.dart`：
  工具栏编辑→openEditCurrentSub，新增→openAddSub（修 UI-07 同入口错误）。
- `apps/desktop/test/repair/sp_16_sub_entry_test.dart`（新增，9 项）。

## 4. 先红后修

红：新测试先引用不存在的 sub_entry.dart，flutter test 报读取/方法缺失
（exit!=0）。Rust 新函数先缺失，standalone 编译红。
修：实现后 Dart 9/9 绿；selection.rs standalone 12/12 绿（含既有 10）；
table_actions 既有 6/6 绿。

## 5. 定向检查（实际结果，未跑全量门禁）

- `rustfmt --check crates/application/src/selection.rs`：exit 0（自有文件）。
  `cargo fmt --all -- --check`：全树仍报 net_host_client.rs 等他卡格式差，
  非本卡文件，不动。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：
  exit!=0；错误全在 net_host_client.rs（E0615 as_u64 缺调用括号 ×3，
  基线既有）+ routing.rs unused_mut；无 selection.rs/groups.rs 相关。
  本卡文件 clippy 干净（按文件过滤零命中）。
- `cargo test -p application --locked`（selection/groups 过滤）：整包 lib
  被上述基线错误阻塞编译，exit!=0；改用 selection.rs 独立
  `rustc --test`：12 passed / 0 failed（合成数据）。
- `flutter analyze`：7 issues，均为 routing/settings 他卡既有；
  sub_entry/sub_direct_edit/profiles_page/sp_16 零命中。
- `flutter test test/repair/sp_16_sub_entry_test.dart`：9/9 passed。
  `flutter test test/table_actions_test.dart`：6/6 passed。
  `test/r4_07_contract_test.dart` / `r4_08_contract_test.dart`：
  均 did not complete（已知 flutter_tester 进程异常，审计已登记），
  不计通过，不装绿。
- `dart format --set-exit-if-changed`（4 自有文件）：修后 exit 0。

## 6. 等待接口 / 阻塞

- SP-12（A04 在途）：SubIndexId 持久化读写、保存 core/platform 分阶段
  收据与重试、独立重开恢复验证。需提供方/DTO/版本/生效点由 SP-00 整合。
- profiles_controller.dart（A07 独占）：setGroupSubId 持久化 + build 恢复
  + group_reopen 真实重开，本卡未碰该文件。
- FRB/桥/引擎真实接线、正式包/DPI 真机对照：未运行，标未验证。

## 7. 卡状态

SP-16：identified（准备实现完成，完整接线 blocked by SP-12 + A07 文件锁）。
SP-18：identified（冻结目标/Enter/Esc 纯合同已备，原生逐场景 trace/截图、
DPI 跨屏、全 ACT-PROF 矩阵未验）。
