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

---

# SP-24 continuation（2026-10-07，正式 UI 重连 + codegen 叶子收尾，未 commit）

状态：implemented（UI/生成器侧正确合同绿；正式 UI→FRB→保存→restart_core→
xray/mihomo 二进制校验→独立重开未跑，不写 verified；manifest SP-24 保持
`identified`，由整合者按本证据提升）。
基线：`0b80627`（工作树有并行批次在途改动，本卡仅动下列锁内文件）。

上游复核（冻结 `7d6a967`，只读 `work/`）：
- `Utils.TryParseMaxSplit(input,0,10000)`（`Utils.cs:634`）：空白通过；
  单值或 `from-to`，两端界内且 `from<=to`（`int.TryParse` 容忍空白）。
- `BuildFragmentsMasks`（`V2rayOutboundService.cs:826-865`）：wire 取
  `Split('-')[0]` + `TryParse`（容忍空白，`"1 "`→1）；空回 `tlshello` /
  `["50-100"]` / `["10-20"]`；首值兼 `length`/`delay`。
- `OptionSettingViewModel.SaveSettingAsync`（`:299-307`）：Lengths/Delays 逐项
  `TryParseRange`，MaxSplit 非空才 `TryParseMaxSplit`；`String2List` 去空项
  （`RemoveEmptyEntries`，不 trim）。
- `CoreConfigContextBuilder`（`:100-118`）：排除地址非法项只警告过滤，不拒绝。
- `DNSSettingWindow.xaml`：Happy 仅暴露总开关，4 参数无窗口控件（本卡保留区
  可编辑，不计入上游窗口字段）。

## 改动文件（本卡锁内）

- `apps/desktop/lib/features/settings/option_setting_window.dart`
  - `_validateDraft` MaxSplit 改整数-only 为 `_isValidMaxSplit`
   （`TryParseMaxSplit(_,0,10000)` 镜像：`1-3`/`1 - 3`/空白通过；
    `3-1`/`abc`/`1-2-3`/`0-10001` 拒绝）。修前 `1-3` 存不进（与 Rust 门矛盾）。
  - `_isValidRange` 部件加 `trim`（对齐 `int.TryParse` 空白容忍）。
  - Happy 参数编辑器随 `EnableHappyEyeballs` 门控显隐（值保留，
    同文件 FakeIP/GlobalFakeIp 联动先例；上游窗口仅有开关）。
  - 新增 `_splitList`（`String2List` 镜像：去零长项，尾逗号无幻影项），用于
    Lengths / Delays / `TunModeItem.RouteExcludeAddress` 三处切分。
  - 新增测试 key：`fragment-lengths` / `fragment-maxsplit` /
    `tun-route-exclude` / `happy-eyeballs-toggle` / `happy-try-delay` /
    `happy-prioritize-ipv6` / `happy-interleave` / `happy-max-concurrent`。
- `apps/desktop/test/repair/sp_24_settings_entry_test.dart`（新建）：
  5 则——`1-3` 保存且 raw 保留；`3-1`/`abc`/`1-2-3`/`0-10001` 拒存；
  Happy 关默认隐藏、开后编辑 `TryDelayMs=300` 落盘；关后值保留；
  Lengths/RouteExclude 尾逗号无幻影项。
- `crates/config_codegen/src/xray/config.rs`：`fragment_mask` 首段解析加
  `trim`（`"1 - 3"` wire 1，与上游一致；修前 wire 0）。
- `crates/config_codegen/tests/sp24_happy_fragment.rs`：
  `sp24_fragment_max_split_range_takes_first` 加 `("1 - 3",1)` / `(" 2 ",2)`。
- DTO/保存链核对（只读，无缺口不新建绑定）：`Fragment4RayItemDto` /
  `HappyEyeballs4RayItemDto` / `CoreBasicItemDto`（`enableFragment` /
  `enableFinalFragment`）/ `SimpleDNSItem`（`enableHappyEyeballs`）/
  `ClashUIItem`（`EnableIPv6`/`EnableMixinContent`）齐备；设置窗经
  `saveSettingsJson` 整文档保存，无需新增 FRB 绑定。

## 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `cargo fmt -p config_codegen -p application -- --check` | 0 | 绿 |
| `cargo test -p config_codegen --locked`（含 sp24 7 用例） | 0 | 全包绿 |
| `cargo test -p application --locked` | 0 | lib 346 + 全部集成套件绿 |
| `cargo clippy -p config_codegen -p application --all-targets --locked -- -D warnings` | 0 | 绿 |
| `dart format lib test`（apps/desktop） | 0 | 本卡 2 文件已格式化；全量复查 0 改动 |
| `flutter analyze` | 0 | No issues found |
| `flutter test test/repair/sp_24_settings_entry_test.dart` | 0 | 5/5（首轮 4/5：关 Happy 用例缺 `ensureVisible` 修好；另发现同文件内 repump 模式导致后续用例 `did not complete`，已改为单泵结构并记下） |
| 相邻回归 8 文件（fix16/t12a/r4_13_s07/r4_13_s11/t12a_storage/audit_tun/fix16b） | 0 | 21/21 |

## 仍阻塞（登记返回，主控接线）

1. 正式验收链：设置窗→FRB→保存→restart_core→xray/mihomo 二进制校验→独立重开
   （需 UI/运行时链路）；154/155 旧文档导入→迁移→wire→重开闭环。
2. G-07（生产 `local_srs_files` 填充者，候选 update/engine，SP-00 核定归属）。
3. FLD-CFG-103 Rust 侧：非法排除地址按上游应警告过滤而非整单拒绝
  （`tun_plan.rs` A03 写锁 + 警告通道缺失）；本卡只对齐 UI 切分（去空项）。
4. FLD-CFG-100/102/105：真实 TUN 会话/启停/OS 效果仅 mock（全局 v6 上下文、
   生产 core 路径缺）；codegen 投影已存在。
5. 测试约束记录：同 `testWidgets` 内 `pumpWidget(SizedBox)` 后重泵导致后续用例
   `did not complete`（本仓 flutter_tester 现象）；新用例一律单泵结构。
6. `127.0.0.1:10808` 未触碰；合成数据/夹具；无 OS 副作用；宿主代理/路由/TUN/
   DNS/Run-key 未改动。不 commit。
