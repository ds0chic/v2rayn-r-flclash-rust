# R3-PROF-05 — UDP 测速忽略 session，返回宿主直连延迟

状态：`implemented`（UDP 探针改为经节点测试核的 SOCKS5 UDP ASSOCIATE，Rust 单 crate 受控 SOCKS5 UDP server 断言目标走 `session.port`；未运行真实内核对端，故不写 `verified`）。

任务 ID：R3-PROF-05

本次唯一用户流程：对选中节点执行 UDP 测速，延迟必须来自经该节点测试核转发的探测；宿主本机 UDP 直连延迟不得再作为节点结果；支持位按真实能力门控。

前置任务及已验证证据：冻结 v2rayN 7.25.4 `7d6a967...`；上游 `ServiceLib.UdpTest/UdpTestService.cs:91-153`（`SendUdpRequestAsync` 经 `Socks5UdpChannel`，两次取最小值 + 校验响应）与 `ServiceLib/Service/SpeedtestService.cs`（先加载节点测试核）。复核来源 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` RE-PROF-08 / R3-PROF-05（HEAD `7edf1ee`）。

对应 feature / field / action / layout ID：`ACT-PROF` UDP 测速、`EConfigType`/UDP 支持位 `RE-PROF-08`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 之外的本机冻结源 `ServiceLib.UdpTest/UdpTestService.cs`、`ServiceLib.UdpTest/Socks5UdpChannel`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`TestSession.port`（节点测试核本地 SOCKS 端口）、`UdpTestTarget`（关键字或 `host:port`）、超时、取消令牌。
- 输出：往返毫秒（≥1）；失败返回 `None`，UI 呈结构化失败，不伪造延迟。
- 错误/取消：连接/握手/超时/取消均安全返回 `None`。
- 权限：仅本机 loopback SOCKS/UDP；不启动额外内核（测试核由既有 net-host 会话提供）。

允许修改的模块：`crates/application/src/speedtest.rs`、`crates/bridge_api/src/api/speedtest.rs`、`docs/evidence/recheck-fixes/R3-06-PROF-05-07/**`、本卡。

禁止改变的已有行为：不改 `SpeedTestSession` trait 形状与既有 TCP/RealPing/下载路径；不改 RE-PROF-10 草稿预览主体；不删除 UDP 入口或降分母。

测试夹具和原版预期：受控 SOCKS5 UDP server（同时绑定 TCP+UDP 于同一 ≥11808 端口）应答 `UDP ASSOCIATE`、解析并记录 SOCKS5 UDP 头中的目标、原样回显；断言 `udp_ping_via_socks` 返回延迟且记录目标等于请求目标（RFC 5737 文档地址）。域名目标编码单元断言。原版预期为经节点代理的 UDP 关联。

本次必须通过的命令/真实场景：
- `cargo test -p application --lib --locked`（`speedtest::re_prof_08_udp_tests::udp_ping_via_socks_routes_through_the_session_port`、`udp_via_socks_datagram_encodes_domain_targets`）
- `cargo test -p bridge_api --lib --locked`（`api::speedtest::tests::udp_ping_routes_through_the_session_port`、`api::speedtest::tests::udp_support_is_gated_by_the_socks_associate_path`）
- `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`、`cargo fmt -p application -p bridge_api -- --check`

证据文件位置：`docs/evidence/recheck-fixes/R3-06-PROF-05-07/`。

完成条件：`NetHostTestSession::udp_ping` 使用 `session.port` 走 SOCKS5 UDP ASSOCIATE；宿主直连 `udp_ping` 不再作为节点结果；支持位 `udp` 由 `udp_via_socks_supported()` 门控；受控测试证明目标经 relay；门禁通过。未运行真实内核，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`speedtest_supported()` 为同步、无 session，故支持位只能按“本构建具备 SOCKS5 UDP associate 路径”门控，不能按当前节点内核逐一判定；如需按节点能力精确置位，需要会话能力查询接口。
- 接口缺口（登记）：受控 server 未校验响应内容（上游 `VerifyAndExtractUdpResponse`）；本卡只保证“经节点 relay 的往返”，NTP/DNS/STUN 响应验证未迁移。

本轮实际结果：`application::speedtest::udp_ping_via_socks` + `socks5_udp_associate` + `socks5_udp_datagram` + `udp_via_socks_supported` 新增；`NetHostTestSession::udp_ping` 改经 `session.port`；`speedtest_supported().udp` 改为能力门控；`bridge_api` 新增受控 SOCKS5 UDP server 测试断言 `session.port` 路径。`application` 224 个 lib 测试、`bridge_api` 59 个通过；clippy/fmt 干净。
