# T07 — Xray 生成器输入→输出契约

- 任务：T07 Xray 生成链
- 内核：Xray（CoreConfigV2rayService）
- 上游：v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`
- 定位：冻结源码 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/`（只读）
- 附件（唯一大表事实源）：[`compat/codegen-map.xray.yaml`](../../compat/codegen-map.xray.yaml)
- 关联：`compat/features.yaml`（config_types / core_capability_matrix）、`compat/fields.entities.yaml`、`compat/fields.yaml`、`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §12

> 约定：下文所有 `path:line` 均为冻结源码相对路径。生成 JSON 的成员名以 `v2rayN/ServiceLib/Models/CoreConfigs/V2rayConfig.cs` 的 C# 属性名（序列化不改名）为准。本文不写新代码、不改台账。

---

## 1. 入口、模板与生成顺序

调度：`CoreConfigHandler.GenerateClientConfig` 按 `RunCoreType` 分派，非 sing-box 一律走 `CoreConfigV2rayService`（`v2rayN/ServiceLib/Handler/CoreConfigHandler.cs:24-31`）。`Custom` 节点不走本生成器，改为复制文件（`CoreConfigHandler.cs:16-22,44-91`）。

入口：`CoreConfigV2rayService.GenerateClientConfigContent()`（`v2rayN/ServiceLib/Services/CoreConfig/V2ray/CoreConfigV2rayService.cs:13-85`）。

| 序 | 步骤 | 证据 |
|---|---|---|
| 1 | 反序列化基础模板 `Global.V2raySampleClient` → `V2rayConfig` | `CoreConfigV2rayService.cs:33-45`；`Global.cs:19` |
| 2 | `GenLog()` | `CoreConfigV2rayService.cs:47` |
| 3 | `GenInbounds()` | `:49` |
| 4 | `GenOutbounds()` | `:51` |
| 5 | `GenRouting()` | `:53` |
| 6 | `GenDns()` | `:55` |
| 7 | `GenStatistic()` | `:57` |
| 8 | `ApplyOutboundFragment()`（`CoreBasicItem.EnableFragment`） | `:59-62` |
| 9 | `ApplyFinalFragment()`（`EnableFinalFragment`） | `:63-66` |
| 10 | 追加 `BuildFinalRule()`（仅当有 balancerTag） | `:68-72` |
| 11 | `ApplyFinalConfigModifiers()`：BindInterface→SendThrough→CustomOutboundReplace→FullConfigTemplate | `V2rayConfigTemplateService.cs:5-13` |

基础模板结构（`v2rayN/ServiceLib/Sample/SampleClientConfig:1-22`）：`log.access=Vaccess.log`、`log.error=Verror.log`、`loglevel=warning`；`inbounds=[]`；`outbounds=[{freedom,direct},{blackhole,block}]`；`routing.domainStrategy=IPIfNonMatch`,`rules=[]`。

---

## 2. 输入字段清单（按 EConfigType）

真值表见 `compat/codegen-map.xray.yaml#protocol_output_map`。下表为概览；Xray 只生成能力集内 8 类 + 3 类复合/自定义（`Global.cs:366-376`）。

| EConfigType | 值 | Xray 生成 | 关键输入字段（→ 顶层输出路径） |
|---|---|---|---|
| VMess | 1 | 是 | Address/Port→settings.address/port；Password→id；ProtoExtra.AlterId→alterId；ProtoExtra.VmessSecurity→security |
| Shadowsocks | 3 | 是 | Address/Port；Password→password；ProtoExtra.SsMethod→method；ProtoExtra.Uot→uot |
| SOCKS | 4 | 是 | Address/Port；Username+Password→user/pass（两者都非空才写） |
| VLESS | 5 | 是 | Address/Port；Password→id；ProtoExtra.VlessEncryption→encryption；ProtoExtra.Flow→flow |
| Trojan | 6 | 是 | Address/Port；Password→password |
| Hysteria2 | 7 | 是 | Address/Port；Password→hysteriaSettings.auth；ProtoExtra.{Ports,UpMbps,DownMbps,HopInterval,SalamanderPass,Hy2RealmUrl,GeckoMin/MaxPacketSize}→finalmask |
| WireGuard | 9 | 是 | ProtoExtra.{WgInterfaceAddress,WgPublicKey,WgPresharedKey,WgReserved,WgMtu,WgDns}；Password→secretKey；Address/Port→peer.endpoint |
| HTTP | 10 | 是 | Address/Port；ProtoExtra.HttpHeaders→settings.headers；Username+Password→user/pass |
| Outbound | 13 | 是（自定义文件） | ProfileItem.Address 为文件路径；`context.CustomOutboundMap` + `{{tag}}/{{detour}}/{{interface}}` |
| PolicyGroup | 101 | 是（组展开） | ProtoExtra.ChildItems；ProtoExtra.MultipleLoad |
| ProxyChain | 102 | 是（链展开） | ProtoExtra.ChildItems→dialerProxy 链 |
| Custom | 2 | 否 | 由 `GenerateClientCustomConfig` 复制文件 |
| TUIC | 8 | 否 | 不在 Xray 能力集（仅 sing-box） |
| Anytls | 11 | 否 | 仅 sing-box |
| Naive | 12 | 否 | 仅 sing-box |

WireGuard 形状特殊：不写常规 `settings.address/port`，而是整体替换 `outbound.settings` 为 `Outboundsettings4Ray`（`V2rayOutboundService.cs:185-209`）。Hysteria2 的 `outbound.protocol` 会被改写为 `"hysteria"`（`:213-216`）。

---

## 3. 输出路径映射

### 3.1 按 network（`ETransport`）

传输提取见 `V2rayOutboundService.cs:250-304`，分派见 `:377-609`；`quic` 在入口被拒（`CoreConfigV2rayService.cs:25-29`），`h2/http` 经 `GetNetwork()` 归一到 `raw`（`Global.cs:330-338`；`ProfileItem.cs:53-60`）。

| network | 输出路径 | 关键输入 | 证据 |
|---|---|---|---|
| raw | `outbounds[].streamSettings.rawSettings.header.{type,request}` | TransportExtra.{RawHeaderType,Host,Path}；DefUserAgent | `:271-273,579-604` |
| ws | `streamSettings.wsSettings.{host,path,headers.User-Agent}` | TransportExtra.{Host,Path}；DefUserAgent | `:283-284,423-441` |
| httpupgrade | `streamSettings.httpupgradeSettings.{host,path,headers.User-Agent}` | TransportExtra.{Host,Path}；DefUserAgent | `:287-290,443-461` |
| xhttp | `streamSettings.xhttpSettings.{path,host,mode,extra}` | TransportExtra.{Path,Host,XhttpMode,XhttpExtra}；MuxEnabled | `:292-297,463-487` |
| kcp | `streamSettings.kcpSettings.*` + `streamSettings.finalmask.udp[]` | KcpItem.*；TransportExtra.{KcpMtu,KcpHeaderType,KcpSeed} | `:279,379-421` |
| grpc | `streamSettings.grpcSettings.{authority,serviceName,multiMode,idle_timeout,health_check_timeout,permit_without_stream,initial_windows_size,user_agent}` | TransportExtra.{GrpcAuthority,GrpcServiceName,GrpcMode}；GrpcItem.*；DefUserAgent | `:299-303,489-502` |
| hysteria | `streamSettings.hysteriaSettings.{version,auth}` + `finalmask.quicParams` + `finalmask.udp[]` | Hysteria2 节点字段 | `:504-575` |

raw 的 HTTP 请求体由模板 `SampleHttpRequest` 做 `$requestHost$/$requestUserAgent$/$requestPath$` 替换（`Global.cs:21`；`V2rayOutboundService.cs:590-604`）。`streamSettings.finalmask` 若节点 `ProfileItem.Finalmask` 非空则整体覆盖（`:611-614`）。

### 3.2 按 security（`ProfileItem.StreamSecurity`）

| security | 输出路径 | 输入 | 证据 |
|---|---|---|---|
| `tls` | `streamSettings.security=tls` + `streamSettings.tlsSettings.{alpn,fingerprint,echConfigList,verifyPeerCertByName,serverName,echForceQuery,certificates,disableSystemRoot,pinnedPeerCertSha256}` | Alpn/Fingerprint(Sni)/EchConfigList/VerifyPeerCertByName/Sni/Cert/CertSha/DefFingerprint | `:310-355` |
| `reality` | `streamSettings.security=reality` + `streamSettings.realitySettings.{fingerprint,serverName,publicKey,shortId,spiderX,mldsa65Verify,show}` | Fingerprint/Sni/PublicKey/ShortId/SpiderX/Mldsa65Verify | `:358-374` |

### 3.3 协议 → 出站 settings

见第 2 节与 `codegen-map.xray.yaml#protocol_output_map`。特别核对：

- **`allowInsecure`：Xray 生成器从不写入。** 源码仅在 `NodeValidator` 中当 Xray + insecure 且无证书时告警 `MsgAllowInsecureDeprecated`（`NodeValidator.cs:134-147`；`ResUI.resx:1794` 说明允许跳过证书验证将于 2026-08 被 Xray 移除）。这是 7.25.x 已移除的旧字段，不得回写。
- **`fakedns` 是顶层字段**，不在 `inbounds[].settings`：`_coreConfig.fakedns = new(){ipPool,poolSize}`（`V2rayDnsService.cs:148-152`；`V2rayConfig.cs:7`）。任务示例中的 `inbounds[0].settings.fakedns` 与源码不符，列入未确证/纠偏。

### 3.4 组/链

- PolicyGroup：`outbounds[].tag`；多节点触发 `GenObservatory`+`GenBalancer`（`V2rayOutboundService.cs:9-14,622-654`；`V2rayBalancerService.cs:5-115`）。
- ProxyChain：tag 命名为 `proxy`/`chain-proxy-i-Remarks`，串联点写 `streamSettings.sockopt.dialerProxy`，xhttp 额外写 `xhttpSettings.extra.downloadSettings.sockopt.dialerProxy`（`:656-761`）。

---

## 4. 全局设置注入

完整表见 `codegen-map.xray.yaml#global_injection`。摘要：

| 来源设置 | 注入路径 | 证据 |
|---|---|---|
| CoreBasicItem.Loglevel | `log.loglevel` | `V2rayLogService.cs:12,18` |
| CoreBasicItem.LogEnabled | `log.access/error`（false→null） | `V2rayLogService.cs:9-21` |
| CoreBasicItem.DefFingerprint | `tlsSettings.fingerprint` / `realitySettings.fingerprint` | `V2rayOutboundService.cs:317,364` |
| CoreBasicItem.DefUserAgent | ws/httpupgrade/grpc/raw 的 User-Agent | `:434-438,454-458,499,590-595` |
| CoreBasicItem.SendThrough | `outbounds[].sendThrough` | `V2rayConfigTemplateService.cs:259-271` |
| CoreBasicItem.BindInterface | `outbounds[].streamSettings.sockopt.interface`；TUN `settings.autoOutboundsInterface` | `V2rayConfigTemplateService.cs:225-257`；`V2rayInboundService.cs:84-88` |
| CoreBasicItem.EnableFragment | `outbounds[].streamSettings.finalmask.tcp += fragment` | `V2rayOutboundService.cs:769-791` |
| CoreBasicItem.EnableFinalFragment | 插入 `{tag}-fragment-freedom` freedom 出站 + 原出站 `sockopt.dialerProxy` | `:793-824` |
| Fragment4RayItem.{Packets,Lengths,Delays,MaxSplit} | `finalmask` 内 fragment.settings | `:826-865`；默认 `ConfigHandler.cs:168-188` |
| Mux4RayItem.{Concurrency,XudpConcurrency,XudpProxyUDP443} | `outbounds[].mux.*` | `:225-248` |
| KcpItem.* | `kcpSettings.*` | `:380-388` |
| GrpcItem.* | `grpcSettings.*` | `:494-499` |
| HysteriaItem.{UpMbps,DownMbps,HopInterval} | `finalmask.quicParams.*` 回退 | `:507-517` |
| HappyEyeballs4RayItem.* | `sockopt.happyEyeballs.*`（仅在写 domainStrategy 且 `SimpleDNSItem.EnableHappyEyeballs==true`） | `V2rayDnsService.cs:532-554` |
| SimpleDNSItem.* | `dns.*`、`outbounds[].targetStrategy`、`sockopt.domainStrategy` | `V2rayDnsService.cs:36-121,155-426` |
| SimpleDNSItem.FakeIP | 顶层 `fakedns.{ipPool,poolSize}` | `V2rayDnsService.cs:128-153` |
| RoutingBasicItem.DomainStrategy | `routing.domainStrategy` | `V2rayRoutingService.cs:40-48` |
| GuiItem.EnableStatistics/DisplayRealTimeSpeed | `stats{}`、`metrics.listen=127.0.0.1:{StatePort}`、`policy.system.statsOutbound{Up,Down}link=true` | `V2rayStatisticService.cs:7-22`；`AppManager.cs:15-22` |
| TunModeItem.* | `inbounds[tun].settings.*` | `V2rayInboundService.cs:56-135` |
| Inbound[0].* | `inbounds[].{port,listen,settings.udp,sniffing.*}` | `V2rayInboundService.cs:5-54,143-175` |
| RoutingItem.RuleSet | `routing.rules[]`（跳过 `RuleType==DNS`） | `V2rayRoutingService.cs:42-65` |

**Inbound 端口偏移**：`port = Inbound[0].LocalPort(default 10808) + (int)protocol`；`socks=0,socks2=1,socks3=2`（`AppManager.cs:165-169`；`EInboundProtocol.cs`；`V2rayInboundService.cs:157`）。LAN 时 `socks3` 监听 `0.0.0.0` 并可选 `settings.auth="password"` + `accounts`（`:26-53`）。

**DNS 规则与 RuleType**：Routing 规则生成时 `RuleType==DNS` 的项被跳过（`ERuleType.DNS=2`；`V2rayRoutingService.cs:57-60`）；DNS 项的域名在 `V2rayDnsService.cs:219-268` 被用于构造 `dns.servers[].domains/expectedIPs` 与 `tag=direct-dns-N`。`RuleType==Routing` 在 DNS 侧被跳过（`V2rayDnsService.cs:226-229`）。

**统计/API 端口**：Xray 用 `StatePort`（`Utils.GetFreePort(GetLocalPort(api))`；`AppManager.cs:15-22`）→ `metrics.listen`；sing-box 用 `StatePort2`。`Global.InboundAPIProtocol="dokodemo-door"`（`Global.cs:65`）在本生成器未见使用（未确证）。

**TUN（Xray 形态）**：模板 `SampleTunInbound`（`Global.cs:25`）。`settings.name=xray_tun`（mac `utunN`）；`MTU=TunModeItem.Mtu`（<=0→`TunMtus.First()=1280`）；`gateway=[IPv4Address or 172.18.0.1/30] (+IPv6)`；`autoSystemRoutingTable` 由 `HasGlobalIPv6Address` 与 `RouteExcludeAddress` Subtract/Supernet 推导；`sniffing.routeOnly` 强制 true（`V2rayInboundService.cs:56-135`；`Global.cs:765-789`）。

---

## 5. 配置模板与插入点

`ApplyFullConfigTemplate`（`V2rayConfigTemplateService.cs:86-223`）：

- 仅当 `FullConfigTemplate.Enabled==true` 且 `Config`/`TunConfig` 非空；`IsTunEnabled` 选 `TunConfig`，否则 `Config`（`:86-104`，`:94`）。
- balancer：模板 `routing.rules[outboundTag==proxy]` 改为 `balancerTag`；生成的 balancers 追加（`:107-146`）。
- observatory / burstObservatory：`subjectSelector` 去重合并（`:149-175`）。
- outbounds：**生成的在先**（`AddProxyOnly==true` 时跳过 `blackhole/dns/freedom`），**模板 outbounds 追加在后**（`:177-220`）。
- `ProxyDetour`：对非私网、无 `dialerProxy` 的出站写 `streamSettings.sockopt.dialerProxy`（`:190-208`）。
- `ApplyCustomOutboundReplace`（`:15-84`）：按 `tag` 替换自定义出站，支持 `{{tag}}/{{detour}}/{{interface}}`；`xhttpSettings.extra.downloadSettings.sockopt` 同步。
- TUN/端口 `< 11808` 约束：按项目 AGENTS.md，测试不得使用 10808。

sing-box endpoint 在 T07 不适用（Xray 无 endpoints 段）。

---

## 6. 生成顺序与覆盖规则

完整表见 `codegen-map.xray.yaml#precedence_rules`。要点：

1. 模板提供骨架，随后 `log/inbounds/outbounds/routing/dns` 逐段覆盖（`CoreConfigV2rayService.cs:33-57`）。
2. `GenInbounds` 直接 `inbounds=[]` 重建，模板 inbounds 不保留（`V2rayInboundService.cs:11`）。
3. 节点字段在模板之后填充；**仅当非空才写**的可选字段大量存在（Username/Password、CertSha、echConfigList、Ports、SalamanderPass、BindInterface、SendThrough 等，`V2rayOutboundService.cs:66-217`；`V2rayConfigTemplateService.cs:225-271`）。
4. 默认回退一览：`DefFingerprint`、`DefUserAgent`（`CoreBasicItem`，可为空串）；`DefaultSecurity="auto"`；`DefaultNetwork="raw"`；`WgInterfaceAddress→172.16.0.2/32`；`WgMtu→TunMtus.First()=1280`；`HysteriaItem.HopInterval<5→Hysteria2DefaultHopInt=30`；fragment `Packets=tlshello/Lengths=[50-100]/Delays=[10-20]/MaxSplit=0`（`ConfigHandler.cs:168-188`）。
5. **后写覆盖前写**：`ProfileItem.Finalmask` 覆盖 `streamSettings.finalmask`（`V2rayOutboundService.cs:611-614`）；`ApplyCustomOutboundReplace` 覆盖整段出站；`ApplyFullConfigTemplate` 最后整体替换 `outbounds`。
6. **禁止的旧字段**：Xray 输出不得含 `allowInsecure`/`insecure`（已在 7.25.x 移除，`NodeValidator.cs:134-147`）；旧 `HeaderType/RequestHost/Path/Extra/Ports/AlterId/Flow/Id/Security`（`ProfileItem.cs:173-218` 均标 `[Obsolete]`，迁移后由 ProtoExtra/TransportExtra 取代，`AppManager.cs:134,159`）。

---

## 7. 差分测试建议（最小合成输入）

用于 T06b/T07/T08 的差分用例；合成数据不得含真实凭据。断言路径以生成 JSON 为准。测试端口一律 ≥ 11808。

| # | 输入（最小合成） | 关键断言路径 |
|---|---|---|
| D1 | VLESS + tcp/raw + reality；`Address=192.0.2.10,Port=443,Password=<uuid>,PublicKey=<synthetic>,ShortId=<hex>,Sni=example.test,Fp=chrome,Flow=""` | `outbounds[0].protocol=="vless"`；`outbounds[0].streamSettings.security=="reality"`；`outbounds[0].streamSettings.realitySettings.publicKey/shortId/serverName`；`outbounds[0].streamSettings.network=="raw"`；**不存在** `allowInsecure` |
| D2 | VMess + ws + tls；`Network="ws",TransportExtra.Host="ws.test",Path="/p",StreamSecurity="tls",CertSha=<sha>,Alpn="h2"` | `streamSettings.wsSettings.host=="ws.test"`、`path=="/p"`；`streamSettings.tlsSettings.pinnedPeerCertSha256`；`mux.enabled==true`（`Flow` 空 + `MuxEnabled`） |
| D3 | Hysteria2 + `Ports="20000-30000,40000"`、`UpMbps=50,DownMbps=0`、`SalamanderPass="pw"` | `outbounds[0].protocol=="hysteria"`；`streamSettings.network=="hysteria"`；`hysteriaSettings.auth`；`finalmask.quicParams.udpHop.ports=="20000-30000,40000"`；`congestion=="brutal"`；`brutalUp=="50mbps"`且无 `brutalDown`；存在 `type:salamander` mask |
| D4 | WireGuard + `WgInterfaceAddress="10.0.0.2/32,fd00::2/128"`、`WgMtu=1420`、`WgReserved="1,2"` | `outbounds[0].protocol=="wireguard"`；`settings.address` 为长度 2 列表；`settings.secretKey`；`settings.peers[0].endpoint=="192.0.2.10:443"`；`settings.mtu==1420`；`settings.reserved==[1,2]` |
| D5 | kcp + `KcpHeaderType="wechat-video"`、`KcpSeed="seed"`、`KcpItem.Mtu=1350` | `streamSettings.kcpSettings.mtu==1350`；`finalmask.udp` 存在 `type:mkcp-legacy` 且 `settings.header=="wechat"`、另一项 `settings.value=="seed"`；`finalmask.udp` 顺序被 Reverse |

补充差分点：DNS 段（`dns.servers[].tag`、`fakedns.ipPool`）、TUN inbound（`settings.gateway/MTU`）、组展开（`balancerTag` 与 `routing.balancers`）。

---

## 8. 能力矩阵复核

| 集合 | 精确值 | 代码行 |
|---|---|---|
| `Global.XraySupportConfigType` | {VMess, VLESS, Shadowsocks, Trojan, Hysteria2, WireGuard, SOCKS, HTTP} | `v2rayN/ServiceLib/Global.cs:366-376` |
| `Global.SingboxSupportConfigType` | {VMess, VLESS, Shadowsocks, Trojan, Hysteria2, TUIC, Anytls, Naive, WireGuard, SOCKS, HTTP} | `Global.cs:378-391` |
| `Global.SingboxOnlyConfigType` | `Singbox.Except(Xray)` = {TUIC, Anytls, Naive} | `Global.cs:393` |
| 阻断网络 | `quic`（入口拒绝） | `CoreConfigV2rayService.cs:25-29` |

---

## 9. 未确证清单（Xray）

| ID | 项 | 说明 | 证据 |
|---|---|---|---|
| UNC-X-001 | v2fly/v2fly_v5 是否经本生成器 | `CoreConfigHandler` 仅区分 sing_box，`Global.CoreTypes` 仅 Xray/sing_box，界面可选性未核实 | `CoreConfigHandler.cs:24-31`；`Global.cs:360-364` |
| UNC-X-002 | Custom 的 pre-socks 出站生成细节 | `configPre.json` 由 `CoreManager` 生成，本次未逐行确证 preContext 出站 | `CoreManager.cs:194-212`；`ConfigHandler.cs:1555-1586` |
| UNC-X-003 | `fakedns` 路径 | 源码为顶层 `fakedns`，非 `inbounds[].settings.fakedns`；任务示例路径被源码否定 | `V2rayDnsService.cs:148-152`；`V2rayConfig.cs:7` |
| UNC-X-004 | h2/http/quic 隐藏分支 | `GetNetwork()` 归一化，network switch 无对应 case；无法确证外部注入 | `Global.cs:330-338`；`V2rayOutboundService.cs:377-608` |
| UNC-X-005 | `Global.InboundAPIProtocol`（dokodemo-door）是否被使用 | 生成器未见引用 | `Global.cs:65` |

---

## 10. 源码索引

- 入口/编排：`CoreConfigV2rayService.cs`
- 出站/传输/安全：`V2rayOutboundService.cs`
- 入站：`V2rayInboundService.cs`
- 路由：`V2rayRoutingService.cs`
- DNS：`V2rayDnsService.cs`
- 日志：`V2rayLogService.cs`
- 统计：`V2rayStatisticService.cs`
- 负载均衡：`V2rayBalancerService.cs`
- 模板合并：`V2rayConfigTemplateService.cs`
- 模型：`Models/CoreConfigs/V2rayConfig.cs`
- 常量/能力集：`Global.cs`
- 校验：`Handler/Builder/NodeValidator.cs`
- 采样模板：`Sample/Sample{ClientConfig,Outbound,Inbound,TunInbound,TunRules,HttpRequest}`
