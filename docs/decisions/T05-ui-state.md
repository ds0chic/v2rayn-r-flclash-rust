# T05 决策 — 本地 UI 状态文件 `ui_state.json` 与迁移计划

状态：T05 实施的可回滚过渡方案。T02 起迁移到 AppEngine 持久化。

## 背景

T05 需要一个跨重启的本地 UI 状态存储，覆盖：
- 主窗口布局方向（`EGirdOrientation` 语义）与分隔条比例；
- 节点表列顺序 / 显隐 / 列宽；
- 主题模式（浅/深）；
- 状态栏中纯 UI 的选择（系统代理四模式索引）。

上游对应键（`compat/layouts.yaml`）为 `UIItem.MainGirdOrientation`、`UIItem.MainGirdHeight1/2`、`UIItem.MainColumnItem`（Name/Width/DisplayIndex，Width<0 表示隐藏）、`UIItem.CurrentTheme`、`SystemProxyItem`。T01 已有 `v2raynr_ui_state.json` 草稿（仅列宽与窗口尺寸）。

## 决定

1. 文件位置：可执行文件同目录 `ui_state.json`（release 即 `build/windows/x64/runner/Release/`）。开发期视“apps/desktop 目录下 `ui_state.json`”为同一语义。
2. 访问层：复用 `UiStateStore` 抽象（`lib/features/profiles/ui_state_store.dart`），新增通用 `loadSection/saveSection`；新增 `MemoryUiStateStore.sections` 供测试。
3. 文档结构（JSON）：
   ```json
   {
     "column_widths": { "Remarks": 150 },
     "window": { },
     "column_layout": {
       "order": ["ConfigType", "Remarks", "..."],
       "visible": { "IpInfo": false },
       "widths": { "Remarks": 150 }
     },
     "layout": { "mode": "vertical", "horizontal_split": 0.5, "vertical_split": 0.5 },
     "theme": { "mode": "light" },
     "status_ui": { "system_proxy": 2 }
   }
   ```
   - `column_layout.order` 即显示顺序，含隐藏列，映射 `MainColumnItem` 的 DisplayIndex；
   - `visible=false` 映射 Width<0 的隐藏语义；
   - `layout.mode ∈ {horizontal,vertical,tab}` 映射 `MainGirdOrientation`；
   - `layout.horizontal_split`/`vertical_split` ∈ [0.1,0.9] 为星值比例，等效 `MainGirdHeight1/2`。
4. 兼容：若存在旧 `column_widths`（T01 草稿，现仍位于同一文件顶层），`column_layout` 缺失时按旧键回填宽度；不回写、不删除旧键。窗口尺寸仍由 T01 runner 的 `v2raynr_window_state.ini` 管理，T05 不改。
5. 失败语义：读写异常一律吞掉并以默认值运行，UI 不因 I/O 失败阻塞。

## 迁移计划（T02+）

1. 在 `crates/persistence` 建立窗口/布局/列/主题设置实体，稳定 ID 与强类型字段。
2. AppEngine 暴露读写 API；Flutter 侧 `UiStateStore` 增补 AppEngine 实现，或在启动时一次性导入 `ui_state.json` → AppEngine，随后以 AppEngine 为唯一真源。
3. 一次性迁移：读取 `ui_state.json` → 转成 `UIItem.*`/`SystemProxyItem` 语义写入 AppEngine → 记录迁移报告；保留原文件只读备份。
4. 迁移完成后 `ui_state.json` 降级为兼容导入格式，不再作为运行时真源。

## 约束

- 不修改 `crates/**` 与另一代理负责的 `lib/bridge/**`（本决策仅描述迁移方向，不在 T05 实施）。
- 状态栏不得据此文件伪造运行状态：`ui_state.json` 只保存用户 UI 选择，运行速率/节点/TUN/路由等真实值必须来自后端。
