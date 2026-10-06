# SP-24 证据（codegen 级）：Happy Eyeballs 门控 + Fragment wire 语义

状态：implemented（codegen 级正确合同绿；正式 UI→FRB→保存→restart_core→xray 二进制校验→重开未跑，不写 verified）。
基线：`3635392`（工作树有并行批次在途改动，本卡仅动锁内文件）。
上游冻结：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
manifest `execution-manifest.json` SP-24 仍为 `identified`（锁外共享状态，本卡未改，由整合者按本证据提升）。

## 改动文件（锁内，本卡）

- `crates/application/src/settings.rs`：校验门按上游
  `Utils.TryParseMaxSplit(input,0,10000)` 重写（G-02 修好）；新增
  `try_parse_max_split` + 矩阵单测 2 则（`validate_accepts_max_split_range_form` /
  `validate_rejects_bad_max_split_range`）。
- `crates/application/src/codegen.rs`：新增 `mihomo_body_for_plan`
 （G-06 生成器侧入口，冻结引擎调用序列）；`CodegenOptions` 新增
  `local_srs_files: BTreeSet<String>`（调用方快照，纯输入无 IO）；
  `settings_from_app` 投影 `ruleset_url`（`effective_srs_source`，空回内置）
  与 `local_srs_files`；新增 `sp24_*` 单测 4 则。
- `crates/application/src/mixin.rs`（仅 tests 模块）：G-06 生成器侧消费证明
  `sp24_rejects_bad_mixin_and_retains_unknown_keys`。
- `crates/application/tests/t11_codegen_matrix.rs`：`opts()` 补
  `local_srs_files: Default::default()`（新字段适配，无语义改动）。
- 前批已合入（`3635392` 含）：`crates/config_codegen/src/xray/dns.rs`
  Happy 门控、`crates/config_codegen/tests/sp24_happy_fragment.rs` 7 用例。

## 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| 新 settings 矩阵修前 `cargo test -p application --locked --lib settings::` | 非 0 | `1-3` 被拒（accept 失败）；`10001` 无上界通过（reject 的 `unwrap_err` 炸） |
| 新 codegen 测试修前（同命令 `codegen::tests::sp24`） | 非 0 | 编译错：`mihomo_body_for_plan` 未定义、`local_srs_files` 无字段 |
| 修后 `cargo test -p application --locked --lib sp24` | 0 | 5 pass（codegen 4 + mixin 1） |
| `cargo test -p config_codegen --locked` | 0 | 全包绿（含 `sp24_happy_fragment` 7 用例与既有 parity 锁） |
| `cargo test -p application --locked` | 0 | 全包绿（含 lib 330 pass；合成节点，无 10808） |
| `cargo clippy -p config_codegen -p application --all-targets --locked -- -D warnings` | 0 | 绿 |
| `cargo fmt --all -- --check` | 非 0（他人在途） | 本卡 4 文件 `rustfmt --check` 绿；剩余 diff 仅 `application/src/monitor.rs`（并行批次在途，本卡未碰） |

## 配置 diff（实测）

合成 vless 节点（`192.0.2.72`）+ `enable_final_fragment` + `max_split="1-3"`：

- 校验门：修前拒存（`E_FIELD_FORMAT`）；修后通过，raw `1-3` 保留。
- wire：`outbounds[0].streamSettings.finalmask.tcp[0].settings.maxSplit == 1`
 （与原版 `Split('-')[0] TryParse` 一致）。
- 合成 mihomo base + mixin：开→`ipv6: true` + `MATCH,DIRECT` + 未知键保留；
  关→原文无 merge；坏 base/坏 mixin→`FIELD_FORMAT`（调用方保留旧计划）。

## 逐字段结果

- 176（EnableHappyEyeballs 门控）：前批修好，codegen 绿；正式链路未验。
- 177..180（4 参数）：ON 落盘 / OFF 隐藏真值表绿；正式链路未验。
- 150..152（Packets/Lengths/Delays）：wire 与回退（`tlshello`/`50-100`/`10-20`）同原版一致，parity 锁绿。
- 153（MaxSplit）：**G-02 本卡修好**——`1-3` 可存且 wire 首整数，非法值
  （倒序/越界/多段/非数字）如实拒绝（`FIELD_FORMAT/Fragment4RayItem.MaxSplit`）。
- 154/155（legacy Length/Interval 迁移）：只读核对通过——
  `domain/src/settings.rs:1119-1132` 仅缺省时回填（显式值优先，legacy 作 fallback 源）；
  domain 锁外未动，无新证据。
- 129/130（Mihomo merge）：**生成器侧本卡可消费**——`mihomo_body_for_plan` +
  helper/投影 parity 绿；正式 custom 计划接入仍被 G-06 阻塞（见下）。
- SRS 本域字段（086/087 类）：**生成器侧本卡可消费**——`ruleset_url` 投影 +
  `local_srs_files` 快照→`local` rule_set 单测绿；生产快照填充者仍缺（G-07，见下）。

## 阻塞（登记返回，主控接线）

1. G-06（engine writer）：锁内 `engine.rs:4840-4848` native_custom 分支原文直通。
   精确接线：原文解析出后、落盘前插入
   `mihomo_body_for_plan(&raw, mixin_text.as_deref(), tun_text.as_deref(), &settings, opts)?`；
   `mixin_text` = mixin 文件内容（IO 归 engine），`tun_text` = 嵌入 TUN YAML 文本；
   `Err(FIELD_FORMAT)` 时保留旧计划并可见错误（不落盘坏配置）；未知键保留已由生成器保证。
2. G-07（SP-00 核定归属）：`local_srs_files: BTreeSet<String>` 快照的生产填充者
   （扫描 `bin/srss/*.srs` → 填 `CodegenOptions.local_srs_files`）无归属实现；
   候选归属 update/engine，network 模块禁碰，本卡仅消费端证明。
3. 正式验收仍缺：DNS/设置窗→FRB→保存→restart_core→xray/mihomo 二进制校验→独立重开
   （需 UI/运行时链路）；154/155 旧文档导入→迁移→wire→重开闭环（需 domain writer 协同用例）。

## `git status --short`（本卡相关）

`M crates/application/src/settings.rs`、`M crates/application/src/codegen.rs`、
`M crates/application/src/mixin.rs`（仅 tests）、
`M crates/application/tests/t11_codegen_matrix.rs`（新字段适配）、本证据目录更新。
其余改动均为并行批次在途文件，本卡未碰。不 commit。
