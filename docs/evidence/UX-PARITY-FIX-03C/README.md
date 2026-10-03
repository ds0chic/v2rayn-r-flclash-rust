# UX-PARITY-FIX-03C — ProxyChain 链节点专用编辑与链式生成语义

状态：`implemented`（ProxyChain 编辑器 widget 7/7；链式生成语义 Rust 合成链单测断言；未做原版实机双窗口逐事件对照，故不写 `verified`）。

本目录只放本卡证据，未进 `dist/`，未碰 `work/` 与 `outputs/`，未 commit。未改任何生产代码，仅新增测试。

## 1. 问题与范围

FIX-03 已让 `editSelectedProfile` 按 configType 分派（ProxyChain→group 编辑器），但既有测试只覆盖 PolicyGroup；链节点的编辑器行为与「生成配置时链式语义正确」缺少显式断言。FIX-03 任务卡 §46 将「ProxyChain 内核端到端校验」拆到本卡。本卡范围限定为**用户手工创建/编辑的 ProxyChain 节点**：编辑→顺序/五模式/保存→重开→生成链式语义；订阅级虚拟链（prev/next）另立后续卡。

## 2. 上游对照结论

- **编辑器分派**：`ProfilesViewModel.EditServerAsync:479/484` 对 `IsGroupType()`（=`PolicyGroup or ProxyChain`，`Extension.cs:89`）统一进 `AddGroupServerViewModel`。ProxyChain 与 PolicyGroup 共用同一编辑器，字段集合完全一致：备注、内核、五模式 `MultipleLoad`、保序 `ChildItems`（多选添加，仅排除 Custom，T/U/D/B）、`SubChildItems` + `Filter`。保存写回 `ChildItems/MultipleLoad/SubChildItems/Filter`，要求备注且（子节点或订阅子项至少其一）。
- **链式语义（关键）**：`V2rayOutboundService.BuildChainOutboundsList:656` 把 `ChildItems` **反序**，逐段用 `streamSettings.sockopt.dialerProxy` 串联（xhttp 另写 `downloadSettings.sockopt.dialerProxy`）；入口出站 tag 固定 `proxy`。sing-box 同语义用 `detour`。注释原文「Based on actual network flow instead of data packets」——即不是按存储顺序「依次代理」，而是按实际拨号流串联。对 `ChildItems=[c1,c2,c3]`：outbounds=[`proxy`=c3 → dials `chain-proxy-1-c2` → dials `chain-proxy-2-c1`]。
- **非负载均衡**：`GenOutbounds` 仅当 `tag.StartsWith("proxy")` 的出站数 >1 才生成 observatory/balancer；链中只有入口 tag 以 `proxy` 开头，故链不产生 balancer。
- **链子节点取用**：`BuildChainOutboundsList` 只读 `ChildItems`（不读 `SubChildItems`）；`SubChildItems` 用于 PolicyGroup 的订阅派生子项与预览。

## 3. 证据

| 文件 | 内容 |
|---|---|
| `observations.json` | 本卡断言清单与结果（Rust 2 项、widget 7 项） |
| `rust-codegen-run.log` | `cargo test -p application --lib codegen --locked` 原始输出 |
| `dart-chain-editor-run.log` | `flutter test test/fix03c_proxy_chain_editor_test.dart` 重试全过输出 |
| `README.md` | 本文件 |

合成夹具全部 loopback/保留地址（`192.0.2.x`），未连接、未监听端口、未动 10808、未改宿主代理/TUN/注册表/路由、未启动内核。

## 4. 命令与结果

```
cargo test -p application --lib codegen --locked        # 6 passed（含 2 项新增链式断言）
cargo fmt -p application -- --check                     # 0 changed
dart format --output=none --set-exit-if-changed test/fix03c_proxy_chain_editor_test.dart  # 0 changed
flutter test test/fix03c_proxy_chain_editor_test.dart   # 7/7（首跑 4 项 did-not-complete，重试通过）
dart analyze test/fix03c_proxy_chain_editor_test.dart   # No issues found
flutter analyze                                         # 3 issues，全部在并行代理改动文件，非本卡
```

## 5. 未完成/后续卡

- 订阅级虚拟 ProxyChain（`SubItem.PrevProfile/NextProfile` → `BuildSubscriptionChainNodeAsync`）未实现，建议立后续卡；需在 RuntimePlan/codegen 组装层新增按 Remarks 解析并合成头/尾链节点的能力。
- 链节点真实窗口端到端（右键→编辑→重开→内核启动）未单独跑；本卡以 widget + Rust 合成链断言为准。
- 未做原版实机双窗口逐事件对照；macOS/Linux/ARM64/Avalonia 未验证。
