# SP-23.FLD-CFG-112 — SpeedTestItem.SpeedTestPageSize

状态：implemented（实例登记完成；多页完整结果/边界/空页/取消验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-112（主 owner SP-23；消费者归属 SP-29）。

本次唯一用户流程：设置窗改测速分页规模（SpeedTestItem.SpeedTestPageSize，
int?，默认 null）→保存→immediate→测速任务按页/批次执行，
多页节点完整结果，不得漏节点或重复启动。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-112；leaf；platform_scope=all；original_type=int?；original_default=null。
关联：FLD-CFG-109/113（同调度组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SpeedTestItem.SpeedTestPageSize`；
只读核对原版分页语义（可空）。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正 int / null（缺省）；
空页/取消→不漏不重。持久化 save；生效 immediate。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-29）：`crates/application/src/speedtest.rs`
（`:105` `page = item.speed_test_page_size.unwrap_or(0).max(0)`）、
`crates/bridge_api/src/api/settings.rs`（`:539/:553/:569`）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 null；缺省 0 语义；取消释放；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:141 'SpeedTestPageSize': null`。
- DTO：`bridge_api` `speed_test_page_size`（`settings.rs:539/553/569`）+
  FRB wire（`frb_generated.rs:9618/13628/16792` 在位）。
- 调用链：保存→`speedtest.rs:105` 归一化→任务分页执行。
- 单测：`codegen.rs:1355`（`speed_test_page_size: Some(5)` 透传在位）。
- 缺口：正式窗分页编辑入口行号未逐行核对（同区 106/109 已核）；
  多页完整结果/边界/空页/取消验收未跑（CSV current_gap）。

测试夹具和原版预期：合成页规模；正向多页→完整结果无遗漏；
负向空页/取消→不重复启动（待验）。

本次必须通过的命令/真实场景（SP-29，未运行）：`cargo test -p application --locked`
（speedtest 分页）；补正式窗→FRB→保存→多页真实测速→取消→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-112.md`。

完成条件：保存、多页完整结果和取消语义三者一致；仅落盘不算。

发现接口缺口时的处理：分页验收缺口已登记；归属 SP-29。
