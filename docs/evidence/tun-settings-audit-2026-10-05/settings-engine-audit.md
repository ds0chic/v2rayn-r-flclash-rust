# 网络、内核与 DNS 设置功能审计

本轮结论：不能宣称这些设置全部真实有效。89 个网络/内核字段已逐行登记，既有消费者大多存在，但本轮新发现的消费者漏接和提交语义缺陷足以阻断“完整移植已完成”的结论。`implemented` 只表示消费者实现可定位，不表示用户完整流程已在真实内核和系统中通过。

审计基线：HEAD `672e666` 加审计开始前已有未提交改动，尤其 `crates/application/src/codegen.rs` 的 adapter name 修复。本轮没有改生产代码、冻结上游、台账或发布包。冻结原版是 v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 范围和证据等级

- 逐字段表：`engine-fields.csv`，89 行；与桌面设置报告、root 的 Config 25 个容器行合并后才是全部 180 行。该表不重复桌面代理负责的 Const/CoreType/CheckUpdate。
- 分组：CoreBasic 9、Inbound 11、Kcp 6、Grpc 4、Tun 11、SpeedTest 8、RoutingBasic 3、Mux4Ray 3、Mux4Sbox 3、Hysteria 3、Fragment4Ray 6、SimpleDNS 18、HappyEyeballs 4。
- 本轮状态：implemented 71，identified 11，blocked 7；全部 `actual_effect_verified=false`。不把静态映射、投影断言或历史 README 的完成宣称当成本轮真机验收。
- `engine-probe` 使用当前实际 application/domain/config_codegen 库做纯验证、真实配置生成以及专用 scratch 文件写入；没有 mock 生成器，没有启动 core/net-host/helper，没有创建 socket、网卡或路由，没有写宿主代理、自启或注册表。
- probe 最终 lock 从根 `Cargo.lock` 复制后固定，依赖包版本与根 lock 一致，唯一新增包为 probe 自身。首次离线解析曾选择缓存较新包，未将其结果作为最终证据；最终以 `locked-replay.log` 为准。

## 新发现

### AUD-ENG-01 / P1：Happy Eyeballs 开关实际上不控制生成

触发流程：在 DNS/参数设置中配置 Happy Eyeballs 参数，例如 `TryDelayMs=20`，将 `EnableHappyEyeballs` 关闭，保存并重启核心。

当前路径：`application/codegen.rs:578` 将开关投影到 `CodegenDns.simple.enable_happy_eyeballs`。但 `config_codegen/xray/dns.rs:225,262` 无条件传入参数，`:349..381` 只检查参数对象是否等于默认对象，不读取开关。全仓消费者检索确认此开关没有其他生产读取位置。

本轮纯生成复现：开关 false 仍输出 `/streamSettings/sockopt/happyEyeballs`；将开关改成 true，整个 Xray 生成结果与 false 完全相同。若参数全部为 null，打开开关仍不会输出对象。

冻结原版：`ServiceLib/Services/CoreConfig/V2ray/V2rayDnsService.cs:543` 明确检查 `EnableHappyEyeballs != true` 并返回；打开后 `:547..553` 写入参数。这是明确的行为不一致。

影响字段：FLD-CFG-176，以及 FLD-CFG-177..180 参数与开关的联动。其他 DNS 字段的消费者存在不能抵消这个缺口。旧 `R4-13.S19` 的测试只断言投影开关值，`S11` 只断言参数投影，均没有验证关掉开关后的最终配置。

修复验收：关闭开关时任何参数组合均不输出 happyEyeballs；打开时按原版生成；覆盖自由出站与代理拨号策略、`AsIs` 与非 `AsIs`、原始 DNS 开关。用户窗保存→重开→配置对比必须在同一测试中验证。

### AUD-ENG-02 / P1：原版合法的 MaxSplit 范围无法保存

触发流程：Fragment 最大拆分数输入 `1-3` 后保存。

当前拒绝点：`apps/desktop/lib/features/settings/option_setting_window.dart:358..363` 只接受 `int.tryParse`；`crates/application/src/settings.rs:91..96` 只接受 `parse::<u32>()`。

冻结原版：`OptionSettingViewModel.cs:303` 使用 `Utils.TryParseMaxSplit`；`Utils.cs:634..661` 接受 0..10000 以内的单值和合法升序范围；`V2rayOutboundService.cs:843..847` 生成时取范围首项。本实现生成器 `xray/config.rs:253..262` 已经会取首项，提交前验证却拒绝同一合法值。

本轮纯 Rust 复现：`validate_settings(MaxSplit="1-3")` 返回 `E_FIELD_FORMAT`，字段 `Fragment4RayItem.MaxSplit`。

修复验收：Dart 和 Rust 统一接受空、0、1、`1-3`、`0-10000`；拒绝倒序、负值和超界；保存范围文本、重开保留、最终配置生成结果与原版一致。不能只修 UI 或只修 Rust。

### AUD-ENG-03 / P1：TUN 自身内核流量保护规则存在生成实现，但生产上下文为空

`CodegenSettings.protect_core_executables` 在 `config_codegen/input.rs:464` 定义，Xray `routing.rs:166` 与 sing-box `routing.rs:93` 会据此生成内核进程的 DNS/直连保护规则。然而 application、bridge、net-host 的生产代码没有任何赋值；出现的非定义/消费赋值仅在测试与 examples。`settings_from_app` 的结果始终为 `[]`。

冻结原版：`Handler/Builder/CoreConfigContextBuilder.cs:53` 在 TUN 时设置受保护核心集合，并在前置服务时加入真实运行核心；`SingboxRoutingService.cs:301..335` 将其解析为真实可执行文件路径。重构的 `pre_socks_of` 会消费 `EnableLegacyProtect` 构建旁路进程，但没有传递对应保护路径。

确认范围：本轮确认生产配置缺少预期保护规则。不能据此宣称真实回环已经发生；路由回环、DNS 回环与多个内核并行的实际影响仍须在隔离环境中验证。`auto_detect_interface=true` 不能代替上游为其他核心进程生成的保护规则。

修复验收：应用组装层从当前真实 RuntimePlan 核心/旁路进程的解析安装路径构建保护上下文，由纯生成器消费；测试原生 sing-box、Xray、legacy protect 前置核心与自定义内核，不依赖按进程名猜测。随后验真实节点拨号不再被自身 TUN 重捕获。

### AUD-ENG-04 / P1：Xray TUN 的全局 IPv6 能力上下文始终 false

`CodegenSettings.has_global_ipv6_address` 在 `input.rs:465` 默认 false；生产组装层没有采样/赋值。Xray `inbound.rs:89,97` 据此决定 `autoSystemRoutingTable` 是否包含 `::/0`。冻结原版 `CoreConfigContextBuilder.cs:52` 读取 `Utils.HasGlobalIPv6Address()`。

本轮纯生成复现：`EnableIPv6Address=true`、`IPv6Address="fd00::1/64"` 时网卡地址已加入，但默认系统路由仍只有 `0.0.0.0/0`，上下文为 false。这证明地址开关不等于完整 IPv6 路由效果。

确认范围：IPv6 能力上下文缺失确定；本机真实公网 IPv6 可用性、泄漏和 auto-route 效果本轮未验证。不要把此结果扩展成所有内核 IPv6 均失效，sing-box 使用自己的 auto-route 配置。

修复验收：对真实平台 IPv6 能力生成只读快照并传入一次性生成上下文；分别覆盖全局 IPv6 有/无、IPv6 地址开/关、路由排除含/不含 IPv6；随后在隔离 IPv6 网络记录系统路由与实际流量。

### AUD-ENG-05 / P1：分组保存绕过存储不可用的保护（root AUD-ROOT-04）

由 root 首先定位，本 probe 交叉复现。全树保存 `engine.rs:1630..1636` 和读取设置有 `guard_storage`；分组保存 `:1679` 无同等检查。

本轮实际结果：构造专用 scratch 的 `AppEngine::storage_unavailable`，读取设置被拒绝；随后 `save_settings_group("UiItem", {"CurrentLanguage":"en"}, 0)` 返回 Ok，并创建 `guiNConfig.json`。这是本轮真实 scratch 写入证据，不是 mock 保存结果。该路径会把不可读/损坏存储旁的默认配置写成“成功”，违反失败后保持原文件的要求。

修复验收：整个存储不可用状态下，全树/分组保存、窗口几何等所有写入入口均拒绝，revision、内存及旧文件保持不动；重试必须来自修复后的真实重新打开。重点对比读取错误后点击保存时的可见错误。

### AUD-ENG-06 / P2：TUN 排除地址的空条目处理与原版不同

当前 `option_setting_window.dart:1244` 使用 `v.split(',')`；原版 `Utils.String2List` 删除换行并以 `RemoveEmptyEntries` 拆分逗号。当前输入 `10.0.0.0/8,` 后保存为 `["10.0.0.0/8", ""]`。`runtime/tun.rs:112..115` 验证每项时拒绝空 CIDR。

本轮纯计划验证复现返回 `E_INVALID_PLAN`，`route_exclude[1]` 非法。原版接受这个末尾逗号。逗号加换行的条目会被 CIDR parser trim，不能错误地宣称所有多行粘贴都会失败；纯换行分隔也不是原版 String2List 的分隔约定。

修复验收：匹配原版空列表/null、删除换行、逗号拆分及空项语义；字段保存前显示非法 CIDR 的具体行/项。原版多行控件应恢复，不能让单行长文本难以操作。

### AUD-ENG-07 / P2：RoutingIndexId 的旧配置一次迁移缺少消费者

该字段不是正常当前路由选择开关。冻结原版 `ConfigHandler.cs:2616..2623` 会在已有路由时按旧 `RoutingIndexId` 设置活动组，再清空旧值。当前该字段只有 domain/DTO/default/timing 的序列化引用，`engine.rs:1223 default_routing` 只读取 `RoutingProfile.is_active` 或首项，没有一次迁移读取。

旧 `R4-13.S18/README.md` 写“RoutingIndexId 由 routing 引擎选活动路由组消费”，与当前生产代码不符。当前正常 UI 选择路由走 `set_default_routing` 并保存 is_active，其消费者成立；缺口限定为旧配置迁移，不应扩展为正常路由选择完全失效。

本轮是静态确认，旧配置整包迁移实测未运行。修复需用两个合成路由、旧 index 指向非首项、无 is_active、重开后的活动选择与清空旧字段来验一次迁移。

## 会影响用户判断的现状

1. TUN 页 `option_setting_window.dart:1183` 仍写“本页仅保存配置”，而真实窗口保存后 `SettingsController.saveAndApply` 会 await `applyActive`。用户无法据此判断保存是否会启动 TUN/触发 UAC。文案必须描述真实产品动作，不应残留任务编号。
2. `AutoRoute/StrictRoute/Stack/IcmpRouting` 的原版消费者主要是 sing-box；Xray 通过 `autoSystemRoutingTable` 实现自己的 TUN 路由，忽略这些 sing-box 字段的行为有上游依据。不能拿“11 字段都生成到模型”宣称每字段在所有核都有效；也不能把禁用 sing-box 的 AutoRoute 当成 Xray 不改路由的保证。
3. KCP/gRPC、Mux4Ray、Mux4Sbox、Hysteria 默认值均依赖匹配核心/传输与节点级启用或覆盖条件；CSV 留有作用域，不将“不适用此节点”当作消费者缺失。
4. 原始自定义 DNS 启用时 simple DNS 的部分设置会被跳过，属于原版优先级；验收必须覆盖 raw/simple 两种模式，而不是只验证字段能存盘。
5. 其他模块交叉复核已通知桌面审计代理：CoreTypeItem 有真实 `resolve_target_core` 独立读取；ClashUI.EnableIPv6/Mixin 投影 helper 没有生产调用；根证书 provider 没有真实 HTTP/TLS 消费；资源模板来源同步入口明确不支持外部下载；`local_srs_files` 生产上下文为空，自动下载的本地 SRS 未被正常配置选择。相关分母与发现由桌面报告负责，未重复计入本 CSV。

## 执行记录和边界

最终执行命令：

```powershell
cargo run --offline --locked --manifest-path docs/evidence/tun-settings-audit-2026-10-05/engine-probe/Cargo.toml --target-dir target
python docs/evidence/tun-settings-audit-2026-10-05/build_engine_matrix.py
```

第一个命令 EXIT 0 表示 probe 对观察到的缺陷断言通过，**不表示设置功能验收通过**。最终日志为 `engine-probe/locked-replay.log`，结构化结果为 `engine-probe/observations.json`；scratch 为 `engine-probe/synthetic-storage-unavailable/`，全部合成且不含用户配置。

本轮未运行 workspace 全门禁、Flutter 原生窗口操作、内核真实会话、系统代理/TUN/UAC/IPv6/崩溃恢复；未执行远端 TLS 或发行更新。历史日志只用于定位已有覆盖的性质，不复用其完成宣称。

建议先修 AUD-ENG-01..05 并补最终输出/失败写入合同，再做真实 UI→FRB→存盘→重开→核心效果场景。TUN 字段和生命周期整改合并验收，成功条件必须同时包括正确配置、真实 adapter/lease、路由/DNS效果与关闭后的恢复，而不能仅凭期望开关或监听端口判成功。
