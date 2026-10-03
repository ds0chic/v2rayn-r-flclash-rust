# UX-PARITY-FIX-16B evidence

FIX-16B：Geo/SRS/路由模板/SubConvert 资源源与区域预设生效链。

## 交付摘要

- 上游规范源：`crates/application/src/dns.rs` 的 `region_sources` 逐字对齐上游
  `Global.GeoFilesSources/SingboxRulesetSources/RoutingRulesSources/DNSTemplateSources`
  （修正此前 Russia/Iran 的 srs、routing URL 与上游不一致）。
- 解析回退语义（对齐上游「空则回落内置」）：
  - `effective_geo_source` → `Global.GeoUrl`
  - `effective_srs_source` → `Global.SingboxRulesetUrl`
  - `effective_sub_convert_url` → `Global.SubConvertUrls[0]`
  - `effective_routing_template_source` → `None`=内置路由；`Some(url)`=必须下载，失败即错。
- 实际消费链（本卡内已接线并验证）：
  `SrsSourceUrl` → `engine::build_codegen_input` → sing-box `route.rule_set[].url`。
- 预设联动：`engine::apply_regional_preset` 经 `region_sources` 写入设置树；非 Default 缺源
  返回 `error.preset_no_source`。消费者读设置树，不再硬编码。
- Flutter：4 个源字段加 `ValueKey`，新增保存→重开 widget 测试。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application` | 0 错误（格式已写回） |
| `cargo clippy -p application --lib --locked -- -D warnings` | 0 警告 |
| `cargo test -p application --lib --locked` | **170 passed; 0 failed** |
| `cargo test -p application --lib --locked srs_source` | 1 passed（`build_codegen_input_uses_settings_srs_source`） |
| `dart format lib/.../option_setting_window.dart test/fix16b_settings_source_test.dart` | 0 错误 |
| `flutter analyze` | No issues found! |
| `flutter test test/fix16b_settings_source_test.dart` | All tests passed! |

## 状态统计（field-matrix 更新）

- `SrsSourceUrl`：registered → **verified**（生成输入断言实际使用）。
- `SubConvertUrl` / `GeoSourceUrl` / `RouteRulesTemplateSourceUrl`：registered → `consumed`
  （解析器已接线；实际网络下载消费者非本卡所有权，登记缺口）。
- 矩阵统计：verified 3→4；consumed 68→71；registered 7→3（仅 Clash Proxies*）。

## 缺口

- Geo `.dat` 下载、订阅转换下载、外部路由模板抓取的真实网络消费者分别在 update /
  subscriptions / routing 导入链，非本卡所有权 → 登记，不伪造。
- 完整本地化 → FIX-16G。
- 并行冲突：执行期间另一执行体在 `crates/application/src/dns.rs` 增补区域 DNS 模板下载
  （`fetch_region_dns_plan` 等），曾使该文件测试模块括号失衡；本卡仅做最小括号修复，
  未回退其功能，fmt/clippy/test 均已通过。
