# UI-DEVIATIONS — UX 阶段 UI 偏离 v2rayN 7.25.4 一览（只增不删）

- 基线：上游冻结 `v2rayN 7.25.4`（`7d6a967c18c697f28dc6917122ed3a4993fcf336`）；本表整理自分支 `audit/2026-10-13` 各波 FIXREPORT。
- 来源（事实唯一出处，不编造）：`findings/FIXREPORT-U-A.md §4`、`findings/FIXREPORT-U-B1.md §1–§3`、`findings/FIXREPORT-U-B2.md §4–§5`、`findings/FIXREPORT-U-C1.md §9`、`findings/FIXREPORT-U-C2.md §1/§4/§8`、`findings/PLAN-U-B1.md`（D2/D3/D5）、`findings/UX-IMPL-PLAN.md §一`。报告里写不清的条目标"以报告为准/待核实"。
- 维护规则：本文件只增行；`compat/` 台账既有行不动（追加由合并方执行）。
- 总原则（UX-IMPL-PLAN §一）：不改任何持久化业务字段的默认值、不删字段、不删台账行；疑似遗留项只降级展示；下表"影响持久化业务字段"列应全为"否"（写 `ui_state_store` 显示偏好的除外，见"已知边界"）。

## 总表

| ID | 波次 | 位置/文件 | 与上游的差异 | 默认状态 | 影响持久化业务字段 | 可回退方式/如何关闭 | 备注 |
|---|---|---|---|---|---|---|---|
| UI-ACCENT-DEFAULT | U-A | 全局主题（`app_theme.dart` 显示兜底） | 未设强调色时的显示种子 Blue `#2196F3` → TealGreen `#0E6B5E`；仅显示兜底 | 青绿显示 | 否（持久化默认仍 null，测试锁定） | 在主题设置里显式选强调色即覆盖 | 待用户决策 (8) |
| UI-THEME-OPTIONS | U-A | 主题下拉 | 去掉 Aquatic/Desert/Dusk/NightSky（本就无映射）；旧值读回 system | 收敛后选项 | 否 | 以报告为准/待核实（旧值回退即兼容） | UX-IMPL-PLAN 决策 4（方案 B） |
| UI-FONT-12 | U-A | 全局文本 | 裸 12px 统一为 body 12.5（+0.5px） | 新字号 | 否 | 以报告为准/待核实 |  |
| UI-THEME-DIALOG-SECTIONS | U-A | 主题窗 | 新增"外观/语言"分组标题 | 常显 | 否 | 以报告为准/待核实 |  |
| UI-FORM-GAP | U-A | 表单 | fieldGap 10→12、checkbox 行高 28→32 | 新间距 | 否 | 以报告为准/待核实 |  |
| UI-SELECTION-TINT | U-A | 路由表选中底 | `Colors.blue@8%` → `scheme.primary@8%`（随强调色，权重不变） | 跟随强调色 | 否 | 换强调色即变 |  |
| （label 168→148 放弃） | U-A | — | 未实施：二分证实顶掉 fix16 证书下拉 tap，已回退 | 未实施 | 否 | — | 计划偏离备忘，非偏离 |
| UI-ONBOARDING | U-B1 | 帮助菜单＋主壳（`features/onboarding/`、`main_menu.dart`、`main_shell.dart`） | 新增首次启动三步向导（编排既有 ACT-MAIN-016/017/019＋启停/探测）；自动弹仅生产开启、证据环境强制抑制 | 按需弹出 | 否（完成标记写 `ui_state_store`，见边界） | 帮助/重置界面提示可清 hint 状态 |  |
| UI-RESET-HINTS | U-B1 | 帮助菜单 | 新增"重置界面提示"：仅清 hint 类状态（向导完成标记＋确认免打扰），密度/布局/业务不动 | 手动触发 | 否（只清 ui_state） | 以报告为准/待核实 |  |
| UI-SUB-MORE | U-B1 | 主菜单订阅入口 | `更新订阅`直通 ACT-MAIN-022；原 020/021/022/023 四项移入"更多方式"子菜单（折叠非删除） | 主入口单叶＋子菜单 | 否 | 以报告为准/待核实 | 智能默认未做，见待决策 (6) |
| UI-DENSITY-DEFAULT | U-B1 | 节点表（`UiShellState.tableDensity`＋工具条循环按钮） | compact24/standard28/relaxed32，默认 28（相对现状 26 的 +2px） | standard 28 | 否（存 `ui_state_store` layout 区，见边界；非业务字段） | 工具条密度按钮可切 | 待用户决策 (7) |
| UI-STATUS-BADGE（UX2-02＋UX1-02） | U-B1 | 状态条头（`status_badge.dart`） | 新增语义徽标＋聚合胶囊（失败优先；仅失败可点进日志）；只读展示，启停仍只在 `_RuntimeToolbar` | 常显 | 否 | 以报告为准/待核实 | 报告未给 UI- 编号，以报告为准 |
| UI-ERROR-ACTIONS（UX1-04） | U-B1 | 状态条＋失败详情对话框 | 错误可操作化（`errorNextTarget` 保守映射；详情加"查看日志/重试/（禁用）回滚"） | 常显；回滚禁用 | 否 | 以报告为准/待核实 | 回滚禁用见待决策 (4) |
| UI-SPEEDTEST-GLOBAL（UX1-09） | U-B1 | Row B 全局投影＋节点页指示器 | 任意 tab 可见可停（同一 controller 第二视图） | 常显 | 否 | 以报告为准/待核实 | 报告未给 UI- 编号，以报告为准 |
| UI-DANGER-CONFIRM（UX1-03） | U-B1 | 新增 `danger_confirm.dart`＋`undo_snackbar.dart` | 危险确认（目标/影响/上游按钮序/破坏性默认聚焦/Esc 取消/按 key 免打扰）；TUN 仅启用侧 `tun-enable-confirm` 门；forcedChange/PAC 经 `sysproxy-apply-confirm` 门 | 按需弹出 | 否（免打扰写 ui_state，见边界） | 勾"不再提示"可跳过；帮助可重置 |  |
| UI-FOCUS-RING（UX2-08） | U-B1 | 节点表 | 键盘焦点环（前导标记；大表无逐行动画） | 常显 | 否 | 以报告为准/待核实 | 报告未给 UI- 编号，以报告为准 |
| UI-ROUTING-FOLD（UX1-10） | U-B1 | 路由窗工具条 | `routing-import-builtin` 默认折进 `routing-advanced-toggle`（布局高度不变）；选择器分组 caption（`label-sysproxy/label-rule-mode/label-routing-scheme`） | 默认折叠 | 否 | 点"高级…"即展开 |  |
| ACT-TRAY-016 | U-B1 | 托盘 | 新增"重启服务"叶（复用 ACT-MAIN-035）；超限子菜单由空改单 hint 条（哨兵永不作节点 id） | 常显 | 否 | 以报告为准/待核实 |  |
| （托盘"停止"/"测速当前节点"） | U-B1 | — | 未做（dropped，无 ACT／接线即新动作面） | 未实施 | 否 | — | 待用户决策 (5) |
| UI-SETTINGS-HELP | U-B2 | 设置窗四行（`settings_fields.dart`） | 新增 helpText/risk；无 help 时零像素变化 | 常显 | 否 | 以报告为准/待核实 |  |
| UI-SETTINGS-REGROUP | U-B2 | 设置窗显示 Tab | 分组重排（key/label 不变，同 Tab 内移位） | 新分组 | 否 | 以报告为准/待核实 |  |
| UI-FONTSIZE-DEFAULT | U-B2 | 设置窗字体行 | CurrentFontSize=0 显示"当前：默认"，持久化不动 | 显示层 | 否 | 以报告为准/待核实 | UX-IMPL-PLAN 决策 3 |
| UI-SUBEDIT-FOLD | U-B2 | 订阅编辑窗 | 基础/进阶/专家折叠＋3 parity-gap 注 | 默认分级 | 否 | 展开即用 |  |
| UI-HOTKEY-PROBE | U-B2 | 热键窗 | 占用预检 display-only（永不拦截保存） | 常显 | 否 | 以报告为准/待核实 |  |
| UI-UPDATE-GUARD | U-B2 | 更新窗 | 空选跳 bridge 检查（与之前空选报错同一文案） | 守卫 | 否 | 以报告为准/待核实 |  |
| UI-BACKUP-WARN | U-B2 | 备份页 | 三处"先本地备份"警告＋WebDAV 明文/待核实注 | 常显 | 否 | 以报告为准/待核实 |  |
| UI-MONITOR-TIP | U-B2 | 日志/连接视图＋测速两行 | 开关 Tooltip；测速"<10 自动改回 10"注（与 `apply_load_defaults` 一致） | 常显 | 否 | 以报告为准/待核实 |  |
| UI-SETTINGS-SEARCH/DIRTY/RESET | U-B2 | 设置窗顶栏＋各组 | 提交式搜索（绝不 live-filter）、dirty 圆点（组级 modified-only 过滤）、按组恢复默认（1 次确认＋saveGroup；恢复即写该组上游默认值本身） | 常显 | 否（恢复默认写的是上游默认值，非改默认） | 以报告为准/待核实 | 行级隐藏未做（§4-9 注，fix16 tap 风险） |
| UI-UX3-09 | U-B2 | 设置窗内 UX3 字段 | ~40 行文案；专家开关本地默认开；进阶节默认展开；Protocol 保持可编辑＋灰注；四处二次确认；缓存开关移节；Padding 只读灰行 | 专家门默认开／进阶默认展开 | 否 | 专家开关可手动关 | 待用户决策 (1)(2) |
| UI-PRESET-10 | U-B2 | 设置窗顶栏预设对话框 | 场景 A/B/C＋新 D 取值表＋强制 diff 预览＋禁用撤销；SysProxyType 永不写、TUN 只写 false、端口/凭据无写入项；未调 DNS `applyPreset` | 对话框 | 否（只写 desired） | 以报告为准/待核实 | 撤销禁用见待决策 (3) |
| UI-SETTINGS-TRISTATE | U-B2 | 设置窗底栏 | 三态行（草稿 n 处/已保存未应用/已应用 rev）；Cancel 直接丢弃（fix08 契约），仅 Esc 脏确认；Enter 仅单行保存 | 常显 | 否 | 以报告为准/待核实 |  |
| UI-HELP-BITS | U-C1 | 新增 `features/routing/help_bits.dart` | 共享展示小件（HelpText/HelpTip/默认折叠 FoldSection/只读 LegacyRow） | 基础件 | 否 | —（被引用方） |  |
| UI-DNS-COPY | U-C1 | DNS 窗三 Tab＋preset 行 | 行下说明全覆盖；preset 离线说明（`applyPreset` 语义零改动） | 常显 | 否 | 以报告为准/待核实 |  |
| UI-DNS-CUSTOM-WARN | U-C1 | DNS 自定义 Tab | 双开顶掉警示（既有互斥的可视化） | 常显 | 否 | 以报告为准/待核实 |  |
| UI-ROUTING-COPY | U-C1 | 路由窗策略行（嵌入式＋独立窗）＋导购小字 | tooltip＋"日常=绕过大陆/省心=全局"小字 | 常显 | 否 | 以报告为准/待核实 |  |
| UI-RULE-EXPERT-FOLD | U-C1 | 规则详情对话框（两窗共享） | 协议/入站标签默认折叠＋有值徽注/自动展开 | 默认折叠（有值展开） | 否 | 展开即用 |  |
| UI-LEGACY-ROW | U-C1 | 方案编辑器（嵌入式＋独立窗） | 方案级策略只读公示＋待核实（保存透传零改动） | 只读行 | 否 | 以报告为准/待核实 |  |
| UI-PRESET-DLG | U-C2 | 主菜单设置组＋向导第四键（`UI-CLEANUP` 后＋`UI-MAIN` 前） | 新增"推荐配置…"纯编排入口（场景→强制 diff→只写预览列＋可选路由切换；撤销三处禁用＋说明；取消零写入） | 按需弹出 | 否（只写 desired；`compat/actions.yaml` 只追加一行） | 不点即无 |  |

## 分节

### U-A（地基 token 与显示修正）

见总表 UI-ACCENT-DEFAULT…UI-SELECTION-TINT（`FIXREPORT-U-A.md §4`）。对比度 light/dark 16/16 ≥ 4.5:1（同报告 §3）。label 168→148 已放弃（fix16 回归证据，同报告 §4-6、§5）。

### U-B1（状态条/菜单/主窗口/表格交互）

见总表 UI-ONBOARDING…ACT-TRAY-016（`FIXREPORT-U-B1.md §1、§3`）。失败详情"回滚"、节点删除撤销、备份恢复危险门因无后端快照 API 而禁用/阻塞（同报告 §4）。托盘"停止"/"测速当前节点" dropped（同报告 §2，见待决策 (5)）。t17/既有断言更新 3 处（同报告 §5，非偏离）。

### U-B2（设置线：文案/分级/搜索/预设/三态）

见总表 UI-SETTINGS-HELP…UI-SETTINGS-TRISTATE（`FIXREPORT-U-B2.md §4`）。设计要点归档同报告 §5：专家门默认开的理由（sp_24/flux 邻居测试）、进阶默认展开、三分组不拆分、UB2-10 四不写＋强制 diff、UB2-11 Esc/Enter 语义。条件 A–F 落点同报告 §4 末。

### U-C1（dns/routing 窗展示层）

见总表 UI-HELP-BITS…UI-LEGACY-ROW（`FIXREPORT-U-C1.md §9`）。r3 陈旧断言系 U-B1 UX1-10 折叠所致、已随测修正（同报告 §3，非产品偏离）。需后端项均未做按钮（同报告 §4）。

### U-C2（推荐配置菜单入口与向导第四键）

见总表 UI-PRESET-DLG（`FIXREPORT-U-C2.md §1、§8`）。写链/约束/禁用态全沿 U-B2 条件 E；撤销三处禁用＋说明（同报告 §4）；U-B1 遗留未入账行不在本批回补（同报告 §8）。

## 待用户决策项

1. **专家开关默认开**：偏离 UX3-04 默认关，等于默认状态下不起门禁（`FIXREPORT-U-B2.md §5` 明确事实；理由：sp_24/flux 系测试直驱专家行）。改回默认关需评估邻居测试。
2. **设置页进阶节默认展开**：同上理由。改回默认折叠同需评估。
3. **推荐预设撤销禁用态**：缺快照 API，需 Rust 后端另案（UX3-12；`FIXREPORT-U-B2.md §4` 条件 E、`FIXREPORT-U-C2.md §4`）。后端落地前保持禁用＋说明。
4. **删除撤销／失败详情"回退上一个可用节点"／备份恢复危险门禁用态**：均需后端（`FIXREPORT-U-B1.md §4`、`PLAN-U-B1.md` D5：`reconcileActiveAfterRemoval` 不记历史）。保持禁用＋说明，不伪装可用。
5. **托盘"停止"与"测速当前节点"未做**：dropped（`FIXREPORT-U-B1.md §2`）。做即新动作面，需另案。
6. **自动代理规则智能默认未做**：UX1-07 的"智能默认"需订阅切片定规则，本波不做、记后续项（`PLAN-U-B1.md` D3）。现主入口直通 ACT-MAIN-022，四项在子菜单。
7. **密度默认 26→28**：`standard=28`，相对现状 26 的 +2px（`PLAN-U-B1.md` D2、`FIXREPORT-U-B1.md §1`）。改回 26 即改回现状。
8. **默认主色换青绿**：未设强调色显示种子 Blue → TealGreen（`FIXREPORT-U-A.md §4-1`；持久化默认仍 null，已存用户外观不变）。

## 已知边界（UX 阶段新增 UI-* 的写入面）

- UX 阶段新增 UI-* **均不写持久化业务字段**（上表"影响持久化业务字段"列全"否"；按组恢复默认写的是该组上游默认值本身，非改默认）。
- 仅写 `ui_state_store` 的三类（据报告原文）：① 首启向导完成标记（`OnboardingHints` 分区，可经"重置界面提示"清除，`FIXREPORT-U-B1.md §1`）；② 危险确认"不再提示"免打扰（存 ui_state，经 UI-RESET-HINTS 可重置，同上）；③ 表格密度（`UiShellState.tableDensity`＋`ui_state_store` layout 区持久，`PLAN-U-B1.md` D2、`FIXREPORT-U-B1.md §1`）。
- 真实代理流量不过 Flutter/FRB、UI 不拥有内核运行状态、开关只写 desired 的硬约束未动（各报告门禁表）。
