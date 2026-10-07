# SP-23.FLD-CFG-142 — SystemProxyItem.CustomSystemProxyScriptPath

状态：implemented（路径校验已登记；脚本真实执行 consumer 缺失，不写 verified）。
任务 ID：SP-23.FLD-CFG-142（主 owner SP-23；消费者归属 SP-32/SP-33；SD-18 平台代理，关联 SD-10）。

本次唯一用户流程（macOS/Linux 专属）：填自定义代理脚本路径→保存→重开→
冻结代理适配器按该路径执行脚本（超时/exit/取消按冻结语义）；
Windows 原版不用此字段（零副作用）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-05; 冻结 ProxySettingLinux/OSX；跨平台 release`。

对应 ID：FLD-CFG-142；leaf；platform_scope=macos,linux；original_type=string；
original_default=null。
关联：`CustomSystemProxyPacPath`（同 candidate 键表），SD-18/SD-10。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SystemProxyItem.CustomSystemProxyScriptPath`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:230`
（`string? CustomSystemProxyScriptPath`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法路径/null（未设置）；
缺失文件→`NotFound` 结构化错误（`platform/tests/models_and_script.rs:85`）。
持久化 save；生效 immediate（冻结台账；`domain/src/settings_timing.rs:254`）。
secret 不入日志。平台写入与执行只授权隔离机；10808 禁占，
测试端口 ≥11808 预探测；不得改宿主系统代理。

允许修改的模块（SP-32/SP-33）：`crates/platform/src/script.rs`
（`:5` 字段契约、`:18` `CUSTOM_SCRIPT_FIELD`、`:26-51` 路径校验）、
`crates/persistence/src/candidate.rs`（`:473` 双键表）、
`crates/persistence/src/upstream_db.rs`（`:180` 键表）。
本卡未改生产代码。

禁止改变的已有行为：Windows 原版不用此字段；null=未设置；未知键保留；
其它 SystemProxyItem 不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储/校验：`platform/src/script.rs` `CustomProxySpec::new`
  （仅 `validate_existing_file` 路径校验，无执行）。
- 单测：`platform/tests/models_and_script.rs:60`（字段名）、`:85`（缺失路径）。
- 调用链缺口：OSX/Linux 自定义脚本执行 consumer 缺席
  （CSV current_gap：`execution consumer absent`）；当前只有路径有效性校验，
  不能称脚本实际生效。
- 缺口：合成脚本→cwd/成功/失败/超时/取消→平台实际效果端到端验收未跑；
  Windows 零副作用待验。

测试夹具和原版预期：合成脚本路径；正向 存在可执行→平台按冻结语义执行；
负向 缺失/不可执行/超时→结构化错误且旧配置不变；取消→无残留。

本次必须通过的命令/真实场景（SP-32/SP-33，未运行）：`cargo test -p platform --locked`
（models_and_script）+ 授权隔离 macOS/Linux 真实执行→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-142.md`。

完成条件：保存、真实脚本执行效果和重开三者一致；仅路径校验不算。

发现接口缺口时的处理：脚本执行 consumer 缺失已登记为接口缺口；归属 SP-32/SP-33。
