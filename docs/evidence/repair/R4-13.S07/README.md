# R4-13.S07 证据：Fragment4RayItem 设置保存到实际效果

- 任务：docs/repair/tasks/R4-13.S07.md（R4-13 协调包实例）
- 仓库 HEAD：`a2968923bb7354d7b099c9a1999c380d74179ff9`（执行时工作树含其他代理 R4-18/R4-19 改动，未触碰）
- 冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，UP=work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/
- 应用基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`
- armed=false；未启动内核，未监听端口，未写宿主代理/注册表/路由/TUN/自启，未读用户凭据。

## 结论

实例状态 **verified（载荷/配置消费者）**。

- 修复前：Flutter 的 LoadConfig 等价层 `mergeWithSettingsDefaults` 不执行上游 `ConfigHandler.LoadConfig:181-187` 的旧字段迁移；当 `Lengths`/`Delays` 为空时保留空列表，忽略已存的 legacy `Length`/`Interval`（FLD-CFG-154/155，apply_timing=next_launch）。
- 修复后：新增 `_promoteFragmentLegacy`，在通用默认合并前把空 `Lengths`/`Delays` 用 `Length`/`Interval`（否则 50-100 / 10-20）回填，非空列表不被覆盖；经 UI draft→save→重开保持。
- 上游对照：`ConfigHandler.cs:181-187`；`ConfigItems.cs:251-256`；消费者 `V2rayOutboundService.cs:828-839`；Rust 侧 domain `AppSettings::apply_load_defaults`（settings.rs:1074-1090）已有同迁移，codegen 由 `crates/config_codegen/src/xray/config.rs::fragment_mask` 消费。

## 复现（先失败后通过）

| 阶段 | 命令 | 结果 | 日志 |
|---|---|---|---|
| 修复前 | `flutter test test/repair/r4_13_s07_repro_test.dart` | FAIL 1/1（`Lengths` 实际 `[]`，期望 `['100-200']`） | `_prefix_repro.log` |
| 修复后 | `flutter test test/r4_13_s07_contract_test.dart` | PASS 4/4 | `_postfix_tests.log` |
| 修复后 | `flutter test test/repair/r4_13_s07_repro_test.dart` | PASS 1/1 | `_postfix_tests.log` |

修复前通过临时 `git stash push -- apps/desktop/lib/features/settings/settings_defaults.dart` 取得旧实现，测完 `git stash pop` 复原；未 add/commit。

## 必过命令结果（本实例相关）

- `flutter analyze` → 本卡修改文件零告警（仓库另有 R4-18 代理 2 条告警，非本卡）。
- `flutter test test/r4_13_s07_contract_test.dart` → All tests passed（4/4）。
- `flutter test test/repair/r4_13_s07_repro_test.dart` → All tests passed（1/1）。
- 本卡未改 Rust/原生。

## blocked / 未验证

- 真实内核 `Fragment` 出站实际效果与 legacy 配置真机迁移未在隔离环境实测 → registered，不伪造。
- codegen 侧 legacy→list 迁移由 domain apply_load_defaults 承担，本卡未新增 Rust 断言。

## 文件哈希（SHA256）

- test/r4_13_s07_contract_test.dart ：FCC28EE5AA0707D227D002F81426EF0A143E2E7CC565BD10CB64A0AD9877CD26
- test/repair/r4_13_s07_repro_test.dart ：4A5AD4834ACC99E7A11A07CA6FE14C74381B0E2EE09C17E929694B84347866E6
- _prefix_repro.log ：B2224026EF0597A29F79778BDD1B36AD634FBB17224B729527AD0D08503C0109
- _postfix_tests.log ：6A326FD6F156368F9A21FC99AA768BBC917E65D8D798080D5721DFBE807FFB9B

## 改动文件

- apps/desktop/lib/features/settings/settings_defaults.dart（新增 `_promoteFragmentLegacy`，合并前迁移旧字段）
- apps/desktop/test/r4_13_s07_contract_test.dart（新增）
- apps/desktop/test/repair/r4_13_s07_repro_test.dart（新增）
- compat/fields.settings.yaml（FLD-CFG-150..155 追加 r4_13_evidence，仅追加）
- docs/repair/tasks/R4-13.md、R4-13.S07.md（状态与协调登记）
