# SP-24 SRS 生成器侧（086/087 类，G-07 登记）

归属：086/087 主 owner SP-25；本卡只做生成器侧可消费证明 + 生产缺口登记，
不碰 update/network/engine（锁外）。

实测（`cargo test -p application --locked --lib sp24` 绿，合成路由 `geosite:google`）：

- `SrsSourceUrl` 投影：`settings_from_app` 按
  `crate::dns::effective_srs_source` 填充 `CodegenSettings.ruleset_url`
  （显式模板原样；空回上游内置 `BUILTIN_SRS_URL`，与
  `config_codegen::util::SINGBOX_RULESET_URL` 同串；engine `:4571` 事后赋同值，
  行为不变）。单测 `sp24_srs_source_reaches_codegen_settings`。
- `local_srs_files` 快照：`CodegenOptions.local_srs_files: BTreeSet<String>`
 （调用方快照，纯输入无 IO）→`settings_from_app` 透传→sing-box 生成器对命中名
  输出 `{"type":"local","format":"binary","tag":..,"path":"bin/srss/<name>.srs"}`
  且不挂 `srs-download-http-client`。单测
  `sp24_local_srs_snapshot_selects_local_ruleset`。
- 缺口 G-07（生产填充者，SP-00 核定归属）：扫描 `bin/srss/*.srs` 并填
  `CodegenOptions.local_srs_files` 的生产代码不存在——全仓库仅定义 + 消费者 +
  测试快照。下载了 srss ≠ 生成器选本地。候选归属 update/engine，本卡未实现。
正式源设置→FRB→保存→下载→快照→断网生成 diff→重开：未验证（SP-25）。
