# SP-23.FLD-CFG-107 — SpeedTestItem.SpeedTestUrl

状态：implemented（实例登记完成；真实下载测速字节速度验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-107（主 owner SP-23；消费者归属 SP-29）。

本次唯一用户流程：设置窗改测速下载 URL（SpeedTestItem.SpeedTestUrl，
string，默认 `https://cachefly.cachefly.net/50mb.test`）→保存→immediate→
Rust 速度测试真实下载请求走 shared HttpPolicy，计真实字节速度，
禁止把 ping 当下载测速。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`；关联 SD-10（HttpPolicy）。

对应 ID：FLD-CFG-107；leaf；platform_scope=all；original_type=string；
original_default=`https://cachefly.cachefly.net/50mb.test`。
关联：FLD-CFG-106（同测速组，已有证据）、108/109（本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SpeedTestItem.SpeedTestUrl`；
只读核对原版下载测速语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 HTTPS URL；
重定向/错误内容/取消→真实 worker 语义，不悬挂。正式入口
`option_setting_window.dart:996-998`
（`_str/_set('SpeedTestItem','SpeedTestUrl')`）。持久化 save；
生效 immediate。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-29）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:996-998`）、`apps/desktop/lib/features/profiles/profiles_controller.dart`
（`:664` 透传 `speedTestUrl`）、Rust speedtest worker。
本卡未改生产代码。

禁止改变的已有行为：原版默认 URL；下载≠ping 区分；取消释放语义；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:136` 默认 URL。
- DTO：FRB wire `speed_test_url`（`frb_generated.rs:4420/9613/13623/16787` 在位）。
- 调用链：正式窗 `_set`→保存→`profiles_controller:664` 透传→worker 下载。
- 集成置值：`ux_test02_https_speedtest_test.dart:350`、
  `ux_speedtest_diag_test.dart:245`（合成本地 URL 置换）。
- 缺口：合成本地定长下载/重定向/错误内容/取消真实字节速度验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成 URL；正向本地定长下载→真实字节速度；
负向错误内容/取消→不悬挂（待验）。

本次必须通过的命令/真实场景（SP-29，未运行）：`cargo test -p application --locked`
（speedtest worker）+ `flutter test`（测速诊断）；补正式窗→FRB→保存→
合成下载真实测速→取消→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-107.md`。

完成条件：保存、真实字节速度和取消不悬挂三者一致；仅落盘不算。

发现接口缺口时的处理：真实下载验收缺口已登记；归属 SP-29。
