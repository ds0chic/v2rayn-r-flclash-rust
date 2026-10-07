# SP-23.FLD-CFG-092 — GlobalHotkeys[].KeyCode

状态：implemented（实例登记完成；真实 OS 同组合注册验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-092（主 owner SP-23；消费者归属 SP-15 方向）。

本次唯一用户流程：全局热键窗改某条目主键（KeyEventItem.KeyCode，
int?，默认 null）→保存→下次启动→native adapter 按新组合键注册，
null/冲突/失败走注册拒绝保留。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; 真实 native hotkey adapter; 原版正式注册入口时机核定`。

对应 ID：FLD-CFG-092；leaf；platform_scope=all；original_type=int?；original_default=null。
关联：FLD-CFG-088/089/090/091（同条目组合键，本批已登记），SD-18/SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: KeyEventItem.KeyCode`；
只读核对原版主键语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法键码 / null（缺省）；
坏值/冲突→注册拒绝保留。持久化 save；生效 next_launch
（`settings_timing.rs:113`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-15 方向）：`crates/bridge_api/src/api/settings.rs`
（`:932` DTO key_code Option、`:936-960` 往返）、
`crates/domain/src/entities.rs`（`:304-308` 存储）。
本卡未改生产代码。

禁止改变的已有行为：原版 nullable 缺省语义；未知键保留；
stale revision 拒绝；保存/运行分离；同条目其它修饰键不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` GlobalHotkey.key_code（`settings.rs:965` 列表段）。
- DTO：`GlobalHotkeyDto.key_code: Option<i32>`
  （`settings.rs:932/943/956` 往返在位）+ FRB wire `hotkey_list`。
- 调用链：persist→`hotkey_list` 过滤→native adapter（OS 真实效果未验）。
- 单测：`settings.rs:1638`（key_code=65 组合判 next_launch，直接覆盖本字段）。
- 缺口：真实按键组合注册/冲突验收未跑（CSV current_gap）。

测试夹具和原版预期：合成键码 65→重开组合生效；负向 null/冲突→
拒绝保留（待隔离机）。

本次必须通过的命令/真实场景（SP-15 方向，未运行）：`cargo test -p domain --locked`
（hotkey timing）+ 正式窗→FRB→保存→重开→隔离机真实按键。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-092.md`。

完成条件：保存、重开和真实 OS 注册三者一致；仅 DTO/单测不算。

发现接口缺口时的处理：真实注册验收缺口已登记；归属 SP-15 方向。
