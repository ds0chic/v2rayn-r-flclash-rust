# SP-23.FLD-CFG-158 — UiItem.WindowSizeItem[].Height

状态：implemented（实例登记完成；实际窗口几何消费未证，不写 verified）。
任务 ID：SP-23.FLD-CFG-158（主 owner SP-23；消费者归属 SD-06 UI/持久化方向）。

本次唯一用户流程：窗口几何 `Height`（int，默认 0，随同组 TypeName/Width
一并持久化）→save→独立重开→同 TypeName 窗口几何恢复（实际消费未证）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI 原型/布局与重开；冻结 legacy 迁移`。

对应 ID：FLD-CFG-158；leaf；platform_scope=all；original_type=int；original_default=0。
关联：FLD-CFG-156/157（同 WindowSizeItem 组，未登记）、FLD-CFG-082
（WindowSizeItem 容器，`settings_timing.rs:310`），SD-06/SD-18/17。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: WindowSizeItem.Height`；
只读核对原版几何语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `8b533f0`（全 `8b533f0f61af608564d3e0a262e2da7df5e4d9ee`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 int 高度；
零/缺省→默认 0（`WindowState::default`）。持久化 save；生效 immediate
（`settings_timing.rs:315`）。取消/错误语义按 SP-02/SP-12。
无平台写入；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SD-06 方向）：`crates/domain/src/entities.rs`
（`:271-282` WindowState，height `:282`）、`crates/domain/src/settings.rs`
（`:489` window_size_item，`:514` 默认为空 Vec）、
`crates/bridge_api/src/api/settings.rs`（WindowSizeItemDto 往返 + FRB wire）。
本卡未改生产代码。

禁止改变的已有行为：原版 TypeName/Width/Height 三元组与 PascalCase 形态；
零值跳过序列化（`skip_serializing_if`）保持上游 guiNConfig 形状；
stale revision 拒绝；保存/运行分离；未知键保留。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` WindowState.height（`entities.rs:282`，`settings.rs:489`）。
- DTO：WindowSizeItemDto 往返在位 + FRB wire
  （`frb_generated.rs:8027/10110/10318/14485/15799/17158/17282`）。
- 时机：`settings_timing.rs:315` Immediate（156/157 同组 `:316/317`）。
- 缺口：CSV current_gap——canonical 子项已存但未被实际 UI 消费；
  实际窗口几何恢复验收未跑。

测试夹具和原版预期：合成 TypeName+Width+Height 三元组；正向两组不同
有效高度→重开恢复；负向零/缺省→默认 0。

本次必须通过的命令/真实场景（SD-06 方向，未运行）：`cargo test -p domain --locked`
（settings round-trip）+ 正式入口→FRB→保存→独立重开→几何恢复断言。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-158.md`。

完成条件：保存、重开恢复一致；仅 DTO 往返不算。

发现接口缺口时的处理：实际 UI 消费缺口已登记；156/157 待后续批次。
