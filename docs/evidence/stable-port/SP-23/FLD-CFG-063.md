# SP-23.FLD-CFG-063 — GuiItem.EnableLog

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-063（主 owner SP-23；消费者归属 SP-25）。

本次唯一用户流程：关闭应用日志开关→保存→restart_app→应用诊断日志停止写入
（含轮转归属清理）；再打开→恢复写入且脱敏（无秘密）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; logger初始化与归属清理`。关联 SD-07。

对应 ID：FLD-CFG-063；leaf；platform_scope=all；original_type=bool；original_default=true。
关联：SD-12。注意：不是内核 LogEnabled（FLD-CFG-026），不得混同。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.EnableLog`；
`AppManager.cs:101`（应用日志开关原版消费）；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
正式 UI 入口缺失（审计：`internal/preserved`，仅 `settings_defaults.dart:105`），
SP-25 须先核对正式入口（不得自造控件，若原版无控件则按 preserved 语义登记）。
SP-02 提交语义。持久化 save；生效 restart_app（`settings_timing.rs:123`）。
恢复 journal 在关日志时仍必须可用（CSV verification 原话）。

允许修改的模块（SP-25）：Rust/host/UI 应用诊断 logger 初始化位（待 SP-25 定位，
先登记为缺口，不预设文件）、settings 正式入口。FRB/DTO 由整合者生成。

禁止改变的已有行为：内核 LogEnabled 行为不动；恢复 journal 可用性不受开关影响；
日志脱敏（无节点凭据/订阅地址）。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:418`（`enable_log`，默认 true `:435`；`:1288,1428` 测试读写）。
- DTO：`bridge_api/src/api/settings.rs:270,285,302`。timing RestartApp（`:123`）。
- 消费者缺口 G-04：`enable_log|EnableLog` 在 crates 内仅 DTO/存储/timing/edge 用例
  （`persistence/tests/edge_cases.rs:97` 为存储用例）；无 logger 初始化/轮转/禁写读者。

测试夹具和原版预期：合成运行；正向 关（无新应用日志写入）/开（写入+轮转）；
负向 存储失败时开关状态与错误可见性（CP-SET-03 类）；脱敏断言。

本次必须通过的命令/真实场景（SP-25，未运行）：`cargo test -p <logger所在包> --locked`；
正式入口→FRB→保存→restart_app→真实日志启停/轮转/脱敏观察→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-063.md`。

完成条件：开关真实启停应用日志 + 轮转/脱敏 + journal 可用 + 重开；只存 bool 不算。

发现接口缺口时的处理：G-04 已登记；正式入口缺失疑问一并移交 SP-25（先核对原版入口）。
