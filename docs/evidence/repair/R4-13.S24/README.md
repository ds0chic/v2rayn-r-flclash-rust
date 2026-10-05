# R4-13.S24 evidence (WebDavItem)

- 状态：implemented（设置文档存储 + T16 WebDAV 字段消费）；真实 TLS blocked。
- 字段：FLD-CFG-143..146（Url、UserName、Password、DirName）。
- 存储/消费合同：`test/r4_13_s24_contract_test.dart`（3 例，pass：设置文档保存→重开；缺省；T16 `t16WebdavConfigSave/Get` 经 backup controller 保存→重开）。
- 复现护栏：`test/repair/r4_13_s24_repro_test.dart`（字段重开 + stale revision 拒绝，pass）。
- R4-28 衔接：控制器异步/取消/busy 修复由 R4-28 落地（`test/r4_28_*`），本实例只补字段保存→重开与消费断言。
- 未发现本卡范围内可在允许文件内修复的存储/消费缺陷。
- blocked：真实 TLS/WebDAV 端点与真实 Windows 重开端到端未实测（无合成远端；凭据不写入证据/日志）。
