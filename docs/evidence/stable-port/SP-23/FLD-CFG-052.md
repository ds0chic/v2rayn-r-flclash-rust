# SP-23.FLD-CFG-052 — GrpcItem.IdleTimeout

状态：implemented（实例登记完成；正式编辑入口缺失 + 真实 gRPC 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-052（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：GrpcItem IdleTimeout（可空，默认 60）→保存→
同 revision plan→xray gRPC `idle_timeout` 及适用 core 映射；
单位/缺省/合法边界按冻结语义，真实 gRPC session 空闲行为验证。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-052；leaf；platform_scope=all；original_type=int?；original_default=60。
关联：FLD-CFG-053/054（同 gRPC 组）、FLD-CFG-055（InitialWindowsSize，待下批）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:60 :: GrpcItem.IdleTimeout`；
只读核对原版 gRPC 空闲超时语义（可空）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正整数 / null（缺省 60）；
非法→拒绝且旧值不变。注意：`option_setting_window.dart` 暂无 GrpcItem 编辑器
（5 个 tab 均无 gRPC 区，`:222-226,499-825` 实测），值经 DTO/默认与合成修订流入；
正式编辑入口缺失已登记为缺口。
持久化 save；生效 restart_core（`settings_timing.rs:116`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:398`（投影）、
xray/sing-box gRPC transport 装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认 Some(60)（`domain/src/settings.rs:365`）；
null 缺省语义；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:351`（`Option<i32>`，默认 Some(60) `:365`）。
- DTO：`bridge_api/src/api/settings.rs:198,208,220` + FRB wire；
  Dart 侧 `api/settings.dart:392` + wire。
- 正式入口：缺失（缺口，见上）；Dart 默认 `settings_defaults.dart:86-90`（`:87 IdleTimeout=60`）。
- 投影：`codegen.rs:398 base.grpc.idle_timeout`。
- 单测：`codegen.rs:1155 r4_13_s09_grpc_item_reaches_codegen`
 （`:1157 IdleTimeout=Some(11)`、`:1162` 断言）；
  `config_codegen/tests/xray_transport_security.rs:249 xray_grpc_reality`
 （`:259-260` 设值、`:287` 落盘断言）；
  `config_codegen/tests/singbox_transport_security.rs:45 singbox_raw_http_httpupgrade_grpc`
 （`:86` 设值、`:101` 落盘断言）；
  `domain/src/settings.rs:1550 all_groups_round_trip_with_unknown_keys`（`:1558` 回环）；
  Dart `r4_13_s09_contract_test.dart:12 GrpcItem save -> reopen`（`:16 IdleTimeout=11`→`:22`）、
  `:28 absent GrpcItem fields keep the frozen defaults`（`:33` 默认 60）。
- 缺口：正式编辑入口缺失；正式入口→FRB→持久化→重开→真实 gRPC session 完整验收未跑
 （CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 11→plan grpcSettings.idle_timeout=11→core 校验；
负向 null→缺省 60、坏类型→拒绝。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s09_grpc_item_reaches_codegen`）+ `cargo test -p config_codegen --locked`
（`xray_grpc_reality`、`singbox_raw_http_httpupgrade_grpc`）；
补正式编辑入口→FRB→保存→同 revision plan→真实 gRPC session→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-052.md`。

完成条件：正式编辑入口 + plan→真实 gRPC session 闭环 + 重开；仅投影/回环单测不算。

发现接口缺口时的处理：正式编辑入口缺失 + 真实会话缺口已登记；归属 SP-24。
