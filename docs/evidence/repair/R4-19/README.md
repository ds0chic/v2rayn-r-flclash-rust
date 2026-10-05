# R4-19 自定义模板Mixin — 证据

状态：`implemented`（生成链与模板窗口合同已修；真实内核启动/重开未验证）。
armed=false；未启动任何内核；未监听任何端口；未触碰 10808、宿主代理/路由/TUN/自启。
夹具全部为合成数据（192.0.2.0/24、合成 UUID/密钥），未读写用户凭据。

## 基线

- 仓库 HEAD：`a296892`
- 上游冻结：`7d6a967c18c697f28dc6917122ed3a4993fcf336`
- 应用基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`
- 上游对照符号：
  - `work/.../V2rayConfigTemplateService.cs:86-223`（Xray `ApplyFullConfigTemplate`）
  - `work/.../SingboxConfigTemplateService.cs:86-144`（sing-box `ApplyFullConfigTemplate`）
  - `work/.../CoreConfigClashService.cs:113-268`（mihomo Mixin，本卡不启用）

## 缺陷与修复

### 1. sing-box 模板合并顺序与上游相反（主缺陷）

上游 `SingboxConfigTemplateService.ApplyFullConfigTemplate:106-125`：以**模板自身的
`outbounds` 数组为基**，把生成出站**追加在后**；endpoints 同理（`:127-141`）。
Xray 服务（`V2rayConfigTemplateService.cs:177-220`）则相反：生成出站在前、模板在后。
两者语义不同。现实现把 sing-box 也做成“生成在前、模板在后”
（依据 `docs/decisions/T07-T08-codegen-impl-notes.md:45-46` 的误读，以及
`src/singbox/config.rs` 旧注释 “Frozen T08 contract §5”）。

修复：`crates/config_codegen/src/singbox/config.rs::full_config_template`
- outbounds：先放模板 `outbounds`，再 `AddProxyOnly` 过滤 `direct/block` 后追加生成出站；
- endpoints：先放模板 `endpoints`，再追加生成 endpoints（仅当生成集非空时）。
两处改动均在 `template_map`（模板节点）上完成，未知/额外顶层键原样保留。

### 2. 模板窗口保存抛异常会卡死（可读错误/可重试）

`apps/desktop/lib/features/profiles/template_window.dart::_save` 之前直接
`widget.onSave(item)`；原生桥抛异常时异常逃逸，`_submitting` 一直为 true，
保存/取消按钮永久禁用，用户无法重试。修复：`try/catch` 捕获后复位
`_submitting`，并在 `template-error` 显示可读消息，窗口保持可重试。

## 上游对照结论

- Xray：模板节点为最终文档；生成出站在前、模板出站在后；balancer 规则重写
  `outboundTag(proxy)→balancerTag` 并把生成 balancer 追加到模板 `routing.balancers`；
  observatory/burstObservatory 的 `subjectSelector` 取并集去重；ProxyDetour 仅对
  非私网且无既有 dialerProxy 的出站写入。现实现与该语义一致（新增测试 `xray_template_outbounds_follow_generated`）。
- sing-box：模板出站/endpoints 在前、生成在后；`AddProxyOnly` 跳过 `direct/block`；
  ProxyDetour 仅对无 `detour` 且 server 非私网的出站写入。修复后与上游一致。
- mihomo Mixin（`CoreConfigClashService.MixinContent`）：`crates/application/src/mixin.rs`
  已按上游实现普通覆盖 + `prepend-/append-/removed-` 列表合并，本卡不改；
  该路径未接入运行生成消费者（属另一接口缺口，非本卡允许范围），故“不强制转 Mihomo YAML”
  按上游语义保持：Xray/sing-box 的 Custom JSON 原样透传，不转 YAML。

## 复现（先失败后通过）

- Dart：`apps/desktop/test/repair/r4_19_repro_test.dart`
  - 未修复的 `template_window.dart`：FAILED（抛异常未被捕获，error 未显示）。
  - 修复后：PASS。
- Rust：`crates/config_codegen/tests/r4_19_template_merge.rs`
  - 暂存 `crates/config_codegen/src/singbox/config.rs` 恢复旧实现：
    `singbox_template_outbounds_precede_generated` 与
    `singbox_template_endpoints_precede_generated` FAILED。
  - 恢复修复：6/6 PASS。
  - 同步修正旧用例 `tests/singbox_template_custom.rs::singbox_template_injection`
    的期望为上游顺序（模板在前）。

## 实际命令与结果

见 `commands.log`。摘要：

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked` | exit 0（全部 test result ok） |
| `flutter analyze` | 2 条 warning，均在 `test/r4_18_p01_contract_test.dart` / `r4_18_p02_contract_test.dart`（R4-18，非本卡文件）；本卡文件 0 issue |
| `flutter test test/r4_19_contract_test.dart test/repair/r4_19_repro_test.dart --concurrency=1` | 7/7 通过 |
| `flutter build windows --release` | exit 0，产出 `build/windows/x64/runner/Release/v2rayn_desktop.exe` |

## 未完成 / 未验证

- 真实内核“启动→重开”后读取运行配置、真实 PreSocks/统计端点的运行效果：未运行（未武装内核，
  无授权隔离平台）。生成层已由 `t10_core_matrix` 与新增测试覆盖。
- mihomo `enable_mixin_content` 的运行时消费（把 `Mixin.yaml` 合并进 mihomo 生成配置）仍未接线，
  登记为接口缺口：提供方 `application::mixin::generate_mihomo`，调用方应为本项目的原生 Custom
  生成路径（`engine.rs` 属另一代理所有权）；参数 `MixinOptions`、错误 `DomainError`、
  生效时机为 Custom 配置写出前。
- `flutter analyze` 的 2 条无关 warning 属 R4-18 文件，未处理。
