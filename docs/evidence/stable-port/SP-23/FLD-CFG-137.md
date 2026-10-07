# SP-23.FLD-CFG-137 — SystemProxyItem.SysProxyType

状态：implemented（实例登记完成；隔离机真实 proxy/PAC 查询与恢复验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-137（主 owner SP-23；消费者归属 SP-15 方向）。

本次唯一用户流程：系统代理设置改模式（SystemProxyItem.SysProxyType，
enum，默认 "ForcedClear"）→即时生效→platform service 按 actual 模式/
endpoint/ownership 下发，隔离机查询真实 proxy/PAC 值与恢复归属。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; actual Endpoint/PlatformReceipt/ownership ledger; 隔离 OS 场景`。

对应 ID：FLD-CFG-137；leaf；platform_scope=all；original_type=enum；original_default="ForcedClear"。
关联：FLD-CFG-138/139/140/141（同系统代理组，已有证据），SD-05/SD-07/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SystemProxyItem.SysProxyType`；
只读核对原版四模式语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：ForcedClear/ForcedChange/
Unchanged/Pac（0..3）；未知值→默认回退（`from_value…unwrap_or_default`）。
同 mode/session/port 改内容仍 apply/retry。生效 immediate
（`settings_timing.rs:260`）。平台写入仅授权隔离机；宿主代理禁动；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-15 方向）：`crates/domain/src/enums.rs`
（`:388-431` SysProxyType）、`crates/application/src/platform_service.rs`
（`:616-645` mode_from_domain）、`crates/platform/src/sysproxy/mod.rs`
（`:4-21` 四模式语义）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 ForcedClear；未知键保留；
stale revision 拒绝；保存/运行分离；138-141 去重/内容键语义不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` system_proxy_item.sys_proxy_type（`settings.rs:697/715`，
  默认 ForcedClear；`settings.rs:1410` 序列化断言 SysProxyType=0）。
- DTO：`sys_proxy_type: i32`（`api/settings.rs:726-753` 往返在位）+
  FRB wire（`frb_generated.rs:9881/13950/16945`）+
  `platform.rs:820-846` mode 转换/校验 helpers。
- 调用链：domain→`mode_from_domain`（`platform_service.rs:640-645`）→
  platform 下发；`runtime_plan.rs:120` system_proxy 上下文。
- 单测：`application/tests/t13_platform.rs:342`（ForcedChange 映射）、
  `platform.rs:981-985`（hotkey 动作→proxy mode）、
  `persistence/tests/edge_cases.rs:106`（SysProxyType=1 往返）。
- 缺口：去重键仅 mode/session/port，遗漏配置内容与 PAC 实际内容 hash
  （CSV current_gap）；隔离机真实 proxy/PAC 查询与恢复未跑。

测试夹具和原版预期：合成四模式；正向 ForcedChange→映射下发；
负向未知值→默认回退、内容变化→重下发（待验）。

本次必须通过的命令/真实场景（SP-15 方向，未运行）：`cargo test -p application --locked`
（t13 platform）+ 正式入口→真实 Rust 提交→同 mode/session/port 改内容
仍 apply/retry→隔离机查询真实 proxy/PAC 值与恢复归属。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-137.md`。

完成条件：保存、下发事实和隔离机查询三者一致；仅映射单测不算。

发现接口缺口时的处理：去重键缺口已登记；归属 SP-15 方向。
