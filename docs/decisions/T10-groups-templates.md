# T10 组/模板应用层决策记录

- 范围：PolicyGroup(101)/ProxyChain(102)/Custom(2)/Outbound(13)/FullConfigTemplate
  的应用层语义与持久化；生成器本体（T07/T08 冻结契约）不动。
- 上游基线：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967`
  commit `7d6a967`（只读）。

## D10-1 内置 Filter 的负向前瞻在 Rust 侧拆分为排除+包含两支正则

- 上游 `Global.PolicyGroupDefaultAllFilter` /
  `CombineWithDefaultAllFilter`（`ServiceLib/Global.cs`）形如
  `^(?!.*(?:<excl>)).*(?:<pat>).*$`，其中
  `excl = 剩余|过期|到期|重置|[Rr]emaining|[Ee]xpir|[Rr]eset`。
- Rust `regex` 无 look-around。`crates/application/src/groups.rs` 的
  `RemarksFilter` 对该精确外形做拆分求值（排除命中则拒绝，否则看包含支）；
  其余模式直接编译。拆分失败/非法模式仍报 `E_FIELD_FORMAT`
 （`error.group_filter_invalid`），与上游“非法正则不可存”一致。
- 地区表 `REGION_FILTERS`（17 项）逐字移植自
  `ConfigHandler.PolicyGroupRegionFilters`（`ConfigHandler.cs:1466-1485`）。

## D10-2 模板表保持“每核一行”，持久化进 guiNConfig.json

- 上游 `FullConfigTemplateItem` 是 SQLite 独立表（每核一行，8 字段）。
  本轮不做 schema 迁移：`AppEngine.templates` 常驻内存，
  随 `guiNConfig.json` 的 `full_config_templates` 键持久化，
  缺键时播种 `builtin-xray` / `builtin-singbox` 两行。
- 无 id 草稿按核 upsert（避免重复行）；`enabled=false` 或内容全空时
  `codegen::template_for` 返回 `None`，生成器行为与无模板一致。

## D10-3 Custom/Outbound 透传文本键与校验边界

- 内联透传文本键为 `proto_extra.extra["customConfigText"]`
 （`codegen::CUSTOM_CONFIG_KEY`），与 `build_input` 的读取路径同一键。
- `custom::validate_custom` 要求备注/地址非空、PreSocksPort 在
  1..=65535、透传文本为 JSON 对象或 YAML 映射；`normalize_custom`
  只做 trim，不吞掉越界端口（越界必须报错而非静默丢弃）。
- `save_profile` 统一打 `config_version = 4`（MIG-ENT-005），
  组草稿先 `normalize_group` 再全量 `validate_group`（含成环校验）。

## D10-4 真实校验矩阵的用例隔离

- 种子模板行默认 `enabled=false`；模板用例在单次 build 前后短暂启用，
  其余 14 个用例走裸节点图。原因：sing-box 的 selector/urltest 不接受
  `detour` 字段，全局启用模板会污染组用例（已复现，后隔离）。
- detour 指向合并后必定存在的 tag（xray `det1` freedom 出站；
  sing-box `corp-detour` 模板出站），保证 `run -test` / `check` 语义有效。
- mihomo 只做 `mixin.rs` 合并 + 文件读写往返，不执行内核
 （`custom_only_partial`，见 `compat/features.yaml` CRM-005）。

## D10-5 Flutter 合成桥接的近似与诚实边界

- `SyntheticBridgePort` 的组子项匹配用包含式近似（非完整正则），
  模板 JSON 校验接受 JSON 对象或 YAML 形文本；仅用于 widget 测试，
  真实语义以 Rust 侧为准。
