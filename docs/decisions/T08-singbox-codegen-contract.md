# T08 — sing-box 生成器输入→输出契约

- 任务：T08 sing-box 生成链
- 内核：sing-box（CoreConfigSingboxService）
- 上游：v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`
- 定位：冻结源码 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/`（只读）
- 附件（唯一大表事实源）：[`compat/codegen-map.singbox.yaml`](../../compat/codegen-map.singbox.yaml)
- 关联：`compat/features.yaml`（config_types / core_capability_matrix）、`compat/fields.entities.yaml`、`compat/fields.yaml`、`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §12

> 约定：所有 `path:line` 为冻结源码相对路径。生成 JSON 成员名以 `v2rayN/ServiceLib/Models/CoreConfigs/SingboxConfig.cs` 为准（snake_case）。`kcp`/`xhttp` 在入口即被拒绝。本文不写代码、不改台账。

---

## 1. 入口、模板与生成顺序

调度：`CoreConfigHandler.GenerateClientConfig` 在 `RunCoreType==sing_box` 时走 `CoreConfigSingboxService`（`v2rayN/ServiceLib/Handler/CoreConfigHandler.cs:24-27`）。

入口：`CoreConfigSingboxService.GenerateClientConfigContent()`（`v2rayN/ServiceLib/Services/CoreConfig/Singbox/CoreConfigSingboxService.cs:13-72`）。

| 序 | 步骤 | 证据 |
|---|---|---|
| 1 | 反序列化 `Global.SingboxSampleClient` → `SingboxConfig` | `CoreConfigSingboxService.cs:32-44`；`Global.cs:20` |
| 2 | `GenLog()` | `:46` |
| 3 | `GenInbounds()` | `:48` |
| 4 | `GenOutbounds()` | `:50` |
| 5 | `GenRouting()` | `:52` |
| 6 | `GenDns()` | `:54` |
| 7 | `GenExperimental()`（clash_api / cache_file） | `:56` |
| 8 | `ConvertGeo2Ruleset()` | `:58` |
| 9 | `ApplyFinalConfigModifiers()` | `:63` |

入口拒绝：`kcp`、`xhttp`（`CoreConfigSingboxService.cs:24-28`）；`quic` 经 `GetNetwork()` 归一到 `raw`（`Global.cs:330-338`）。

基础模板（`v2rayN/ServiceLib/Sample/SingboxSampleClientConfig:1-16`）：`log.level=debug`、`log.timestamp=true`；`inbounds=[]`；`outbounds=[{type:direct,tag:direct}]`；`route.rules=[]`。出站模板 `SingboxSampleOutbound`（`{type:vless,tag:proxy,server:"",server_port:443}`，`Global.cs:27`）。

---

## 2. 输入字段清单（按 EConfigType）

真值表见 `compat/codegen-map.singbox.yaml#protocol_output_map`。sing-box 支持 11 类 + 2 类复合 + Outbound（`Global.cs:378-391`）。

| EConfigType | 值 | 形态 | 关键输入（→ 顶层输出路径） |
|---|---|---|---|
| VMess | 1 | outbound | Address/Port→server/server_port；Password→uuid；ProtoExtra.AlterId→alter_id；ProtoExtra.VmessSecurity→security |
| Shadowsocks | 3 | outbound | ProtoExtra.SsMethod→method；Password→password；ProtoExtra.Uot→udp_over_tcp；运输经 `plugin/plugin_opts` |
| SOCKS | 4 | outbound | version="5"；Username+Password→username/password |
| VLESS | 5 | outbound | Password→uuid；packet_encoding="xudp"；ProtoExtra.Flow→flow（vision 归一） |
| Trojan | 6 | outbound | Password→password |
| Hysteria2 | 7 | outbound | Password；ProtoExtra.{SalamanderPass,GeckoMin/MaxPacketSize,UpMbps,DownMbps,Ports,HopInterval,Hy2RealmUrl}→obfs/up_mbps/down_mbps/server_ports/hop_interval/realm |
| HTTP | 10 | outbound | Username+Password；**HttpHeaders 未读取**（见未确证） |
| TUIC | 8 | outbound | **仅 sing-box**；Username→uuid；Password；ProtoExtra.CongestionControl→congestion_control |
| Anytls | 11 | outbound | **仅 sing-box**；Password→password |
| Naive | 12 | outbound | **仅 sing-box**；Username/Password；ProtoExtra.{NaiveQuic,CongestionControl,InsecureConcurrency,Uot} |
| WireGuard | 9 | **endpoint** | ProtoExtra.{WgInterfaceAddress,WgPublicKey,WgPresharedKey,WgReserved,WgMtu}；Password→private_key；Address/Port→peers[0].address/port |
| Outbound | 13 | outbound 或 endpoint | ProtoExtra.IsSingboxEndpoint 决定反序列化为 `Endpoints4Sbox` 还是 `Outbound4Sbox` |
| PolicyGroup | 101 | 组 | ChildItems→多个出站 + urltest/selector |
| ProxyChain | 102 | 组 | ChildItems→`detour` 链 |
| Custom | 2 | 复制文件 | `CoreConfigHandler.cs:16-22` |

WireGuard 走 `FillEndpoint` 返回 `Endpoints4Sbox`（`SingboxOutboundService.cs:80-85,347-380`），并跳过 `FillOutboundTls`。

---

## 3. 输出路径映射

### 3.1 按 network（`ETransport`）

生产者 `FillOutboundTransport`（`SingboxOutboundService.cs:487-591`）。sing-box 仅支持 `raw/ws/httpupgrade/grpc`，`kcp/xhttp` 被拒（`CoreConfigSingboxService.cs:24-28`），`quic` 归一为 raw。Shadowsocks 不使用 transport，走 `plugin/plugin_opts`。

| network | 输出路径 | 关键输入 | 证据 |
|---|---|---|---|
| raw（http） | `outbounds[].transport.{type=http,host[],path,headers.User-Agent}` | TransportExtra.{RawHeaderType==http,Host,Path}；DefUserAgent | `:498-512` |
| ws | `outbounds[].transport.{type=ws,path,headers.Host,headers.User-Agent,max_early_data,early_data_header_name}` | TransportExtra.{Path,Host}；DefUserAgent；path 中 `?ed=N`/`?eh=` 解析 | `:514-557` |
| httpupgrade | `outbounds[].transport.{type=httpupgrade,path,host,headers.User-Agent}` | TransportExtra.{Path,Host}；DefUserAgent | `:559-569` |
| grpc | `outbounds[].transport.{type=grpc,service_name,idle_timeout,ping_timeout,permit_without_stream}` | TransportExtra.GrpcServiceName；GrpcItem.{IdleTimeout,HealthCheckTimeout,PermitWithoutStream} | `:571-577` |

`transport.type` 为空时不写 `transport`（`:582-585`）。`GrpcAuthority` 仅作 `tls.server_name` 回退，`GrpcMode` 未使用。

Shadowsocks plugin（`:137-178`）：`raw`+`header=http` → `plugin="obfs-local"`,`plugin_opts="obfs=http;obfs-host={Host};"`；`ws`/`tls` → `plugin="v2ray-plugin"`，opts 含 `mode=websocket;host;path;tls;certRaw;mux=0`。

### 3.2 按 security（`ProfileItem.StreamSecurity`）

生产者 `FillOutboundTls`（`SingboxOutboundService.cs:405-485`）。提前返回：非 `tls/reality`，或 `Shadowsocks/SOCKS/WireGuard`（`:409-416`）。

| 输出路径 | 输入 | 证据 |
|---|---|---|
| `outbounds[].tls.enabled=true` | 常量 | `:437` |
| `outbounds[].tls.server_name` | Sni 或按 network 取 Host 首项（grpc 用 GrpcAuthority） | `:417-434` |
| `outbounds[].tls.insecure` | `GetAllowInsecure()`；有证书/reality 强制 false | `:439,461,472` |
| `outbounds[].tls.alpn` | Alpn | `:440` |
| `outbounds[].tls.{fragment,record_fragment}=true` | CoreBasicItem.EnableFragment | `:442-446` |
| `outbounds[].tls.utls.{enabled,fingerprint}` | Fingerprint 或 DefFingerprint | `:447-454` |
| `outbounds[].tls.certificate` + `insecure=false` | Cert（tls） | `:455-463` |
| `outbounds[].tls.reality.{enabled,public_key,short_id}` + `insecure=false` | PublicKey/ShortId（reality） | `:464-473` |
| `outbounds[].tls.ech` | EchConfigList → `ParseEchParam`（config 列表 或 query_server_name） | `:474-478,802-826` |

sing-box 侧差异：`SpiderX`、`VerifyPeerCertByName`、`CertSha`、`Mldsa65Verify` **未写出**（与 Xray 不同）。

### 3.3 Mux 与 endpoint

- Mux：`MuxEnabled && Mux4SboxItem.Protocol` 非空 → `outbounds[].multiplex.{enabled,protocol,max_connections,padding}`（`:382-403`），被 VMess/Shadowsocks/VLESS/Trojan 调用。
- WireGuard endpoint：`endpoints[].{address[],private_key,mtu,peers[]}`；peer `{public_key,pre_shared_key,reserved,address,port,allowed_ips=["0.0.0.0/0","::/0"]}`（`:347-380`）。
- `FillRangeProxy` 把 `Outbound4Sbox` 写 `outbounds`、`Endpoints4Sbox` 写 `endpoints`，普通节点 prepend 到头部（`:774-800`）。
- 组：多节点生成 `selector(proxy)` + `urltest(proxy-auto)`，`Fallback` 时 `urltest.tolerance=5000`（`:593-620`）；ProxyChain 用 `detour`（`:656-740`）。

---

## 4. 全局设置注入

完整表见 `codegen-map.singbox.yaml#global_injection`。摘要：

| 来源设置 | 注入路径 | 证据 |
|---|---|---|
| CoreBasicItem.Loglevel | `log.level`（`warning→warn`；`none→disabled=true`） | `SingboxLogService.cs:9-27` |
| CoreBasicItem.LogEnabled | `log.output=bin/sbox_<date>.txt` | `SingboxLogService.cs:28-32` |
| CoreBasicItem.DefFingerprint | `outbounds[].tls.utls.fingerprint` | `SingboxOutboundService.cs:452` |
| CoreBasicItem.DefUserAgent | `outbounds[].transport.headers.User-Agent` | `:493-510,552-556,563-567` |
| CoreBasicItem.SendThrough | `outbounds[].inet4_bind_address` | `SingboxConfigTemplateService.cs:159-171` |
| CoreBasicItem.BindInterface | `outbounds[].bind_interface` | `SingboxConfigTemplateService.cs:146-157` |
| CoreBasicItem.EnableFragment | `outbounds[].tls.{fragment,record_fragment}` | `SingboxOutboundService.cs:442-446` |
| CoreBasicItem.EnableFinalFragment | `route.rules[] {protocol:[tls],action:route-options,tls_record_fragment:true}` | `SingboxRoutingService.cs:133-157` |
| CoreBasicItem.EnableCacheFile4Sbox | `experimental.cache_file.{enabled,path,store_fakeip}` | `SingboxStatisticService.cs:16-25` |
| Mux4SboxItem.* | `outbounds[].multiplex.*` | `SingboxOutboundService.cs:387-396` |
| Mux4RayItem.* | 不适用（仅 Xray） | — |
| KcpItem.* | 不适用（sing-box 拒绝 kcp） | `CoreConfigSingboxService.cs:24-28` |
| GrpcItem.* | `outbounds[].transport.{idle_timeout,ping_timeout,permit_without_stream}` | `SingboxOutboundService.cs:574-576` |
| HysteriaItem.* | `outbounds[].{up_mbps,down_mbps,hop_interval}` | `:250-288` |
| Fragment4RayItem.* | 不适用（sing-box 用 `tls.fragment` 布尔） | `:442-446` |
| HappyEyeballs4RayItem.* | 不适用 | — |
| SimpleDNSItem.* | `dns.servers/rules/final`；`route.default_domain_resolver` | `SingboxDnsService.cs:5-30,32-136,148-521`；`SingboxRoutingService.cs:9-34` |
| SimpleDNSItem.FakeIP | `dns.servers[] {type:fakeip,inet4_range}` + fakeip 规则 + `cache_file.store_fakeip` | `SingboxDnsService.cs:123-135,256-278` |
| RoutingBasicItem.DomainStrategy4Singbox | `dns.rules[].strategy` / resolve 规则 strategy | `SingboxRoutingService.cs:238-252` |
| RoutingBasicItem.DomainStrategy | `route` resolve 触发（IPOnDemand/IPIfNonMatch） | `SingboxRoutingService.cs:249-285` |
| GuiItem.EnableStatistics/DisplayRealTimeSpeed | `experimental.clash_api.external_controller=127.0.0.1:{StatePort2}`（源码无条件写入） | `SingboxStatisticService.cs:7-14`；`AppManager.cs:24-31` |
| TunModeItem.* | `inbounds[tun].*` | `SingboxInboundService.cs:54-82` |
| Inbound[0].* | `inbounds[].{listen,listen_port,type}` | `SingboxInboundService.cs:5-97` |
| Inbound[0].SniffingEnabled | `route.rules[] {action:sniff}` | `SingboxRoutingService.cs:116-158` |
| RoutingItem.RuleSet | `route.rules[]`（跳过 `RuleType==DNS`） | `SingboxRoutingService.cs:254-277` |
| RoutingItem.CustomRulesetPath4Singbox | `route.rule_set[]` 追加自定义 srs | `SingboxRulesetService.cs:60-75` |

**域名策略映射**：`Utils.DomainStrategy4Sbox` 把 Xray 风格 `AsIs/IPIfNonMatch/...` 映射为 sing-box 的 `prefer_ipv4/prefer_ipv6/ipv4_only/ipv6_only`（`Global.cs:402-409`；`SingboxRoutingService.cs:12-13`）。

**DNS 服务器与前缀**：`direct-dns-N`/`remote-dns-N`/`local-local`/`hosts-dns`/`fake-dns`（`Global.cs:106-115`）；remote 服务器 `detour=proxy`（`SingboxDnsService.cs:46-54`）。

**RuleType 区分**：路由生成跳过 `RuleType==DNS`（`SingboxRoutingService.cs:265-268`）；DNS 生成跳过 `RuleType==Routing`（`SingboxDnsService.cs:340-343`）。这与 Xray 侧对称（见 T07 §4）。

**统计/API 端口**：sing-box 写 `StatePort2`（`GetLocalPort(api2) + (EnableTun?1:0)`；`AppManager.cs:24-31`），供 `StatisticsSingboxService` 的 `ws://127.0.0.1:{StatePort2}/traffic` 与 `ClashApiManager`（`StatePort2`）使用。

**TUN（sing-box 形态）**：模板 `tun_singbox_inbound`（`Global.cs:30`）。`interface_name=singbox_tun`（mac `utunN`）；`mtu`；`auto_route`/`strict_route`/`stack`（默认 `gvisor`）；`address=[IPv4Address or TunIPv4Address.First()]`(+IPv6)；`route_exclude_address`（`SingboxInboundService.cs:54-82`）。路由侧 `auto_detect_interface=true`、`tun_singbox_rules`、TUN 地址自身 `reject/drop` 防环、ICMP 策略（`SingboxRoutingService.cs:36-113`）。sing-box 的 TUN inbound **不写 sniffing**，改用 `route.rules[].action="sniff"`。

---

## 5. 配置模板与插入点

`ApplyFullConfigTemplate`（`SingboxConfigTemplateService.cs:86-144`）：

- 仅当 `FullConfigTemplate.Enabled==true` 且 `Config`/`TunConfig` 非空；`IsTunEnabled` 选 `TunConfig`（`:86-104`）。
- outbounds：生成的在先（`AddProxyOnly==true` 时跳过 `direct/block`），模板 outbounds 追加（`:106-125`）。
- **endpoints 特殊处理**：生成 endpoints 追加到模板 `endpoints`；为空则移除该键（`:127-141`；`ApplyCustomOutboundReplace` `:26-35`）。
- `ProxyDetour`：无 `detour` 且非私网 → `outbounds[].detour` / `endpoints[].detour`（`:119-122,134-137`）。
- `ApplyCustomOutboundReplace`（`:15-84`）：`{{tag}}/{{detour}}/{{interface}}`；自定义 endpoint 写入 `endpoints` 数组。

`ConvertGeo2Ruleset`（`SingboxRulesetService.cs:5-131`）：把 route/dns 规则的 `geosite/geoip` 转为 `rule_set`，离线优先本地 `bin/srss/*.srs`，否则远程 `Global.SingboxRulesetUrl` 并加 `http_clients[srs-download-http-client].detour=proxy`。

---

## 6. 生成顺序与覆盖规则

完整表见 `codegen-map.singbox.yaml#precedence_rules`。要点：

1. 模板提供骨架，`Gen*` 逐步覆盖（`CoreConfigSingboxService.cs:32-58`）。
2. `GenInbounds` 清空重建 `inbounds`（`SingboxInboundService.cs:12`）。
3. `GenOutbounds` 通过 `FillRangeProxy(prepend=true)` 把代理出站插入 `outbounds` 头部（`SingboxOutboundService.cs:5-9,774-800`）。
4. 可选字段按需写入；空值不写（`tls`/`transport`/`multiplex`/`obfs` 等均在条件内）。
5. `ConvertGeo2Ruleset` 在 `GenDns`/`GenRouting` 之后统一规则集替换（`CoreConfigSingboxService.cs:58`）。
6. `ApplyCustomOutboundReplace` → `ApplyFullConfigTemplate` 最后合并。
7. 默认回退：`DefFingerprint`/`DefUserAgent`；`WgInterfaceAddress→172.16.0.2/32`；`WgMtu→1280`；`TunStacks.First()=gvisor`；`Hysteria2DefaultHopInt=30`；FakeIPRange=`Global.FakeIPRanges.First()=198.18.0.0/15`（`ConfigHandler.cs:90-96,158-188`；`Global.cs:575-590,752-756`）。
8. **禁止的旧字段**：sing-box 允许 `tls.insecure`（来自 `AllowInsecure`），但不写 Xray 专用 `allowInsecure`；`SpiderX/VerifyPeerCertByName/CertSha/Mldsa65Verify` 无对应输出。

---

## 7. 差分测试建议（最小合成输入）

测试端口一律 ≥ 11808；合成数据不得含真实凭据。

| # | 输入（最小合成） | 关键断言路径 |
|---|---|---|
| S1 | VLESS + tls；`Address=192.0.2.20,Port=443,Password=<uuid>,StreamSecurity="tls",Sni="a.test",Fp="chrome"` | `outbounds[0].type=="vless"`；`outbounds[0].tls.enabled==true`；`tls.server_name=="a.test"`；`tls.utls.fingerprint=="chrome"`；`uuid` 存在；无 `transport` |
| S2 | VMess + ws（带 `?ed=2048`）；`Network="ws",TransportExtra.Host="ws.test",Path="/p?ed=2048"` | `transport.type=="ws"`；`transport.path=="/p"`（ed 被剥离）；`transport.max_early_data==2048`；`early_data_header_name=="Sec-WebSocket-Protocol"`；`transport.headers.Host=="ws.test"` |
| S3 | TUIC（仅 sing-box）；`Address=192.0.2.30,Port=443,Username=<uuid>,Password="pw",ProtoExtra.CongestionControl="bbr"` | `outbounds[0].type=="tuic"`；`uuid==<uuid>`；`password=="pw"`；`congestion_control=="bbr"` |
| S4 | Hysteria2 + `Ports="20000-30000,40000"`；`SalamanderPass="pw",UpMbps=50` | `outbounds[0].type=="hysteria2"`；`server_port` 为 null；`server_ports==["20000:30000","40000:40000"]`；`hop_interval` 形如 `"30s"`；`obfs.type=="salamander"`；`up_mbps==50` |
| S5 | WireGuard；`Address=192.0.2.40,Port=51820,Password=<priv>,ProtoExtra.WgPublicKey=<pub>,WgInterfaceAddress="10.0.0.2/32",WgMtu=1420` | `endpoints[0].type=="wireguard"`；`endpoints[0].private_key==<priv>`；`endpoints[0].mtu==1420`；`peers[0].address=="192.0.2.40"`、`port==51820`、`allowed_ips==["0.0.0.0/0","::/0"]`；`outbounds` 中无该节点 |

补充差分点：`route.final=="proxy"`、`route.default_domain_resolver.server`、`dns.final`、`experimental.clash_api.external_controller`、`rule_set`/`http_clients`、TUN inbound 的 `address/stack/auto_route`。

---

## 8. 能力矩阵复核

| 集合 | 精确值 | 代码行 |
|---|---|---|
| `Global.SingboxSupportConfigType` | {VMess, VLESS, Shadowsocks, Trojan, Hysteria2, TUIC, Anytls, Naive, WireGuard, SOCKS, HTTP} | `v2rayN/ServiceLib/Global.cs:378-391` |
| `Global.SingboxOnlyConfigType` | {TUIC, Anytls, Naive}（`Singbox.Except(Xray)`） | `Global.cs:393` |
| `Global.XraySupportConfigType` | {VMess, VLESS, Shadowsocks, Trojan, Hysteria2, WireGuard, SOCKS, HTTP} | `Global.cs:366-376` |
| 传输约束 | 不支持 kcp/xhttp；非 VMess/VLESS/Trojan/SS 只允许 raw；SS 只允许 raw/ws | `NodeValidator.cs:16-23,173-193` |
| endpoint 类型 | WireGuard；Outbound 且 `IsSingboxEndpoint==true` | `SingboxOutboundService.cs:66-85` |

---

## 9. 未确证清单（sing-box）

| ID | 项 | 说明 | 证据 |
|---|---|---|---|
| UNC-S-001 | HTTP `HttpHeaders` 映射 | `FillOutbound` HTTP 分支只写 username/password，未见 headers | `SingboxOutboundService.cs:195-204` |
| UNC-S-002 | `WgDns` 映射 | `FillEndpoint` 未读取 WgDns | `SingboxOutboundService.cs:347-380` |
| UNC-S-003 | `SpiderX/VerifyPeerCertByName/CertSha/Mldsa65Verify` 替代路径 | sing-box 生成器均未写出 | `SingboxOutboundService.cs:405-485` |
| UNC-S-004 | 内核版本（`LockedMaxVersion=1.14`）对生成结构的影响 | 生成器不按版本分支，限制在 `CoreInfoManager` | `features.yaml` CRM-004 |
| UNC-S-005 | `GrpcMode`/`GrpcAuthority` 完整语义 | 仅 GrpcAuthority 作 tls.serverName 回退，GrpcMode 未使用 | `SingboxOutboundService.cs:424-434,571-577` |
| UNC-S-006 | `experimental.clash_api` 是否受 GuiItem 开关控制 | 源码条件被注释为无条件写入 | `SingboxStatisticService.cs:7-14` |

---

## 10. 源码索引

- 入口/编排：`CoreConfigSingboxService.cs`
- 出站/传输/TLS/endpoint：`SingboxOutboundService.cs`
- 入站：`SingboxInboundService.cs`
- 路由：`SingboxRoutingService.cs`
- DNS：`SingboxDnsService.cs`
- 规则集：`SingboxRulesetService.cs`
- 日志：`SingboxLogService.cs`
- 统计/实验：`SingboxStatisticService.cs`
- 模板合并：`SingboxConfigTemplateService.cs`
- 模型：`Models/CoreConfigs/SingboxConfig.cs`
- 常量/能力集：`Global.cs`
- 校验：`Handler/Builder/NodeValidator.cs`
- 采样模板：`Sample/SingboxSampleClientConfig`、`Sample/SingboxSampleOutbound`、`Sample/tun_singbox_inbound`、`Sample/tun_singbox_rules`、`Sample/dns_singbox_normal`、`Sample/tun_singbox_dns`、`Sample/singbox_fakeip_filter`
