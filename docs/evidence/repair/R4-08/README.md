# R4-08 即时选择与键盘对象 — 证据

- 基线：`ef02954`（应用基线 `77c74ed`），`armed=false`。
- 范围：profiles 表格选择输入层；无内核/端口/宿主代理/TUN/路由/自启操作。
- 夹具：`SyntheticBridgePort` + `CountingRuntimeBridge` + `MemoryUiStateStore` 合成数据，20 行。

## 改动
- `apps/desktop/lib/features/profiles/profiles_table.dart`
  - `_onPointerDown`：主键按下且命中数据行时**立即** `selectRow`，Ctrl/Shift 在 pointerDown 事件时刻捕获（不再在 300ms 后的 `onTap` 回调读实时键盘状态）。
  - `_dataCell`：移除 `onTap` 选择，仅保留 `onDoubleTap`；双击业务独立识别，避免 pointer 选择与 onTap 重复 toggle。
- 未改 `profiles_controller.dart`（选择语义 `selectRow/selectRange/navigate` 保持原版）。

## 合同与断言
1. 普通单击只选择、立即生效（50ms 内，不等 300ms 双击超时），且不激活（R4-02 保持）。
2. Ctrl/Shift 多选立即生效；Ctrl 在 pointerDown 冻结，50ms 内松键后仍保留多选。
3. 单击/快速 Ctrl 点击后 50ms 内按 Enter，目标为刚选中行（primary）。
4. 无 pointer/onTap 双 toggle：第二次 Ctrl 点击只 toggle 一次。
5. 双击按 `DoubleClick2Activate`：false 打开编辑器，true 激活。
6. 拖选/排序/主行合同不回归（见命令记录）。

## 复现（先失败后通过）
- 断言来源：`docs/evidence/user-flow-audit-2026-10-05/profiles-ctrl-timing-repro.dart`、`profiles-enter-timing-repro.dart`，合并为单次页面构建写入 `apps/desktop/test/repair/r4_08_repro_test.dart`（flutter_tester 每 `pumpWidget` 泄漏，需单构建）。
- 基线（临时 `git stash` 仅回退 `profiles_table.dart`）：失败于复现测试 `line 54`（Ctrl 多选断言），与审计结论一致。
- 修复后：同一测试通过。

## 状态
`implemented`（合成 widget 合同通过；真实 OS/平台与未武装包验收未运行 → 未 verified）。
