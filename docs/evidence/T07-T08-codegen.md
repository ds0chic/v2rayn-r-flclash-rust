# T07/T08 — Xray 与 sing-box 配置生成实现证据

- 任务：T07 Xray 生成链 / T08 sing-box 生成链（实现部分）
- 契约事实源：`docs/decisions/T07-xray-codegen-contract.md`、`docs/decisions/T08-singbox-codegen-contract.md`
- 映射大表：`compat/codegen-map.xray.yaml`、`compat/codegen-map.singbox.yaml`
- crate：`crates/config_codegen`
- 约定：本回合不运行上游内核、不下载内核、不触碰 10808；测试数据全部为合成数据（RFC5737 / example.com / 合成密钥）。

## 1. 交付物与 crate 结构

```
crates/config_codegen/
  Cargo.toml
  src/lib.rs                     公共入口、CodegenError/Diagnostic/GeneratedConfigs/GeneratedFile
  src/input.rs                   CodegenInput/CodegenProfile/CodegenSettings/CodegenRouting/
                                 CodegenDns/SimpleDns/CodegenTemplate 等纯输入类型（serde，无 IO）
  src/util.rs                    常量表、JSON 构造辅助、PEM 解析、CIDR 加减/聚合、域名/UA 映射
  src/diff.rs                    语义差分辅助（对象无序、数组保序、归一化随机端口/临时路径）
  src/xray/{mod,log,inbound,outbound,routing,dns,stat,config}.rs
  src/singbox/{mod,log,inbound,outbound,routing,dns,ruleset,stat,config}.rs
  tests/common/mod.rs            测试辅助（合成 profile/settings、fixtures 读取）
  tests/*.rs                     14 个集成测试文件
```

公共入口：

- `pub fn generate_xray(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError>`
- `pub fn generate_singbox(input: &CodegenInput) -> Result<GeneratedConfigs, CodegenError>`
- `GeneratedConfigs { main: serde_json::Value, files: Vec<GeneratedFile>, diagnostics: Vec<Diagnostic> }`

生成过程为纯函数：不启动进程、不访问文件系统、不修改系统状态；调用方把模板/样例内容与
`custom_outbound_content` 作为数据传入。`Custom` 配置类型原样透传（返回解析后的 JSON/字符串 +
`custom_passthrough` 诊断）。不允许的组合返回结构化错误码并携带字段路径，不静默丢弃。

## 2. 覆盖范围

### 2.1 Xray（T07）

- 协议：VMess / VLESS / Shadowsocks / Trojan / Hysteria2 / WireGuard / SOCKS / HTTP + Outbound 自定义 + PolicyGroup / ProxyChain；Custom 透传。
- 传输：raw（含 http header 请求体替换）、ws、httpupgrade、xhttp、kcp（`mkcp-legacy` mask 反序）、grpc、hysteria。
- 安全：tls（alpn/fingerprint/ech/pinnedPeerCertSha256/certificates/disableSystemRoot）、reality（publicKey/shortId/serverName/spiderX/mldsa65Verify/show）；`allowInsecure` 永不写出。
- 全局：Inbound（`LocalPort + EInboundProtocol`，socks/socks2/socks3、AllowLANConn 认证）、log/loglevel、DNS（SimpleDNS 普通 + FakeIP + RawDNS 双路）、routing（RuleType DNS/Routing 区分、Remarks→tag 解析与回退、balancer 改写、final rule）、统计/API（StatePort）、TUN、mux（Mux4Ray）、Kcp/Grpc 全局参数、Hysteria 回退、Fragment4Ray、HappyEyeballs4Ray、BindInterface/SendThrough。
- 模板：FullConfigTemplate 的 Config/TunConfig 注入、AddProxyOnly、ProxyDetour、balancer/observatory/burstObservatory 合并、自定义出站 `{{tag}}/{{detour}}/{{interface}}`。

### 2.2 sing-box（T08）

- 协议：VMess / VLESS / Shadowsocks / Trojan / Hysteria2 / TUIC / Anytls / Naive / WireGuard（endpoint）/ SOCKS / HTTP + Outbound（`IsSingboxEndpoint` 决定 outbounds/endpoints）+ PolicyGroup / ProxyChain；Custom 透传。
- 传输：raw（http）、ws（`?ed=`/`?eh=` 解析）、httpupgrade、grpc；kcp/xhttp 结构化拒绝。
- 安全：tls（enabled/server_name/insecure/alpn/utls/certificate/reality/ech/fragment）、Shadowsocks 的 `plugin/plugin_opts`。
- 全局：log 等级映射（warning→warn、none→disabled）、Inbound（mixed、socks/socks2/socks3、users）、TUN inbound 与 route 规则（auto_detect_interface、tun 规则、自身地址 reject/drop、ICMP、sniff/hijack-dns、final fragment）、DNS（servers/rules/final、hosts、fakeip filter、evaluate/respond）、experimental（clash_api 无条件、cache_file）、Geo→rule_set 转换（本地 srs/远程 srs + http_clients）、组 selector/urltest、BindInterface/SendThrough。

### 2.3 结构化错误

`unsupported_combination`、`missing_required_field`、`dangling_reference`、`custom_outbound_missing`、`custom_outbound_invalid`、`invalid_template`，均带 `field_path`。
引用悬空（PolicyGroup/ProxyChain 子项缺失、自定义出站内容缺失）明确报错而非静默丢弃。

## 3. 测试与结果

命令与结果（本机 Windows，端口全部 ≥ 11808）：

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过（无输出） |
| `cargo clippy -p config_codegen --all-targets --locked -- -D warnings` | 通过 |
| `cargo test -p config_codegen --locked --no-fail-fast` | 通过：14 个测试二进制，71 个测试全部 ok |
| `cargo test --workspace --locked --no-fail-fast` | 通过（所有 crate 测试 ok） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | **阻塞（非本 crate）**：`crates/domain` 有 14 个 lib + 17 个 test clippy 错误（见 §6） |

测试文件与用例：

| 文件 | 用例数 | 覆盖点 |
|---|---|---|
| `xray_protocols.rs` | 5 | VMess+ws+tls、VLESS+reality+raw、SS/SOCKS/HTTP、Trojan/Hysteria2（含 finalmask/brutal/salamander）、WireGuard |
| `xray_transport_security.rs` | 5 | raw http 请求体替换与 UA 映射、ws/httpupgrade/xhttp（mux 复位）、kcp mask 反序、grpc+reality、TLS 证书/ECH |
| `xray_global.rs` | 4 | inbound 端口偏移/socks3 认证、log、stat API、TUN（gateway/MTU/route exclude/direct-exe/rules 顺序）、fragment 两段、BindInterface/SendThrough |
| `xray_routing_dns.rs` | 4 | SimpleDNS+FakeIP+RuleType 区分、Remarks→tag 解析与回退、PolicyGroup balancer、ProxyChain + HappyEyeballs |
| `xray_template_custom.rs` | 5 | fixtures 模板注入、balancer 改写、自定义出站占位符、Custom 透传、缺失自定义内容错误 |
| `xray_errors.rs` | 7 | 不支持类型、quic、缺失必填、reality publicKey、悬空引用、空组、确定性 |
| `singbox_protocols.rs` | 5 | VMess/VLESS/SS、SOCKS/HTTP/Trojan、TUIC/Anytls/Naive、Hysteria2 端口/obfs、WireGuard endpoint |
| `singbox_transport_security.rs` | 4 | ws `?ed=`、raw http/httpupgrade/grpc、Shadowsocks plugin、tls/reality/ech/fragment |
| `singbox_global.rs` | 6 | log/inbounds、TUN+rules、DNS+experimental、rule_set 转换（本地/远程）、rule type 区分、BindInterface/final fragment |
| `singbox_groups.rs` | 3 | selector/urltest、ProxyChain detour、组标签解析 |
| `singbox_template_custom.rs` | 4 | 模板注入、自定义 endpoint 标记、自定义出站替换、Custom 透传 |
| `singbox_errors.rs` | 6 | kcp/xhttp 拒绝、WireGuard 地址、必填、悬空、Custom 内容缺失、确定性 |
| `diff_semantics.rs` | 8 | 同输入一致、对象无序、数组保序、缺失/多余/长度、归一化路径与端口、值替换 |
| `fixtures_samples.rs` | 5 | 读取 `dns_v2ray_normal`/`dns_singbox_normal`/`singbox_fakeip_filter`/`SampleTunRules`/`tun_singbox_inbound` 并注入 |

组合抽样：ws+tls、grpc+reality、xhttp、raw http header、kcp、hysteria、httpupgrade；sing-box
端 ws（含 early data）、raw http、httpupgrade、grpc、tls/reality、plugin、endpoint。

## 4. 与契约的偏差 / UNC 处理

实现遵循两份契约；以下为需要记录的差异（多为契约文字与上游源码不一致，或纯函数化取舍）：

1. **T08 模板 outbounds 顺序**：契约 §5 写“生成的在先、模板追加”；上游 `SingboxConfigTemplateService.cs:106-125`
   实际把生成出站追加到模板出站之后。本实现按契约（生成在前）执行；需在 T06b 真实内核/上游差分时复核。
2. **quic**：T07 契约声明入口拒绝 quic；上游 `ProfileItem.GetNetwork()` 会把 quic 归一为 raw，
   拒绝分支实际不可达（UNC-X-004）。本实现对显式 `network=="quic"` 返回 `unsupported_combination`
   以符合 T07；T08 按契约归一为 raw。
3. **Xray `#` 域名去重**：上游 `V2rayRoutingService.GenRoutingUserRule` 存在 `RemoveAt` 后越界索引的
   缺陷；本实现先过滤 `#` 再替换 `<COMMA>`，不复制崩溃。
4. **TUN direct-exe**：上游从 CoreInfoManager 推导可执行文件；本实现接受已解析的
   `settings.protect_core_executables`（纯函数、不读进程/路径）。
5. **sing-box 自定义 rule_set**：上游读取 `CustomRulesetPath4Singbox` 文件；本实现接受解析后的
   `routing.custom_ruleset`，本地 srs 是否存在以 `settings.local_srs_files` 表示。
6. **DNS evaluate 规则 id**：上游用 `Utils.GetGuid(false)` 生成随机 id；本实现用输入 `CodegenRule.id`
   （默认 `final`）保持确定性，语义差分时可按需归一化。
7. **Hysteria2 realm**：`HyRealm` 的 `ToServerUrl/ToUriForFinalmask` 细节属 UNC-X/UNC-S；本实现做
   最小解析（server_url/token/realm_id/stun_servers）并标注待真实内核校验。
8. **macOS TUN 名称**：上游随机 `utunN`；本实现用 `settings.tun.name` 或确定性默认（`utun0`）。
9. **system hosts**：不读宿主机，改为输入 `CodegenDns.system_hosts`。
10. **sing-box utls 回退**：上游仅在节点 `Fingerprint` 非空时写 utls（`DefFingerprint` 回退为死代码）；
    本实现按 T08 契约在节点指纹为空时回退 `DefFingerprint`。
11. **WgReserved 非法项**：上游 `int.Parse` 抛异常被吞；本实现跳过非法项。
12. **UNC-X-003**：`fakedns` 为顶层字段已按源码实现（非 `inbounds[].settings.fakedns`）。
13. **UNC-S-001/002/003**：sing-box HTTP 不写 `HttpHeaders`、不写 `WgDns`、不写
    `SpiderX/VerifyPeerCertByName/CertSha/Mldsa65Verify` —— 与契约一致，均已按“不写”实现并在测试覆盖。
14. **not_applicable**：Xray 的 HappyEyeballs 仅 Xray 生效；sing-box 的 Kcp/Mux4Ray/HappyEyeballs 不适用。

## 5. 后续需要真实内核校验的点（T06b/T07/T08 后续）

- 用冻结版 Xray / sing-box 二进制对生成结果跑官方配置检查（`xray run -test` / `sing-box check`），
  本回合未运行任何内核。
- 与上游 `CoreConfigV2rayService` / `CoreConfigSingboxService` 的实测 JSON 做语义差分（`diff.rs` 已就绪），
  重点：T08 模板出站顺序、raw ws/httpupgrade 头部、DNS evaluate/respond 结构、rule_set 远程 URL。
- 端到端路由/DNS/TCP/UDP 行为验证（配置可解析 ≠ 路由正确）。
- `Custom` 透传在真实内核下的启动方式、以及 Custom 的 pre-socks（UNC-X-002）。

## 6. workspace clippy 阻塞记录

`cargo clippy --workspace --all-targets --locked -- -D warnings` 当前失败，失败点全部位于
`crates/domain`（本回合边界外，另一路线在改），错误类别：

- `variable does not need to be mutable`（`runtime_plan.rs:551`）
- `this impl can be derived`（`enums.rs:540` `InboundProtocol`）
- `result_large_err`（`profile.rs:273`、`revision.rs:113`、`runtime_plan.rs:172/225/249/308/384/458` 等）
- `collapsible_if`（`runtime_plan.rs:392/465`）
- `field_reassign_with_default`（`profile.rs:351`，测试）

本 crate 自身的 `cargo clippy -p config_codegen --all-targets --locked -- -D warnings` 通过。
`cargo test --workspace --locked` 与 `cargo fmt --all -- --check` 均通过。待 domain 修复后重跑
workspace clippy 门禁即可全绿；本文件末尾会在重试后更新结果。

重试记录（本回合内两次，间隔约 25s）：结果不变，`crates/domain` 仍报 14 lib + 17 test clippy 错误，
因此 workspace clippy 门禁在本次会话无法全绿，阻塞点位于本子代理写入边界之外（`crates/config_codegen/**`
之外）。其余门禁全部通过：

```
cargo fmt --all -- --check                                    -> PASS
cargo clippy -p config_codegen --all-targets --locked -- -D warnings -> PASS
cargo test -p config_codegen --locked                          -> PASS (71 tests)
cargo test --workspace --locked                                -> PASS (all crates)
cargo clippy --workspace --all-targets --locked -- -D warnings -> FAIL: crates/domain only
```

接口契约（供 domain 修复者参考）：本 crate 不依赖 domain；两者可独立验证。`crates/config_codegen`
不含任何 `#[allow(clippy::result_large_err)]`，其 `CodegenError`（约 72 字节）不触发该 lint。

