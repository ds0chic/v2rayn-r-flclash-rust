# SP-23.FLD-CFG-055 — GrpcItem.InitialWindowsSize

状态：implemented（实例登记完成；正式编辑入口缺失 + 真实 gRPC 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-055（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：GrpcItem InitialWindowsSize（可空 int，默认 0）→保存→
同 revision plan→xray gRPC `initial_windows_size` 及适用 core 映射；
窗口参数在实际 gRPC transport 发送路径按冻结语义生效。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-055；leaf；platform_scope=all；original_type=int?；original_default=0。
关联：FLD-CFG-052/053/054（同 gRPC 组，本批 054 已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GrpcItem.InitialWindowsSize`；
只读核对原版 gRPC 初始窗口语义（可空 int，零值/合法边界）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 int/null（缺省 0）；
零值/正值按冻结语义透传，非法类型→拒绝且旧值不变。正式编辑入口缺失
（同 052/054，`option_setting_window.dart` 无 gRPC 区），值经 DTO/默认与合成修订流入，已登记缺口。
持久化 save；生效 restart_core（`settings_timing.rs:117`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:401`（投影
`base.grpc.initial_windows_size`）、xray/sing-box gRPC transport 装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认/缺省语义；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` GrpcItem 段（`Option<i32>`，Dart 默认
  `settings_defaults.dart:90 InitialWindowsSize=0`）。
- DTO：`bridge_api/src/api/settings.rs` GrpcItem 段 + FRB wire；
  Dart 侧 `api/settings.dart` + wire。
- 投影：`codegen.rs:401 base.grpc.initial_windows_size`。
- 单测：`codegen.rs:1160 initial_windows_size=Some(65535)`、`:1165` 断言；
  Dart `r4_13_s09_contract_test.dart:19/25/36`（同组 save→reopen 与冻结默认）。
- 缺口：正式编辑入口缺失；窗口参数真实 gRPC session 发送路径验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 65535→plan 落值→core 校验；
负向 null→缺省 0、坏类型→拒绝。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（grpc codegen 合同）+ `cargo test -p config_codegen --locked`（gRPC transport）；
补正式编辑入口→FRB→保存→同 revision plan→真实 gRPC session→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-055.md`。

完成条件：正式编辑入口 + plan→真实 gRPC session 闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：正式编辑入口缺失 + 真实会话缺口已登记；归属 SP-24。
