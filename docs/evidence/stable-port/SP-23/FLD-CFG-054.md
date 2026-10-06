# SP-23.FLD-CFG-054 — GrpcItem.PermitWithoutStream

状态：implemented（实例登记完成；正式编辑入口缺失 + 真实 gRPC 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-054（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：GrpcItem PermitWithoutStream（可空 bool，默认 false）→保存→
同 revision plan→xray gRPC `permit_without_stream` 及适用 core 映射；
布尔组合在实际 gRPC transport 配置/会话中按冻结语义生效。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-054；leaf；platform_scope=all；original_type=bool?；original_default=false。
关联：FLD-CFG-052/053（同 gRPC 组）、FLD-CFG-055（InitialWindowsSize，待下批）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:62 :: GrpcItem.PermitWithoutStream`；
只读核对原版 gRPC 无流许可语义（可空 bool）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false/null（缺省 false）；
非法→拒绝且旧值不变。正式编辑入口缺失（同 052，`option_setting_window.dart`
5 tab 无 gRPC 区），值经 DTO/默认与合成修订流入，已登记缺口。
持久化 save；生效 restart_core（`settings_timing.rs:118`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:400`（投影）、
xray/sing-box gRPC transport 装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认 Some(false)（`domain/src/settings.rs:367`）；
null 缺省语义；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:355`（`Option<bool>`，默认 Some(false) `:367`）。
- DTO：`bridge_api/src/api/settings.rs:200,210,222` + FRB wire；
  Dart 侧 `api/settings.dart:394` + wire。
- 正式入口：缺失（缺口）；Dart 默认 `settings_defaults.dart:89 PermitWithoutStream=false`。
- 投影：`codegen.rs:400 base.grpc.permit_without_stream`。
- 单测：`codegen.rs:1155 r4_13_s09_grpc_item_reaches_codegen`
 （`:1159 PermitWithoutStream=Some(true)`、`:1164` 断言）；
  `config_codegen/tests/xray_transport_security.rs:249 xray_grpc_reality`
 （`:261` 设值、`:294` 落盘断言）；
  `config_codegen/tests/singbox_transport_security.rs:45 singbox_raw_http_httpupgrade_grpc`
 （`:88` 设值、`:111` 落盘断言）；
  Dart `r4_13_s09_contract_test.dart:12/28`（同组 save→reopen 与冻结默认）。
- 缺口：正式编辑入口缺失；布尔组合真实 gRPC transport 会话验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 true→plan permit_without_stream=true→core 校验；
负向 null→缺省 false、坏类型→拒绝。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s09_grpc_item_reaches_codegen`）+ `cargo test -p config_codegen --locked`
（`xray_grpc_reality`、`singbox_raw_http_httpupgrade_grpc`）；
补正式编辑入口→FRB→保存→同 revision plan→真实 gRPC session→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-054.md`。

完成条件：正式编辑入口 + plan→真实 gRPC session 闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：正式编辑入口缺失 + 真实会话缺口已登记；归属 SP-24。
