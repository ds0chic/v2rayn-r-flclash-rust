# SP-23.FLD-CFG-143 — WebDavItem.Url

状态：implemented（实例登记完成；合成本地 WebDAV 真实协议验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-143（主 owner SP-23；消费者归属 SD-17 t16 备份链路）。

本次唯一用户流程：备份设置改 WebDAV 地址（WebDavItem.Url，
string，默认 null）→保存→t16 备份链路用该地址做真实 PROPFIND/PUT/GET，
合成本地 WebDAV 验证编码/目录/auth/失败/重试/中断。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/03; SD-10; actual backup lifecycle service`。

对应 ID：FLD-CFG-143；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-144/145/146（同 WebDav 组，本批已登记），SD-17/SD-04/16。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: WebDavItem.Url`；
只读核对原版地址语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 URL / null；
坏地址/auth 失败/中断→保留旧备份并显式报错。持久化 save
（`settings_timing.rs:313`）。secret 不入日志（合成地址 only）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SD-17 t16 链路）：`crates/bridge_api/src/api/settings.rs`
（`:767` DTO url）、`crates/bridge_api/src/api/contract.rs`
（`:553-556` WebDavConfigDto）、t16 FRB wire
（`frb_generated.rs:5650-5852` backup/check/list/restore/config）。
本卡未改生产代码。

禁止改变的已有行为：原版 nullable 缺省语义；未知键保留；
stale revision 拒绝；保存/运行分离；144/145/146 同组字段不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储/DTO：`WebDavItemDto.url: Option<String>`（`api/settings.rs:767`）+
  settings wire `web_dav_item`（`frb_generated.rs:9330/9364/13369/16638`）。
- 调用链：t16 `t16_webdav_backup/check/list/restore/config_get/config_save`
 （`frb_generated.rs:10439-10442/10614-10615` 分发在位）+
  Dart `bridge_port.dart:346-359` 真实端口与 fake（`:3270-3419`）。
- 单测：fake 仅故障注入（`webdavCheckOk` 开关）；真实协议验收未跑。
- 缺口：mock HTTP/ZIP 解析不证明真实 WebDAV+DB+UI 重开
  （CSV current_gap）；ZIP/native/原版 bundle canonical IDs/未知键往返待验。

测试夹具和原版预期：合成 `https://dav.example/backup`；正向连通→备份往返；
负向坏地址/auth 失败/中断→保留旧备份（待验）。

本次必须通过的命令/真实场景（SD-17，未运行）：合成本地 WebDAV 真实协议
（编码/目录/auth/失败/重试/中断）→ZIP/native/原版 bundle 往返→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-143.md`。

完成条件：保存、真实协议备份往返和重开三者一致；仅 DTO/fake 不算。

发现接口缺口时的处理：真实协议验收缺口已登记；归属 SD-17 t16 链路。
