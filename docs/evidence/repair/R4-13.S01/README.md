# R4-13.S01 证据：CheckUpdateItem 设置保存到实际效果

- 任务：docs/repair/tasks/R4-13.S01.md（R4-13 协调包实例）
- 仓库 HEAD：`a2968923bb7354d7b099c9a1999c380d74179ff9`（执行时工作树含其他代理 R4-18/R4-19 改动，未触碰）
- 冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，UP=work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/
- 应用基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`
- armed=false；未启动本项目内核，未监听端口，未写宿主代理/注册表/路由/TUN/自启，未读用户凭据。

## 结论

实例状态 **verified（配置/请求消费者）**。

- 修复前：`updateControllerProvider.build()` 忽略已持久化的 `CheckUpdateItem`，`prerelease=false`、`viaProxy=false`、`selected`=全部 supported；`setPrerelease`/`setViaProxy`/`toggleCore` 只改内存、不回写。保存的字段是死数据，重开复位。
- 修复后：`UpdateController.build()` 从存储读 `CheckUpdateItem` 种子（`UpdateViaProxy` 缺省 true、`SelectedCoreTypes` null⇒全部选中，对齐 `UpdateService.cs:119`）；三个 setter 变更即 `saveGroup('CheckUpdateItem', ...)` 回写。真实消费者 `t16CheckUpdates(cores, prerelease, viaProxy)` 收到持久化参数。
- 上游对照：`CheckUpdateViewModel.cs:36-37/103-125/170`；`ConfigItems.cs:243-247`；`UpdateService.cs:119`。

## 复现（先失败后通过）

| 阶段 | 命令 | 结果 | 日志 |
|---|---|---|---|
| 修复前 | `flutter test test/repair/r4_13_s01_repro_test.dart` | FAIL 2/2（seeding 返回 false；toggle 不持久化） | `_prefix_repro.log` |
| 修复后 | `flutter test test/r4_13_s01_contract_test.dart` | PASS 4/4 | `_postfix_tests.log` |
| 修复后 | `flutter test test/repair/r4_13_s01_repro_test.dart` | PASS 2/2 | `_postfix_tests.log` |

修复前通过临时 `git stash push -- apps/desktop/lib/features/update/update_controller.dart` 取得旧实现，测完 `git stash pop` 复原；未 add/commit。

## 必过命令结果（本实例相关）

- `flutter analyze` → 本卡新增/修改文件零告警（仓库当前另有 R4-18 代理引入的 2 条 test unused_import 告警，非本卡文件，未触碰）。
- `flutter test test/r4_13_s01_contract_test.dart` → All tests passed（4/4）。
- `flutter test test/repair/r4_13_s01_repro_test.dart` → All tests passed（2/2）。
- 回归：`flutter test test/t16_update_test.dart` → 7/7；`flutter test test/r4_03_contract_test.dart` → 3/3；`r4_05`/`recheck_rr04_app_update`/`r3_08_update_source` → 全绿。
- 本卡未改 Rust、未改原生，未运行 cargo build windows（见下）。

## 回归适配

上游 `UpdateViaProxy` 缺省 true。修正后 `test/t16_update_test.dart`（3 处）与 `test/r4_03_contract_test.dart`（harness `proxyAvailable=true`）按修正后的缺省显式设定，未改变各自用例意图。

## blocked / 未验证

- 真实内核下载/安装、真实 GitHub 网络检查未在隔离环境实测 → registered，不伪造。
- 应用自身更新发行源未配置（`error.update_app_source_unconfigured`）保持 fail-closed。

## 文件哈希（SHA256）

- test/r4_13_s01_contract_test.dart ：7BE92CDA8F56050B7C6B0E1C920D2DF9430A6D2726BDEC811C0E33C8D6ADB307
- test/repair/r4_13_s01_repro_test.dart ：11BFA37F58AAE1C72B27194BF715BA477D70AFE63C3DC57A2DE5579722490F88
- _prefix_repro.log ：16F1C03C144DD1963E4CF6B59C1CD5BA3F5681B70989874997D502216DCDE70B
- _postfix_tests.log ：E2ECF9760A793BCB47D373E85F7DA2EFE414C9F8076F8BDA3908669E6686B0D2

## 改动文件

- apps/desktop/lib/features/update/update_controller.dart（从存储种子 + 变更回写 CheckUpdateItem）
- apps/desktop/test/r4_13_s01_contract_test.dart（新增）
- apps/desktop/test/repair/r4_13_s01_repro_test.dart（新增）
- apps/desktop/test/t16_update_test.dart、apps/desktop/test/r4_03_contract_test.dart（回归适配）
- compat/fields.settings.yaml（FLD-CFG-147..149 追加 r4_13_evidence，仅追加）
- docs/repair/tasks/R4-13.md、R4-13.S01.md（状态与协调登记）
