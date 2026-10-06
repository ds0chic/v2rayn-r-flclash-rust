# SP-24 G-06 engine 接线：Mihomo 原生计划落盘前 merge

基线 `393fafd`。上游冻结 v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
A09 登记插入点 `engine.rs` native_custom 分支，签名
`mihomo_body_for_plan(&raw, mixin_text.as_deref(), tun_text.as_deref(), &settings, opts)?`。
不 commit。

## 改动文件（本卡锁内）

- `crates/application/src/engine.rs`（唯一源码改动）：
  - 新增 `MIHOMO_MIXIN_FILE_NAME = "Mixin.yaml"`（上游 `Global.ClashMixinConfigFileName`
    原名，位于 `<data>/config/`；任务书“如 config/mixin.yaml”为示例，大小写以 work 源码为准，
    Windows 文件系统下等价）。
  - 新增 `MIHOMO_TUN_YAML`（逐字节对应上游 `ServiceLib/Sample/clash_tun_yaml`）。
  - 新增 `read_mihomo_mixin_text` / `mihomo_mixin_text` / `mihomo_tun_text`：
    mixin 受 `ClashUIItem.EnableMixinContent` 门控（上游 `MixinContent` 提前返回），
    缺文件/不可读/空白一律 `None`（上游记日志后继续无 merge）；TUN 受
    `TunModeItem.EnableTun` 门控（上游 `CoreConfigContextBuilder` 快照 `isTunEnabled`）。
    有意偏离：plan 构建保持无副作用，缺文件时不从嵌入默认创建（上游仅启动期建文件）。
  - native_custom 分支：仅 `core == CoreType::Mihomo` 时调
    `mihomo_body_for_plan(&raw, mixin_text.as_deref(), tun_text, &settings, opts)?`
    （`tun_text: Option<&'static str>` 直传，与 A09 登记的 `.as_deref()` 语义相同，
    直传系 clippy `needless_option_as_deref` 强制）；其它原生核心原文直通。
    `Err(FIELD_FORMAT)` 经 `?` 在计划构造前返回，调用方保留旧计划，不落盘半成品。
  - tests 模块新增 `sp24_g06_*` 7 则（合成节点 + 合成 mixin/tun 文本，不动宿主网络）。
- `crates/application/tests/r3_03_native_custom.rs`（必要测试更新，超出字面锁请整合者复核）：
  旧 `mihomo_yaml_custom_is_preserved_verbatim` 断言与 G-06 目标直接矛盾
  （接线后按预期红：body 变 merge 输出），已按新合同改为
  `mihomo_yaml_custom_merges_runtime_rewrites`；naive/mieru verbatim 与 xray 严格分支未动。
- codegen.rs / mixin.rs / lib.rs / domain / IPC / BridgePort / FRB / Cargo 锁：未碰。

## 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| 修前 `cargo test -p application --locked --lib engine::tests::sp24_g06` | 非 0 | 全包编译失败：并行批次 `tun_plan.rs` 多余 `}`（锁外，未碰；后由对方修好） |
| 修后同命令 | 0 | 7 pass |
| `cargo test -p application --locked --lib` | 0 | 342 pass，0 fail |
| 同 `--lib mihomo` / `--lib mixin` | 0 | 8 pass / 12 pass |
| `cargo test -p application --locked` 全包 | 0 | lib + 全部集成套件绿（含更新后 r3_03 4 pass；旧 mihomo verbatim 用例在接线后红过一次，见上） |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | 0 | 绿（中途修掉一处 `needless_option_as_deref`） |
| `cargo fmt --all -- --check` | 0 | 绿 |

## 配置 diff（实测，合成数据）

合成 Mihomo Custom（base 含 `secret: hunter2`，`local_port=11808`，TUN 开 / mixin 关 / ipv6 开）：
body 含 `mixed-port: 11808` + `ipv6: true` + `auto-route: true`（嵌入 TUN 段），
`hunter2` 消失，无 `10808`。TUN 关则无 `tun:` 段且 `ipv6: false`（上游无条件重写）。
合成 mixin（`<data>/config/Mixin.yaml`，开关开）：`unknown-kept: 42` 保留 +
`append-rules: MATCH,DIRECT` 合入；开关关则同一文件被忽略。
坏 base（YAML list）：`FIELD_FORMAT`，`applied_session` 为空且 `desired_revision` 不变。
NaiveProxy 同输入：body 与原文逐字相等。

## 阻塞与在途干扰（登记返回）

1. 本卡接线中途 `engine.rs` 被并行批次回退到 HEAD（改动一次性丢失），已按写锁重放并重验 7/7；
   如再次出现写冲突，由主控串行。
2. 并行批次 `tun_plan.rs` 缺省括号曾阻塞全包编译（锁外，未碰）。
3. 正式验收仍缺：设置窗→FRB→保存→restart_core→mihomo 二进制校验→独立重开（需 UI/运行时链路）。

## 卡与 manifest 状态（如实，未改）

任务卡 `docs/repair/stable-port-2026-10-06/tasks/SP-24.md` 与
`execution-manifest.json` 中 SP-24 均为 `identified`（锁外共享状态，本卡未改，由整合者按本证据提升）。
