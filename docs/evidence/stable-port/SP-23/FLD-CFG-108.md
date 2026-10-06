# SP-23.FLD-CFG-108 — SpeedTestItem.SpeedPingTestUrl

状态：implemented（实例登记完成；真实代理请求 delay 与 TCP/HTTP 区分验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-108（主 owner SP-23；消费者归属 SP-29）。

本次唯一用户流程：设置窗改 ping 目标（SpeedTestItem.SpeedPingTestUrl，
string，默认 `https://www.google.com/generate_204`）→保存→immediate→
Rust delay/ping worker 按正式 codegen speedPing URL 上下文发真实请求，
分清 TCP 与 HTTP delay，选定节点走真实代理请求。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`；关联 SD-10（HttpPolicy）。

对应 ID：FLD-CFG-108；leaf；platform_scope=all；original_type=string；
original_default=`https://www.google.com/generate_204`。
关联：FLD-CFG-106/107（同测速组，106 已有证据、107 本批），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SpeedTestItem.SpeedPingTestUrl`；
只读核对原版 ping 目标语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 URL；
超时/错 URL→分类准确，不悬挂。正式入口
`option_setting_window.dart:1002-1004`
（`_str/_set('SpeedTestItem','SpeedPingTestUrl')`）。持久化 save；
生效 immediate。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-29）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:1002-1004`）、`apps/desktop/lib/features/profiles/profiles_controller.dart`
（`:666` 透传）、`apps/desktop/lib/features/monitor/monitor_bridge.dart`
（`:48` 推送 delay-probe URL）、`apps/desktop/lib/bridge/api/monitor.dart`
（`:107` 置 URL 语义）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 URL；TCP 与 HTTP delay 区分；取消释放语义；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:137` 默认 URL。
- DTO：FRB wire `speed_ping_test_url`（`frb_generated.rs:4421/9614/13624/16788` 在位）。
- 调用链：正式窗 `_set`→保存→`profiles_controller:666` 透传→
  `monitor_bridge:48` 推送→worker 探测。
- 单测/集成：`r4_13_s20_contract_test.dart:18-19` 置值；
  `ux_test02_https_speedtest_test.dart:349`、`ux_speedtest_diag_test.dart:246`
  合成本地 ping 置换。
- 缺口：本地响应/超时/错 URL/选定节点真实代理请求 + TCP/HTTP 区分验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成 ping URL；正向本地响应→真实 delay；
负向超时/错 URL→分类准确（待验）。

本次必须通过的命令/真实场景（SP-29，未运行）：`flutter test`
（`r4_13_s20_contract`、测速诊断）；补正式窗→FRB→保存→真实 delay→
选定节点代理请求→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-108.md`。

完成条件：保存、真实 delay 和代理请求区分三者一致；仅落盘不算。

发现接口缺口时的处理：真实 delay 验收缺口已登记；归属 SP-29。
