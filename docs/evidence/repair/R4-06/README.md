# R4-06 底栏可操作分区 — 证据

- 基线：`ef02954`（应用基线 `77c74ed`），`armed=false`。
- 范围：底栏 `status_bar_view.dart` + 桌面视觉 token；无内核/端口/宿主代理/TUN/路由/自启操作。
- 夹具：`SyntheticBridgePort` + `CountingRuntimeBridge` + `FakePlatformBridge` + `FakeMonitorBridge` + `MemoryUiStateStore` 合成数据。

## 改动
- `apps/desktop/lib/app/shell/status_bar_view.dart`：按上游 `StatusBarView.xaml` (DockPanel) 分区重写：
  - 左：两行 `本地/局域网` 端口块 → `启用 Tun` + 真实 `Switch` + 实际 TUN 标签 → 系统代理选择器 → 路由模式 → 路由方案。
  - 中：两行服务摘要（`节点` + `运行状态·端口`）。
  - 右：两行 `代理`/`直连` 速率 → `今日` 统计 → `详情`。
  - 选择器用带边框 + 下拉箭头的可辨认控件（不再是裸 Text 假控件），点击仍走共享用例（`applyModeFromConfig` / `setRuleMode` / `setDefaultAndReload` / `toggleTunDesired`），状态来自实际读模型。
  - 技术诊断（host/PID/端口/revision/epoch/seq/counts）移入 `详情` 弹出，不再混入操作行。
  - 长值单行不换行；窄窗整条分区条横向滚动，保证控件可辨可达（不缩字）。
- `apps/desktop/lib/shared/theme/app_theme.dart`：`statusBarHeight` 30 → 40，容纳两行分区。

## 合同与断言（`test/r4_06_contract_test.dart`）
1. 控件可辨认：端口两行、TUN、系统代理/路由选择器、服务/速率/统计、详情键均存在；选择器带 `arrow_drop_down` 可见下拉箭头。
2. 无会话：`节点:` 含“未运行”，速率为 `--`，不伪造已连接。
3. 点击走共享用例：无会话时选 PAC → `E_NO_RUNNING_SESSION` + “没有运行中的代理会话”，`appliedModes` 为空（诚实反馈，不假造成功）。
4. 技术诊断移详情：打开前无 `runtime-info`，打开后可见 `runtime-info/revision/counts`。
5. 100%/125% DPI：无 overflow 异常，分区矩形不重叠，主要控件 hittable。

## 已知边界
- 未运行真实截图/鼠标点击/OS 平台效果；真实 TUN/系统代理未实测 → 未 verified。
- 上游为固定单行 DockPanel；本实现窄窗以横向滚动兜底，未缩字、未整栏重排。
