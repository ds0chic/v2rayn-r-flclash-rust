# SP-23.FLD-CFG-153 — Fragment4RayItem.MaxSplit

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-153（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：输入 `MaxSplit=1-3`（原版合法范围串）→保存成功→restart_core→
wire `maxSplit=1`（冻结：只取首整数）→xray 校验通过→重开显示仍为 `1-3`（raw 保留）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/07; 冻结Utils.TryParseMaxSplit与V2rayOutboundService; core版本能力`。

对应 ID：FLD-CFG-153；leaf；platform_scope=all；original_type=string；original_default="0"。
关联：032/033、150/151/152，SD-09。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Fragment4RayItem.MaxSplit`；
`OptionSettingViewModel.cs:303`（`Utils.TryParseMaxSplit(input,0,10000)`，接受 `1-3`）；
`Utils.cs:634`；`V2rayOutboundService.cs:831-843`；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：
合法：单值/`1-3` 类范围串（原版 `TryParseMaxSplit`）；空按默认；非法类型/坏串拒绝并保留旧值。
UI 草稿 `option_setting_window.dart:358-364,761-762`（当前整数-only，错）。
提交/持久化 save、生效 restart_core、取消/错误语义同 150；
校验失败 `E_FIELD_FORMAT field=Fragment4RayItem.MaxSplit` 必须可恢复（旧值不受损）。

允许修改的模块（SP-24）：`crates/application/src/settings.rs:91-99`
（按 `TryParseMaxSplit` 重写校验：接受范围串、保留 raw）、
`option_setting_window.dart:358-364`（同）、wire 侧 `config.rs:258-262` 保留首整数语义。

禁止改变的已有行为：wire 只取首整数的冻结语义；raw 范围保留（不得存成 `1` 丢信息）；
其它 fragment 叶行为。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:778,795`（`max_split: Option<String>`，默认 `"0"`）；
  legacy 回填 `:1119-1120`。
- 校验门（错）：`application/src/settings.rs:92-93`
  `!max_split.is_empty() && max_split.parse::<u32>().is_err()` → `"1-3"` 被拒
  （`E_FIELD_FORMAT/error.fragment_maxsplit`，`:282-284` 有非数字拒绝单测）。
- DTO：`bridge_api/src/api/settings.rs:835,847,861`（`max_split: Option<String>`）。
- 发射：`config_codegen/src/xray/config.rs:253-269`（首段整数，`unwrap_or(0)`）。
- 缺口 G-02：校验（整数-only）与 wire（首整数）语义分裂；原版合法 `1-3` 存不进去。

测试夹具和原版预期：正向 `1-3`（wire=1）/单值；边界 `0/10000`；负向倒序/负数/多段/
空/非法类型按 `TryParseMaxSplit` 冻结行为；UI/Rust 判定一致。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（MaxSplit 矩阵）；正式窗→FRB→保存→restart_core→JSON diff→xray 校验→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-153.md`。

完成条件：范围串可存 + wire 首整数 + raw 保留 + 校验 + 重开；只放宽校验而不保
范围语义不算（审计 CP-SET-06 原话）。

发现接口缺口时的处理：G-02 已登记，SP-24 按 `TryParseMaxSplit` 统一 UI/Rust 判定。
