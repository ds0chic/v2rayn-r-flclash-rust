# UX-SPACE-02 执行证据 — 右键菜单结构/样式恢复

日期：2026-10-03。基线 HEAD `25c907e`；本回合工作树含改动，未 commit。与 UX-CTX-03 同批。
权威结构来源：`docs/evidence/context-menu-review-2026-10-03/upstream-menu-structure.json`
（冻结 `ProfilesView.xaml` 7d6a967）与真实中文标签 `fixtures/source/upstream/resx/ResUI.zh-Hans.resx`
（`upstream-menu-structure.json` 的 `localizedHeader` 是 GBK 乱码，未使用）。

## 结构对照表（上游 → 当前实现）

上游根条目 17、分隔线 4。分隔线位置：第 6 项后、第 11 项后、第 14 项后、第 16 项后。

| # | 上游 name | 文案 (ResUI.zh-Hans) | 动作 ID | 当前 kind | 状态 |
|---|---|---|---|---|---|
| 1 | menuSetDefaultServer | 设为活动 | ACT-PROF-005 | activate | 已接线（setActive） |
| 2 | menuEditServer | 编辑 | ACT-PROF-001 | edit | 已接线（editor） |
| 3 | menuCopyServer | 克隆所选 | ACT-PROF-004 | copy | 已接线（copy） |
| 4 | menuRemoveServer | 移除所选 (多选) | ACT-PROF-002 | delete | 已接线（确认+delete） |
| 5 | menuRemoveDuplicateServer | 移除重复 | ACT-PROF-003 | removeDuplicate | **禁用**：桥接无去重接口，tooltip 说明 |
| 6 | menuRemoveInvalidServerResult | 按测试结果移除无效 | ACT-PROF-021 | removeInvalid | 已接线（removeInvalidResults） |
| — | Separator | — | — | Divider | — |
| 7 | menuTcpingServer | 测试延迟 Tcping (多选) | ACT-PROF-016 | tcping | 已接线（speedtest kind 0） |
| 8 | menuRealPingServer | 测试真连接延迟 (多选) | ACT-PROF-017 | realping | 已接线（kind 1） |
| 9 | menuUdpTestServer | 测试 UDP 延迟 (多选) | ACT-PROF-018 | udpTest | **禁用**：`SpeedTestSupport.udp=false`，tooltip 登记 |
| 10 | menuSpeedServer | 测试速度 (多选) | ACT-PROF-019 | speedtest | 已接线（kind 3） |
| 11 | menuSortServerResult | 按测试结果排序 | ACT-PROF-020 | sortResult | 已接线（sortByResult 真实排序） |
| — | Separator | — | — | Divider | — |
| 12 | menuMoveToGroup | 移至订阅分组 | ACT-PROF-013 | moveToGroup | 真实子菜单（分组列表+无分组）；动作走 moveProfilesToGroup（save 路径） |
| 13 | menuMoveTo | 移至上下（子菜单 4 项） | ACT-PROF-009..012 | moveTop/Up/Down/Bottom | 已接线 |
| 14 | menuSelectAll | 全选 | ACT-PROF-032 | selectAll | 已接线 |
| — | Separator | — | — | Divider | — |
| 15 | menuShareServer | 分享 | ACT-PROF-006 | share | 已接线（shareProfilesQr 二维码） |
| 16 | menuExportConfig | 导出（子菜单 5 项） | ACT-PROF-022..026 | export* | share/base64/inner 已接线；完整配置两项**禁用**（无客户端配置导出桥接） |
| — | Separator | — | — | Divider | — |
| 17 | menuGenGroupServer | 一键生成策略组（子菜单 2 项） | ACT-PROF-007/008 | genGroupAll/Region | 已接线（genGroupAll/genGroupRegion） |

**已删除的多余根条目**：`快速真延迟`(ACT-PROF-014)、`混合测试 (真连接+测速)`(ACT-PROF-015)。
其工具栏入口保留未动（`profiles_page.dart` 工具栏仍含「快速真延迟」「混合」，属 UX-SPACE-01 范围）。

## 样式恢复

- 每个菜单行强制 32 逻辑像素高（`_menuRowStyle` fixedSize），对齐上游 `App.xaml`
  MenuItemHeight=32；`firstRowHeight=32.0` 实测（`menu-structure-parity` passed）。
- 文本列 / 右对齐快捷键列 / `SubmenuButton` 自带的子菜单箭头列分别对齐。
- 初始左右内边距 12（`_menuItemPadding`）。
- 宽度按实际标签测量；`MenuStyle.maximumSize` 限制在窗口可见边界内，超长文字移到
  `helpTooltip`（禁用项）而非产品标签。
- 分隔线 4 处按上游位置（`separatorCount=4` 实测 passed）。

## 未实现的条目（禁用 + 诚实 tooltip，禁止假成功）

| 条目 | 缺口 | tooltip | 所需 API 建议 |
|---|---|---|---|
| 移除重复 | 桥接无去重结果比对接口 | 去重需后端提供等价结果比对；当前桥接无对应接口 | 新增 `profiles_deduplicate` 或复用 `subscriptions::deduplicate` 暴露为 FRB |
| 测试 UDP 延迟 | 后端 `SpeedTestSupportDto.udp=false` | 后端 udp=false，受限范围未实现 | 实现真实 UDP 探测后置 true |
| 导出所选完整配置 / 至剪贴板 | 无客户端完整配置导出接口（仅 share uri） | 客户端完整配置导出桥接未提供 | 扩展 `export_profiles` 支持 client-config 渲染 |

## 「移至订阅分组」是否接通/阻塞

**接通（真实持久化路径）**。桥接无独立 move-to-group API，但存在等价的 profile 保存路径：
`engine.saveProfile` 回写 `ProfileDto.subid`（`dto_to_profile` 显式携带 `subid`，
`store_repo` 写入 `Subid` 列）。`ProfilesController.moveProfilesToGroup` 读取每个节点的
存储 `ProfileDto`、设置 `subid`（无分组为 `""`）、经乐观 revision 保存并 reload。
子菜单实时列出 `listSubItems()` + 「无分组」；动作执行前经会话快照校验目标 ID 仍存在。
工具断言：`move-to-group-submenu` passed（`submenu-move-to-group.png`）。
真实持久化路径由 `apps/desktop/test/context_menu_move_test.dart` 3 项单元测试证明：
`moveProfilesToGroup` 回写 `bridge.getProfile(id).subid`、无分组清空 subid、
目标失效返回 false 不假成功。
**未做「移动 → 关窗 → 重开验证 subid」的跨进程读数取证**，登记为待补（见下）。

## 证据文件

- 结构断言：`UX-CTX-03/interaction-5/observations.json` 的 `menu-structure-parity`。
- 截图：`UX-CTX-03/screens-1/menu-overview.png`（全貌）、`submenu-export.png`、
  `submenu-move-to-group.png`、`menu-edge-bottom-right.png`、`menu-keyboard-highlight.png`。
- 单元断言：`apps/desktop/test/context_menu_model_test.dart` 的
  `restores the frozen upstream 17-root / 4-separator structure`；
  `apps/desktop/test/context_menu_move_test.dart`（移动分组的真实持久化）。
- 结构观测副本：`UX-SPACE-02/structure-observations.json`（interaction-5 原样副本）。

## 未验证与遗留

- 「移至订阅分组」跨进程持久化读数（关窗重开确认 Subid）未取证。
- 完整客户端配置导出的节点级渲染未实现。
- 多 DPI / 大字号下菜单宽度边界未回归；仅测到 1184×761 逻辑像素一档。
- 原版按下/松开时机、子菜单 Esc 退层、滚轮规则仍未实测（与 UX-CTX 系列共享）。

## 门禁

同 `docs/evidence/UX-CTX-03/README.md`：format/analyze（本轮文件 0）、build windows release 成功、
逐文件 flutter test 全绿、cargo fmt/clippy/test 全 0（未改 Rust）。
