# FIX-16B 资源源与区域预设生效链（Geo/SRS/路由模板/SubConvert）

状态：`implemented`（SRS 源→生成链已在本机单 crate 测试验证；Geo/路由模板/SubConvert 的解析消费者已接线，实际网络下载消费者登记为缺口）。
任务 ID：FIX-16B（来源 `docs/tasks/FIX-16.md` 后续卡表、`docs/evidence/UX-PARITY-FIX-16/field-matrix.md` 中 4 个 `registered` 源字段）。

本次唯一用户流程：在「参数设置 → 显示 → 资源与证书」编辑 `SubConvertUrl` / `GeoSourceUrl` / `SrsSourceUrl` / `RouteRulesTemplateSourceUrl` → 保存 → 关闭 → 重开，字段从持久化文档恢复；且应用区域预设（Default/Russia/Iran）后，设置里的三个源写入上游 `Global` 规范 URL，生成 sing-box 配置时 `route.rule_set[].url` 实际使用 `SrsSourceUrl`（空则回退上游内置 `Global.SingboxRulesetUrl`）。

前置任务及已验证证据：FIX-16（字段补齐、5 页分组、草稿保存语义）；FIX-08（Apply 草稿语义，本卡不回退）。上游基准 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`：
- `ServiceLib/Global.cs:8/9/142/185/192/199/206`（`GeoUrl`、`SingboxRulesetUrl`、`SubConvertUrls`、`GeoFilesSources`、`SingboxRulesetSources`、`RoutingRulesSources`、`DNSTemplateSources`）
- `ServiceLib/Services/UpdateService.cs:369-531`（Geo/SRS 下载：空则回退内置）
- `ServiceLib/Handler/SubscriptionHandler.cs:129-152`（SubConvert：空则回退 `Global.SubConvertUrls.First`）
- `ServiceLib/Handler/ConfigHandler.cs:2515-2543`（路由模板：空则内置；外部下载失败回退内置）
- `ServiceLib/Handler/ConfigHandler.cs:2894-2957`（`ApplyRegionalPreset` 写 `Global.*Sources`）

对应 feature / field：`FLD-CFG-084` SubConvertUrl、`FLD-CFG-085..087` GeoSourceUrl/SrsSourceUrl/RouteRulesTemplateSourceUrl、SET-19。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：设置文档草稿（`ConstItem` 4 字段）；预设命令 `Default/Russia/Iran`。
- 输出：无新 IPC。`saveDocument` 落 `guiNConfig.json`；`apply_regional_preset` 写回设置树。
- 错误：非 Default 预设缺源表 → `error.preset_no_source`（明确报错，不静默跳过）；外部路由模板下载失败 → 由下载层返回错误（不静默回退伪造内容）。
- 取消：沿用 FIX-08 草稿语义。
- 权限：仅本机 UI + FRB/Rust；不监听端口、不写系统代理/TUN。
- 生效：SRS 源在生成 sing-box 配置时进入 `route.rule_set[].url`（`engine::build_codegen_input`）。

允许修改的模块：`crates/application/src/{dns.rs,engine.rs}`、`apps/desktop/lib/features/settings/option_setting_window.dart`、`apps/desktop/test/**`、`docs/evidence/UX-PARITY-FIX-16B/**`、`docs/evidence/UX-PARITY-FIX-16/field-matrix.md`、本卡。未改任何生成文件/台账分母。

禁止改变：FIX-08 草稿语义、FIX-16 分组与字段键、`settings-apply/save` 等键、编辑器与节点表、菜单结构。

测试夹具与原版预期：`SyntheticBridgePort`（内存设置文档）；上游预期：设置显式值优先，空/空白回退内置；预设写 `Global` 规范源。

本次必须通过的命令（实际结果见下）：
- Rust：`cargo fmt -p application -- --check`、`cargo clippy -p application --lib --locked -- -D warnings`、`cargo test -p application --lib --locked`
- Flutter：`dart format`、`flutter analyze`、`flutter test test/fix16b_settings_source_test.dart`

证据文件位置：`docs/evidence/UX-PARITY-FIX-16B/{README.md,observations.json}`；矩阵更新在 `docs/evidence/UX-PARITY-FIX-16/field-matrix.md`。

完成条件：4 个源字段保存→重开；预设写规范源；SRS 源实际进入生成；门禁通过。已达成（实际网络下载消费者除外，见缺口）。

接口缺口（登记，不自行削减）：
- Geo `.dat` 实际下载消费者在 update 模块（`crates/application/src/update_service.rs`/`features/update`，非本卡所有权）；本卡只提供 `dns::effective_geo_source` 解析器与回退语义。
- 订阅转换下载链在 `crates/subscriptions`/`crates/application/src/subs.rs`（非本卡所有权）；本卡只提供 `dns::effective_sub_convert_url`。
- 外部路由模板抓取+解析的消费者需在路由导入链接线（routing 模块）；本卡提供 `dns::effective_routing_template_source`（空=内置，Some=必须下载，失败即错）。
- `flutter_localizations` + ARB 全量本地化 → FIX-16G（本卡只保证现有 `locale_config` 不回归）。
- 并行冲突：本卡执行期间另一执行体在 `crates/application/src/dns.rs` 增补了区域 DNS 模板下载（`fetch_region_dns_plan`/`load_dns_template`/`RegionalDnsPlan`），并一度造成该文件测试模块括号失衡；本卡只做最小括号修复，未回退对方功能。

本轮实际结果：
- `dns.rs`：`region_sources` 改为逐字对齐上游 `Global.*Sources`（此前 Russia srs/routing、Iran srs/routing 与上游不符）；新增 `BUILTIN_GEO_URL`/`BUILTIN_SRS_URL`/`BUILTIN_SUB_CONVERT_URL` 与 `effective_geo_source`/`effective_srs_source`/`effective_sub_convert_url`/`effective_routing_template_source`；新增 2 个单测。
- `engine.rs`：`apply_regional_preset` 用 `region_sources`（去硬编码，缺源报错）；`build_codegen_input` 将 `SrsSourceUrl`（或内置）写入 `input.settings.ruleset_url`；新增 2 个单测。
- `option_setting_window.dart`：4 个源字段加 `ValueKey`（`settings-sub-convert-url` 等）。
- 新增 `test/fix16b_settings_source_test.dart`：保存 4 字段→断言持久化→重开恢复，通过。
