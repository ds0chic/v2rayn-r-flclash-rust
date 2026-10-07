# SP-23.FLD-CFG-088 — GlobalHotkeys[].EGlobalHotkey

状态：implemented（实例登记完成；真实 OS 注册/多动作验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-088（主 owner SP-23；消费者归属 SP-32/SP-33）。

本次唯一用户流程：全局热键窗改动作类型（KeyEventItem.EGlobalHotkey，
enum，默认 null）→保存→下次启动→native adapter 按 Alt/Control/Shift/KeyCode
组合注册对应动作，冲突/失败走注册拒绝保留。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; 真实 native hotkey adapter; 原版正式注册入口时机核定`。

对应 ID：FLD-CFG-088；leaf；platform_scope=all；original_type=enum；original_default=null。
关联：FLD-CFG-089/090/091/092（同条目组合键，本批已登记），SD-18/SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: KeyEventItem.EGlobalHotkey`；
只读核对原版动作枚举语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法动作值 0..4 / null；
冲突/失败→注册拒绝保留旧绑定。持久化 save；生效 next_launch
（`settings_timing.rs:112`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-32/SP-33）：`crates/bridge_api/src/api/platform.rs`
（`:141-152` HotkeyDto、`:797-814` hotkey_proxy_mode/hotkey_list）、
`crates/bridge_api/src/api/settings.rs`（`:925-960` GlobalHotkeyDto）。
本卡未改生产代码。

禁止改变的已有行为：原版动作枚举值域；未知键保留；
stale revision 拒绝；保存/运行分离；其它热键条目不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` GlobalHotkey（`entities.rs:304-308`，`EGlobalHotkey` rename 在位）。
- DTO：`GlobalHotkeyDto.action`（`settings.rs:927-934`，EGlobalHotkey 数值通道）+
  FRB wire `hotkey_list`（`frb_generated.rs:1666-1690/10503` 编解码在位）。
- 调用链：`hotkey_list` 过滤→`hotkey_proxy_mode`（`platform.rs:797-814`）→
  native adapter 注册（OS 侧真实效果未验）。
- 单测：`platform.rs:981-986`（proxy_mode 映射/过滤）、
  `settings.rs:1630-1638`（hotkey 改动判 next_launch）。
- 缺口：台账 next_launch 与冻结正式入口即时注册路径的时机待核定
  （CSV research_status）；同组合多动作真实按键验收未跑。

测试夹具和原版预期：合成动作值；正向 action=1→proxy_mode 映射；
负向 9/越界→None 拒绝（单测已覆）；真实注册/冲突/retry 待隔离机。

本次必须通过的命令/真实场景（SP-32/SP-33，未运行）：`cargo test -p bridge_api --locked`
（hotkey helpers）+ 正式窗→FRB→保存→重开→隔离机真实按键→冲突/retry。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-088.md`。

完成条件：保存、重开和真实 OS 注册三者一致；仅 DTO/单测不算。

发现接口缺口时的处理：时机核定缺口已登记；归属 SP-32/SP-33。
