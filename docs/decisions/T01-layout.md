# T01 工程布局与集成决策

状态：accepted（T01）
范围：记录工程骨架、FRB 集成方式、表格组件选型与尝试过程。

## 1. 目录布局（最终）

```
Cargo.toml                     # 根 workspace：domain / application / bridge_api
crates/domain/                 # 纯模型（CoreType/ConfigType/ProfileSummary，无平台依赖）
crates/application/            # 最小用例：合成节点生成、ping、blocking 探针
crates/bridge_api/             # FRB API（cdylib+staticlib+lib）；src/api/{mod,profiles,mirrors}.rs
apps/desktop/                  # Flutter Windows 壳（org dev.v2raynr, package v2rayn_desktop）
  lib/app/                     # MaterialApp，标题 "v2rayN-R (T01)"
  lib/bridge/                  # FRB 生成代码 + 手写 BridgePort seam
  lib/features/profiles/       # 页面、虚拟表格、控制器、模型映射、草稿持久化
  lib/perf/perf_harness.dart   # S1 帧时间骨架
  rust_builder/                # FRB 生成的 cargokit 构建胶水（包名 bridge_api）
  windows/runner/main.cpp      # 窗口标题 + 草稿窗口尺寸持久化
  test/                        # 单元/widget 测试 + support harness
tools/                         # 门禁脚本（测试重试、release 截图）
```

依赖方向：UI → bridge_api → application → domain。domain 不依赖 Flutter/窗口/平台 API。

## 2. FRB 集成尝试与结论

尝试 1：`flutter_rust_bridge_codegen create` 到临时目录研究 wiring。得到标准结构：`rust/`（crate）+ `rust_builder/`（cargokit CMake/Gradle/CocoaPods 胶水）+ `lib/src/rust/`。

尝试 2：把 bridge crate 物理放到 `crates/bridge_api`，仅保留 `rust_builder/` 于 Flutter 侧，并修改 cargokit 的 manifest 相对路径指向 `crates/bridge_api`。**成功**：`cargokit.cmake` 用 `CARGOKIT_MANIFEST_DIR=${CMAKE_CURRENT_SOURCE_DIR}/${manifest_dir}` 解析，因此 `rust_builder/windows/CMakeLists.txt` 中
`apply_cargokit(${PROJECT_NAME} ../../../../../../../../crates/bridge_api bridge_api "")`
在 `flutter build windows --release` 时正确解析到仓库根的 `crates/bridge_api`。

结论：**bridge crate 保持在 `crates/bridge_api`，包名 `bridge_api`，无需物理放入 `apps/desktop/rust`。**

生成配置：`apps/desktop/flutter_rust_bridge.yaml`。重建以 `flutter_rust_bridge_codegen generate` 为准，生成 `lib/bridge/frb_generated*.dart`、`lib/bridge/api/*.dart`、`crates/bridge_api/src/frb_generated.rs`。二次运行无 diff（哈希一致）。

## 3. 表格组件选型

- 首选 flutter.dev 的 `two_dimensional_scrollables`（0.5.5）`TableView.builder`，真实二维虚拟化：pinned 表头/行号列 + 固定行高/列宽。**采用**。
- 未触发“不兼容则行虚拟化+共享列宽”的降级条件：该组件在 release 应用与单次构建的 widget 测试中均正常工作，10k 行仅物化可见单元格（实测 < 1000）。
- 曾评估降级方案（`SingleChildScrollView(horizontal)` + `ListView.builder(itemExtent)` 共享列宽）：可稳定替代，但在本任务不需要；保留为 T02 的性能备选。
- 测试期 `flutter_tester` 崩溃与本组件无因果关系：改用自研行虚拟化后同样会崩溃（详见 `docs/evidence/T01.md` S1/测试方法学）。

## 4. 窗口与持久化

- 窗口标题 `v2rayN-R (T01)`、默认 1200×800、最小宽 400/高 300（`windows/runner/main.cpp`）。
- 草稿窗口尺寸持久化：`main.cpp` 读写 exe 同目录 `v2raynr_window_state.ini`（`GetPrivateProfileIntW`/`WritePrivateProfileStringW`）。**仅为验证“保存-重开-恢复”闭环存在，不是最终 AppSettings/WindowSizeItem（FLD-CFG-156..158）合同。**
- 列宽草稿持久化：`lib/features/profiles/ui_state_store.dart` 的 `FileUiStateStore`（exe 同目录 `v2raynr_ui_state.json`），同上为临时格式。

## 5. 未决/风险

- 引擎测试崩溃（环境级）→ T02 需在 CI/真实设备上复测；本任务用重试包装规避。
- `two_dimensional_scrollables` 未在 120Hz 高刷硬件上实测帧预算；S1 仅给出 release 桌面脚本滚动骨架数据。
- Windows ARM64 / macOS / Linux 均未构建或运行（见 S4）。
