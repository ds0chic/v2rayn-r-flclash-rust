# R3-PROF-03 — 排序边界、结果排序稳定性与整组写序

状态：`implemented`（合成 controller/pure 回归本轮已跑并绿；未做真实 Windows 窗口端到端，故不写 `verified`）。

任务 ID：R3-PROF-03

本次唯一用户流程：表头按延迟/速度排序时，失败/未测（`<=0`）行在两个方向都沉底；「按测试结果排序」后下一次 reload 不回退到之前的列排序；表头整组排序（不叠文字过滤）写 `ProfileExItem.Sort`，隐藏行也更新。

前置任务及已验证证据：第三轮复核 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` 的 R3-PROF-03 行；冻结 `UP/ServiceLib/Handler/ConfigHandler.cs:1004-1006`（`SortServers` 用 `ProfileModels(subId, "")` 整组）、`:1087-1105`（Delay/Speed `<=0` 补 `maxSort` 沉底）；既有 `reprof04_sort_readback_test.dart` 覆盖读回链与失败提示（FIX-10B，不回归）。

对应 ID：`ACT-PROF-014..020`、`LAY-PROFILES-002`、`FLD-CFG-*`（Sort/方向）。

必读上游文件、符号和固定 commit：UP `7d6a967`；`UP/ServiceLib/Handler/ConfigHandler.cs:1004-1111`；`UP/ServiceLib/ViewModels/ProfilesViewModel.cs` 排序入口。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：当前组（`groupSubId`）与 `state.sort`（列/方向）；`sortByResult` 使用 DelayVal。
- 输出：整组（忽略文字过滤）排序后经 `speedtestApplyProfileOrder` 写 `Sort`；`state.sort` 与写序同步。
- 错误：写失败保留 `orderMessage`，成功后清除（沿用 FIX-10B）。
- 取消/权限：无网络、无内核、无系统代理。
- 持久化：`ProfileExItem.Sort`；reload/重开读回。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_models.dart,profiles_controller.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/R3-PROF-01-04/**`、`compat/actions.yaml`（仅追加）。
禁止改变：不改 FIX-10/10B 已提交的排序写链与选中集合语义；不回归 RE-PROF-14（自动列宽/搜索时机）与 RE-PROF-13（活动标记）；不改 `features/**`（除 profiles 目录）与 `crates/**`。

测试夹具和原版预期：合成 A/B/C（delay -1/10/100）+ 先备注排序再结果排序 + reload 断言稳定；隐藏 B 后验证整组写序包含 B。原版预期：失败/未测两向沉底；整组写序不叠文字过滤。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`
- `flutter test test/recheck_r3_prof_controller_test.dart test/reprof04_sort_readback_test.dart`
- 真窗 + 真 SQLite 重开对照（本轮未做，登记）。

证据文件位置：`docs/evidence/recheck-fixes/R3-PROF-01-04/`。

完成条件：`applySort` 对 DelayVal/SpeedVal 两向沉底；`sortByResult` 同步 `state.sort` 且 reload 稳定；`sortBy` 写整组含隐藏行；既有 `reprof04_sort_readback_test` 保持绿。未真窗对照，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`sortBy` 的整组写序复用 `applyProfileOrder`；如需“部分组/部分 id 原子写序 + 部分失败上报”，需要新用例契约，本卡不造新 IPC。

本轮实际结果：`profiles_models.dart` 的 `applySort` 对 `DelayVal`/`SpeedVal` 增加 `<=0` 两向沉底；`profiles_controller.dart` 的 `sortByResult` 改为经统一 DelayVal 排序并同步 `state.sort`；`sortBy` 改为对 `_groupScope`（整组、忽略文字过滤）排序并 `_persistOrder(ids:)` 写入含隐藏行；新增 `_groupScope` 与 `_persistOrder({ids})`。命令：`flutter analyze` → No issues；`flutter test test/recheck_r3_prof_controller_test.dart`（3 项排序）→ 8/8；`flutter test test/reprof04_sort_readback_test.dart` → 5/5。
