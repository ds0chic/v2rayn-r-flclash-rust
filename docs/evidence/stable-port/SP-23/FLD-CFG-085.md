# SP-23.FLD-CFG-085 — ConstItem.GeoSourceUrl

状态：implemented（实例登记完成；URL 落盘≠正式 plan 实际消费下载内容，未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-085（主 owner SP-23；消费者归属 SP-25）。

本次唯一用户流程：设置窗改 Geo 源 URL（ConstItem.GeoSourceUrl，`{0}`=geoip/geosite，
string，默认 null）→保存→本地合成资源源下载→verified 文件→
正式 codegen 上下文实际消费下载内容，SRS 不暗访 remote。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; SD-10 client; routing/resource source schema; 正式 codegen context`。

对应 ID：FLD-CFG-085；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-086/087（同资源源组，已有证据），SD-15。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ConstItem.GeoSourceUrl`；
只读核对原版 `{0}` 模板语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 HTTPS 模板 URL / null；
坏内容/中断取消/离线→保留旧资源。正式入口
`option_setting_window.dart:1033-1035`
（`_str/_set('ConstItem','GeoSourceUrl')`）。持久化 save。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测；不装 OS 证书。

允许修改的模块（SP-25）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:1033-1035`）、`apps/desktop/lib/features/settings/resource_auto_update.dart`
（`:23/:39` 资源键）。
本卡未改生产代码。

禁止改变的已有行为：原版 `{0}` 占位语义；未知键保留；stale revision 拒绝；
保存/运行分离；其它 ConstItem 不动。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:130 'GeoSourceUrl': null`。
- DTO：FRB wire `geo_source_url`（`frb_generated.rs:6745/11278/14857` 编解码在位）。
- 调用链：正式窗 `_set`→保存→`resource_auto_update` 资源键→下载 verified 文件。
- 单测：`r4_13_s04_contract_test.dart:19-44`（置值/空白往返）、
  `fix16b_settings_source_test.dart:79/98`（源恢复）。
- 缺口：只落下载文件/保存 URL 不算效果，必须在正式 plan 实际接入
  （CSV current_gap，同 086/087 已有证据口径）。

测试夹具和原版预期：合成模板 URL；正向 `https://mirror.example/{0}.dat`→
下载→plan 消费；负向坏内容/中断/离线→保留旧资源（待验）。

本次必须通过的命令/真实场景（SP-25，未运行）：`flutter test`
（`r4_13_s04_contract`、`fix16b_settings_source`）；补正式窗→FRB→保存→
本地合成源下载→正式 plan 消费→SRS 不暗访 remote→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-085.md`。

完成条件：保存、下载落地和正式 plan 实际消费三者一致；仅落盘不算。

发现接口缺口时的处理：plan 实际接入缺口已登记（同组 086/087 口径）；归属 SP-25。
