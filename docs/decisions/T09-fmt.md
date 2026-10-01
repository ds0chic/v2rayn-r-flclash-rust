# T09-fmt — 与上游 Fmt/订阅行为的差异与理由

事实源：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Handler/Fmt/*`
与 `Handler/ConfigHandler.cs`、`Handler/SubscriptionHandler.cs`、`Common/Utils.cs`、
`Global.cs`、`Manager/CertPemManager.cs`。以下仅列出**有意**的偏差。

## D1. 分层：纯库 + 下载适配，无文件系统副作用

上游 `V2rayFmt`/`SingboxFmt`/`ClashFmt`/`ResolveFull2`/`InnerFmt` 会把整份配置或
`CustomOutboundObj` 写入临时文件，并把路径存进 `ProfileItem.Address`。本 crate 的
`parse_content`/`resolve_custom` 保持纯函数：原文写入 `Profile.extra["RawConfig"]`
（`fmt::batch::RAW_CONFIG_KEY`），`Address` 留空。理由：T09 边界是“纯函数 + 下载适配”，
临时文件的生命周期与清理应由上层持久化/运行层决定；同时避免测试夹具与用户路径耦合。
下层 config_codegen 若需要文件，可在此键上落地。

## D2. InnerFmt 的 JSON 形状

- 上游内层 JSON 使用 PascalCase 字段，并把 `ProtoExtra`/`TransportExtra` 序列化为**字符串**
  （扁平化自 `*Obj`），导出前 `RemoveEmptyJson` 去空。
- 本实现使用 `domain::Profile` 的 serde 形状（`snake_case`、`config_type` 为数字），
  `proto_extra`/`transport_extra` 为**嵌套对象**；导入端兼容三种拼写
  （`proto_extra` 对象、`ProtoExtraObj` 对象、`ProtoExtra` 字符串），导出端产出嵌套对象。
- 理由：domain（T02）已定结构化模型，plan §11 禁止“blob-everything”。因此 v2rayn:// 与
  上游不是逐字节互操作，但语义（Id 重映射、`self` 哨兵、版本=4、CoreType∈{null,Xray,sing_box}、
  跳过 Custom、空组剔除）一致；已用测试固定。
- `index_id` 可复现导出：上游用 `session salt + HashCode.Combine & 0x7FFFFFFF` 的 4 字节
  base64；本实现用 `DefaultHasher(salt,id)` 取低 31 位 + 4 字节 LE base64，语义等价（会话内稳定、
  跨会话变化）。

## D3. 数值/空值的序列化

VMess 的 `v/port/aid` 上游以字符串写出（`JsonNumberHandling.WriteAsString`），导入接受数字或
字符串；本实现显式以字符串写出、导入两者皆收。空 `Option` 序列化为 `null` 并在 InnerFmt 导出时
按上游 `RemoveEmptyJson` 规则剔除（null/空串/空对象/空数组），布尔与数字保留。

## D4. 百分号解码只做一次

沿用上游 `BaseFmt` 注释：`ParseQueryString` 已解码一次，`GetQueryDecoded` 不再二次解码。
`util::Query::parse` 对键与值各做一次 `url_decode`，`=` 只按**首个**分割（RFC 3986 允许值含 `=`）。
因此 `ob%41fs`、`AAj%2B...==` 等边界与上游一致（有测试）。

## D5. SocksFmt 的 legacy 分支判定

上游 `ResolveSocksNew` 先以 `Uri` 解析，legacy base64（无 `@`）在某些实现下会被当成 host
（`Port=-1`）而提前返回，产生退化节点。Rust `url` crate 会接受含 `=` 的 host，行为不同。
本实现改为：**仅当新式解析得到非空地址且端口 > 0 时才采用**，否则回退 legacy base64 解码；
legacy 结果为空/端口 0 才报错。这样真实 legacy 形态可解析，且不产生垃圾节点（有测试）。

## D6. Hysteria2 端口范围与 gecko

- `Ports` 内部存 `host:port` 风格 `5000:6000`，导出为 `mport=5000-6000`；导入**原样**保留
  `5000-6000`（不还原 `:`）。这是上游既有行为（测试 `GetShareUriAndResolveConfig_Hysteria2_*`
  即断言导入后为 `5000-6000`），故保留。
- `pinSHA256` 存在时自动置 `insecure=true`（上游为兼容 Xray 自签名链接）；`gecko` 缺省
  `minPacketSize=512`/`maxPacketSize=1200`。
- 证书指纹：上游会按 BasicConstraints 跳过 CA 证书；本实现取 PEM 链首证书的 SHA-256
  大写十六进制。理由：纯 Rust 无 x509 解析依赖，且实际链通常 leaf 在前。

## D7. 正则保护

上游 `Utils.IsRegexMatch` 用 .NET 正则 + 2s 超时，非法/超时 fail-open（返回 true）。
本实现用 Rust `regex`（回溯安全、线性时间）并限制 pattern 长度与编译尺寸；非法或超长
fail-open。副作用：.NET 专有语法（lookaround/backreference）无法编译，会 fail-open，即
“不过滤也不丢节点”，与上游 fail-open 语义一致。`PolicyGroup` 的默认 lookahead 过滤属于
配置生成层（非 T09）范畴。

## D8. 下载器：代理与重定向

- 明确 `no_proxy()`，仅当传入 `ProxyConfig` 时加 `Proxy::all`；不读取环境变量代理（plan §15）。
- 代理仅接受 `http://`/`https://`：本地离线依赖缓存无 `tokio-socks`，故不启用 reqwest `socks`
  feature。上游支持 socks5 本地代理，此处作为未决项记录。
- 手动跟随重定向（`Policy::none` + 循环），跨 origin（scheme/host/port 任一变化）时显式移除
  `Authorization`/`Cookie`/`Proxy-Authorization`，并对总次数设上限。这样可被本地测试验证“不泄漏认证头”。
- 超时为**整次下载**预算（含重定向）；上游为连接/读取分档。合理解释见 plan §15“超时…都要明确”。
- 体积上限用流式 `chunk()` 累加，超限返回 `TooLarge`；`Content-Length` 先行短路。

## D9. 检测顺序

`ContentHint::Auto` 复刻 `ConfigHandler.AddBatchServers`：base64 列表 → 明文列表 → base64 回退 →
SIP008 → WireGuard `.conf` → 内层 `v2rayn://`（可与明文混合累加）→ 结构化 custom
（Xray→sing-box→Clash→Hysteria2）。HTML 检测要求 `<html`+`<!doctype html`+`<head` 三者同时出现
（上游 `BaseFmt.Contains` 的 `All` 语义），因此仅命中一处不会误判为 HTML。

## D10. 网络 token 大小写

`Global.Networks` 成员判定在上游是**大小写敏感**的；本实现 `base::get_network` 同样精确匹配，
空/未知回退 `raw`。注意 domain 的 `Profile::network()` 会小写化（另一用途），T09 不使用它，
以免把 `WS` 误判为 `ws`。

## D11. Merge / Refresh 语义

上游刷新时先 `RemoveServersViaSubid` 再导入，失败/空结果**不删除**旧集合
（`ProcessDownloadResult` 仅在结果非空且成功时替换）。本实现以
`RefreshOutcome::{Replaced, PreservedOnEmpty, PreservedOnError}` 显式表达，`KeepOlderDedupl`
决定去重保留旧/新，`Filter` 正则过滤先于去重；去重键与 `CompareProfileItem(..., remarks=false)`
一一对应。
