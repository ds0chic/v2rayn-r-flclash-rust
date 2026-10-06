# SP-23.FLD-CFG-150 — Fragment4RayItem.Packets

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-150（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：分片设置 `Packets=tlshello`（非默认）→一次保存→restart_core→
xray fragment 出站 `settings.packets=tlshello`→真实 xray 配置校验通过→重开仍为该值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/07; 冻结Utils.TryParseMaxSplit与V2rayOutboundService; core版本能力`。

对应 ID：FLD-CFG-150；leaf；platform_scope=all；original_type=string；original_default="tlshello"。
关联：FLD-CFG-032/033（EnableFragment/EnableFinalFragment 开关）、151/152/153，SD-09。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Fragment4RayItem.Packets`；
wire 语义 `V2rayOutboundService.cs:831-843`；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 raw 字符串/空（空回默认）；
UI 草稿 `option_setting_window.dart:728,737`、`settings_defaults.dart:192`。
SP-02 提交语义（expectedRevision/datasetEpoch/mutationId，未知结果先 query）。
持久化 save；生效 restart_core（`settings_timing.rs:108` 同组）。
取消/错误语义按 SP-02/SP-12。

允许修改的模块（SP-24）：`crates/config_codegen/src/xray/config.rs:248-273`
（fragment 节构造）、`crates/application/src/codegen.rs:447`（透传）。
本卡未改生产代码。

禁止改变的已有行为：原版 raw 字符串格式；EnableFragment×FinalFragment×TLS 真值组合
与冻结覆盖顺序；legacy 迁移语义。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` `Fragment4RayItem`（`packets`，legacy 回填 `:1114-1120`）。
- 透传：`application/src/codegen.rs:443` 组拷贝（`codegen.rs:447` 为 max_split 同组行）。
- DTO：`bridge_api/src/api/settings.rs` Fragment DTO（含 `:835 max_split` 同结构）。
- 发射：`config_codegen/src/xray/config.rs:248-273`（packets/lengths/delays/maxSplit 同函数）；
  历史单测 `config_codegen/tests/xray_global.rs:140` 仅置 max_split，不能当本叶证据。
- 审计态：implemented（静态）/ actual_effect 未验证。

测试夹具和原版预期：合成出站；正向 `tlshello` 与自定义串各一；负向空串（回默认）/
超长/非法字符按冻结处理；与开关关组合（关时不发射 fragment，见 032/033）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`；
正式窗→FRB→保存→restart_core→JSON diff→真实 xray 校验→独立重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-150.md`。

完成条件：两正向 + 负向 + 校验 + 重开；只 round-trip 不算。

发现接口缺口时的处理：本叶透传/发射存在，缺端到端；MaxSplit 分裂见 G-02（153）。
