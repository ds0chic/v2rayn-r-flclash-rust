# SP-23.FLD-CFG-064 — GuiItem.RootCertProvider

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-064（主 owner SP-23；消费者归属 SP-25）。

本次唯一用户流程：设置窗切应用证书来源 system→mozilla→一次保存（immediate）→
受控合成 CA 签名的订阅 HTTPS 源按新来源建信：mozilla 源接受、system 源拒绝；
全程不装 OS 证书、不改系统证书库、不影响内核 TLS。

前置任务及已验证证据：SP-00（含共享 HttpPolicy/ClientFactory 核定）；
SP-01/02、SP-12（未 verified）。
CSV dependencies：`锁定TLS backend/root bundle来源版本hash; 各网络模块注入policy`。
research：需核定 TLS backend 与 bundle 实现；无 OS 证书安装需求。

对应 ID：FLD-CFG-064；leaf；platform_scope=all；original_type=string；
original_default="system"。关联 SD-10 / SD-07/15/16/17。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.RootCertProvider`；
`CertPemManager.cs:313-357`（信任初始化）、`DownloadService`（消费侧）；
`Global.RootCertProviders` 顺序（system 首位回退）；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。
纠正旧误：R4-13.S10 曾写“证书安装/OS blocked”有误；本来源只控制应用 HTTPS
自定义根信任（审计 CP-SET-07 原话），缺消费者不得归因于用户未授权。

输入、输出、错误、取消、权限、持久化及生效语义：
合法 system/chrome/mozilla（未知回退 system，`platform/src/cert/mod.rs:27-88`，
`bridge_api/src/api/platform.rs:661-668 cert_provider_info`）。
UI 草稿 `option_setting_window.dart:1024,1030`、`settings_controller.dart:193,482-487`、
`settings_defaults.dart:6,10`。SP-02 提交语义；持久化 save；生效 immediate（冻结）。
取消在途 HTTPS 请求按各 client 取消语义；错误（过期/hostname/不可信）结构化可见。
日志脱敏。

允许修改的模块（SP-25）：`crates/subscriptions/src/download.rs`（`:119` 自建 Client）、
`crates/updater/src/{fetch.rs:36,download.rs:104}`、`crates/application/src/webdav.rs:110`、
拟新增低层 HTTP 共享模块（路径先经 SP-00 核定）+ `Cargo.toml/lock`。
路由/Geo 资源 client 同批。FRB 由整合者生成。本卡未改生产代码。

禁止改变的已有行为：OS 证书库零写入；内核 TLS 不动；未知 provider 回退 system；
各 client 现有代理/超时/重试语义。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:420`（`root_cert_provider`，默认 system `:436`；
  规范化 `:1054-1057`；`:1345` 单测）。
- DTO：`bridge_api/src/api/settings.rs:271,286,303`。
- 映射层（存在）：`platform/src/cert/mod.rs:40 RootCertProvider`,
  `:84-88 trust_source`（System→SystemStore，Chrome/Mozilla→Bundled），
  `:92 uses_system_store`；`bridge_api/src/api/platform.rs:162-168 CertProviderDto`,
  `:646-668`。
- 消费者缺口 G-05：`subscriptions/src/download.rs:119`
  `reqwest::Client::builder()` 仅设 `accept_invalid_certs`/proxy，未读 provider；
  审计定位 updater/webdav 同类自建（本卡复核 download.rs，其余沿审计）。

测试夹具和原版预期：受控TLS（合成 CA/叶证书，hostname/过期/不可信矩阵）；
正向 mozilla 接受合成 CA、system 拒绝自签；负向 错 CA/过期/hostname 不匹配；
取消与重开；订阅/资源/更新/WebDAV/路由/Geo 逐 client（SP-25 逐项证）。

本次必须通过的命令/真实场景（SP-25，未运行）：`cargo test -p subscriptions -p updater --locked`
（受控 TLS 矩阵）；正式窗→FRB→保存→各真实 client 建信观察→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-064.md`。

完成条件：逐 client 信任来源一致 + 矩阵 + 重开；只映射 enum 不算。

发现接口缺口时的处理：G-05 已登记；共享 HTTP 模块由 SP-00 整合者排期，不私改他人文件。
