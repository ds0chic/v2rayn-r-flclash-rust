# T07/T08 实现笔记 — 生成器输入模型与已冻结取舍

- 任务：T07 Xray / T08 sing-box 配置生成（实现）
- 状态：implemented（真实内核校验未运行，属 T06b/后续）
- 关联：`docs/decisions/T07-xray-codegen-contract.md`、`T08-singbox-codegen-contract.md`、
  `docs/evidence/T07-T08-codegen.md`、`crates/config_codegen`

## 1. 输入模型（`config_codegen::input`）

生成器不依赖 `domain` crate，也不做 IO。运行时上下文一律以显式字段传入：

- `CodegenInput { profile, profiles, custom_outbound_content, tun_rules, tun_singbox_rules,
  singbox_fakeip_filter, dns_v2ray_normal, dns_singbox_normal, tun_singbox_dns, settings, routing,
  dns, template }`
  - `profiles`: 以 `IndexId` 为键的全部节点，用于组展开与路由 `Remarks→tag` 解析。
  - `custom_outbound_content`: `IndexId -> 原始 JSON 文本`（Outbound 类型）。
  - 各样例字段是 fixtures 内容的纯数据副本，测试用其读取 `fixtures/source/upstream/sample/*`。
- `CodegenSettings` 承载 `CoreBasic/Fragment4Ray/Mux4Ray/Mux4Sbox/Kcp/Grpc/Hysteria/
  HappyEyeballs4Ray/Gui/Inbound/Tun/RoutingBasic`，另加：
  - `platform`（Windows/MacOS/Linux，影响 `sing-box.exe`/进程路径/TUN 名称）
  - `state_port` / `state_port2`（`AppManager.StatePort/StatePort2` 的解析结果）
  - `log_directory` / `bin_directory` / `log_date`（合成路径，测试使用相对目录）
  - `speed_ping_test_url`（observatory/urltest 使用）
  - `protect_core_executables`（TUN direct-exe 规则，替代 CoreInfoManager 推导）
  - `has_global_ipv6_address`（TUN 路由表推导）
  - `ruleset_url` / `local_srs_files`（sing-box srs 远程/本地判定）
- `CodegenDns` 区分 Raw DNS（`enabled` + `normal`/`tun`）与 `SimpleDns`；`system_hosts` 由调用方读入。
- `CodegenTemplate` 承载 `config`/`tun_config` 文本、`add_proxy_only`、`proxy_detour`。
- `CodegenRouting.custom_ruleset` 为已解析 JSON（文件读取在调用方）。

设计理由：可测试、可重放、无副作用；把“读文件/读宿主机/随机数”统一挤出纯函数边界。
代价是调用方需做少量装配（fixtures 读取、StatePort 解析、平台探测）。

## 2. 输出模型

`GeneratedConfigs { main: Value, files: Vec<GeneratedFile>, diagnostics: Vec<Diagnostic> }`。
当前两内核均只产出 `main`（`files` 为空）；`Custom` 透传返回解析后的 `main` 与
`custom_passthrough` 诊断。序列化遵循上游 `WhenWritingNull` 语义：可选字段为空即不写出。

## 3. 有意取舍（与契约/源码的关系）

1. 生成顺序、覆盖规则、默认回退严格按契约与 `compat/codegen-map.*.yaml` 的 `precedence_rules`：
   模板骨架 → log → inbounds（重建）→ outbounds（前置插入）→ routing → dns → stat/experimental
   → fragment → final rule → Bind/SendThrough → CustomReplace → FullTemplate。
2. 模板出站顺序：Xray 按“生成在前、模板在后”（`V2rayConfigTemplateService.cs:177-220`）；sing-box
   上游同样是“生成在前、模板在后”（`SingboxConfigTemplateService.cs:110-125`，模板 `outbounds`
   数组先取出、逐个 `customOutboundsNode.Add(生成出站)`）。T06b 已按上游源码核实本实现方向正确，
   契约 §5 表述即此语义。
3. quic：Xray 显式拒绝（契约），sing-box 归一 raw（契约）。
4. 引用悬空：**与上游一致，保留回退 `Global.ProxyTag`**（Xray `V2rayRoutingService.cs:189-195`；
   sing-box 同源），但 T06b 起补发结构化 `routing_dangling_reference` warning 诊断，不再静默；
   组/链子项缺失、自定义出站内容缺失仍为硬错误（`dangling_reference` / `custom_outbound_missing`）。
   （此前本文件“一律结构化报错”的表述过宽，已按上游语义更正。）
5. 所有随机/时间/宿主相关内容改为输入字段或确定性默认，便于语义差分。
6. 保留端口：`settings.inbound.localPort` / `statePort` / `statePort2` 为 10808 时一律拒绝
   （`reserved_port`），防止生成器写出宿主在用的活代理端口。
7. reality 校验放宽为“任何协议类型 + `stream_security==reality` + `publicKey` 空 → 报错”。
8. sing-box 对无法承载的 transport（非 `raw` 的非 VMess/VLESS/Trojan/SS 协议；非 raw/ws 的 SS）
   补发 `singbox_transport_ignored` warning；`endpoints` 仅在非空时写出。

## 4. 差分辅助

`config_codegen::diff::Normalizer` 支持按键忽略、路径前缀归一、精确值替换；`semantic_diff`
对对象无序、数组保序比较。测试覆盖随机端口与临时路径两种归一化场景，供 T06b 与真实内核
输出对照使用。
