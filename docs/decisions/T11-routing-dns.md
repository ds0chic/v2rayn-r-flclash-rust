# T11 决策记录 — 路由与 DNS 全量

- 状态：implemented（跨平台未验证；远程模板类事项见未决）
- 关联：`docs/evidence/T11.md`、`compat/features.yaml`（F-ROUTING-001..006、
  F-DNS-001..009）、`compat/fields.entities.yaml`（FLD-ENT-116..150）、
  `docs/decisions/T07-xray-codegen-contract.md`、`T08-singbox-codegen-contract.md`、
  `T07-T08-codegen-impl-notes.md`

## D11-1 规则顺序即数组顺序

`RoutingProfile.rule_set` 为 JSON 数组文本；移动操作只重排数组并同步
`rule_num`（上游 `MoveRoutingRule` + `RuleNum=_rules.Count` 对等）。
生成器按数组顺序追加规则，矩阵用例 `multi-rule-order` 双核断言保序。

## D11-2 悬空 OutboundTag：回退但不静默

与 T06b 一致：内置 tag（proxy/direct/block）直通；备注引用须命中 live
profile，否则回退 `proxy` 并产生结构化 warning（引擎层
`routing_dangling_reference` / `routing_empty_outbound`，生成器层同名诊断）。
上游 `CoreConfigContextBuilder` 行为相同（仅 UI 提示形式不同）。

## D11-3 端口解析权：opts 优先于 settings

`CodegenOptions.local_port/state_port/state_port2` 为调用方解析的运行时
上下文（含测试空闲端口）；`settings_from_app` 投影除入站端口外的全部树
内容。生产调用方须把配置的入站端口填入 `opts.local_port`
（T10 矩阵即此用法；改动前 T10 行为保持不变，未动其测试逻辑）。

## D11-4 RuleMode 的生成语义

上游 `ERuleMode` 只驱动 Clash/mihomo 侧（`clash_mode` + Clash API），Xray
无对应分支。为满足 F-ROUTING-001“三态路由段不同”且可真实校验：
Rule=上游行为；Global=用户规则旁路+单条 proxy catch-all；
Direct=单条 direct catch-all。sing-box 保留 sniff/hijack-dns/clash_mode
基座并将 `final` 指向模式出站。`rule_mode` 以 `None`=Rule 加入
`CodegenInput`（serde default，既有测试不受影响），持久化于
`guiNConfig.json` 的 `rule_mode` 键。

## D11-5 DNS 双通道

Raw（`DNSItem.enabled` + Normal/TunDNS）与 Simple（`SimpleDNSItem`）互斥
优先级沿用生成器既有实现；本轮只做装配：
`dns_to_codegen`（行+树+只读系统 hosts+保护域名表）。系统 hosts 只读解析，
合并顺序 common→system（TryAdd）→custom（覆盖）与上游一致。
sing-box 自定义 DNS 沿用严格 `Dns4Sbox` 校验（空/无 type 拒绝）。

## D11-6 区域预设离线诚实策略

Default 全量本地可做；Russia/Iran 的 URL（Geo/SRS/RouteRules）可本地写，
但三份远程 DNS/路由模板需联网。无网时启用自定义 DNS + 内嵌默认保证生成
可用，远程 URL 以 `pending_remote_templates` 逐项返回并在 UI 标注，
不写入伪造的区域内容。

## D11-7 自定义规则集（sing-box）

`CustomRulesetPath4Singbox` 的文件 IO 在应用层（`read_custom_ruleset`：
缺失/不可读/不可解析→None），生成器只收解析后的 JSON。
条目须带 tag/type/format（上游 `SingboxRulesetService` 同条件）。
`sing-box check` 会打开 local 条目路径：矩阵使用绝对路径的
source-format 规则集 JSON（相对路径按 CWD 解析已被真实踩到）。
