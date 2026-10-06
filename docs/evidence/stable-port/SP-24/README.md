# SP-24 证据（codegen 级）：Happy Eyeballs 门控 + Fragment wire 语义

状态：implemented（codegen 级正确合同绿；正式 UI→FRB→保存→restart_core→xray 二进制校验→重开未跑，不写 verified）。
基线：`788adf6`（工作树有并行批次在途改动，本卡仅动锁内文件）。
上游冻结：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
manifest `execution-manifest.json:809` SP-24 仍为 `identified`（锁外共享状态，本卡未改，由整合者按本证据提升）。

## 改动文件（锁内）

- `crates/config_codegen/src/xray/dns.rs`：`set_sockopt_domain_strategy` 增加
  `enable_happy_eyeballs: bool` 门控；两处调用点透传
  `simple.enable_happy_eyeballs`。语义对齐
  `V2rayDnsService.cs:543`（`skipHappyEyeballs || EnableHappyEyeballs != true → return`，
  此前仅做 `params != default` 比较，G-01）。
- `crates/config_codegen/tests/sp24_happy_fragment.rs`（新）：7 个正确预期用例。
- `crates/application/src/mixin.rs`（仅 tests 模块）：129/130 helper parity 回归 2 则。

## 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| 新测试修前 `cargo test -p config_codegen --locked --test sp24_happy_fragment` | 非 0 | 4 pass / 3 fail：OFF+非默认参数仍输出块（freedom/dial 双点），ON+全默认缺空块 |
| 修后同命令 | 0 | 7 pass |
| `cargo test -p config_codegen --locked` | 0 | 全包绿（含既有 `xray_proxy_chain_and_happy_eyeballs`） |
| `cargo clippy -p config_codegen --all-targets --locked -- -D warnings` | 0 | 绿 |
| `rustfmt --check`（本卡 3 文件） | 0 | 绿（`cargo fmt --all` 因他人在途 `application/src/dns.rs:663` 括号错位失败，非本卡） |
| `cargo test -p application --locked`（含 codegen/mixin 相关） | 未运行（阻塞） | 同一在途语法错误致整包无法编译；本卡锁内禁碰该文件 |

## 配置 diff（实测）

合成 dualstack 节点，`strategy=UseIP` + `tryDelayMs=300/prioritizeIPv6=true/interleave=2/maxConcurrentTry=3`：

- 修前 OFF：`sockopt` 含 `"happyEyeballs":{"tryDelayMs":300,"prioritizeIPv6":true,"interleave":2,"maxConcurrentTry":3}`（错）。
- 修后 OFF：`sockopt` 仅 `"domainStrategy":"UseIP"`，无 `happyEyeballs` 块（对）。
- 修后 ON：块与值齐出（对）；ON+全默认：`"happyEyeballs":{}`，与原版 `??= new()` + `WhenWritingNull` 一致。

## 逐字段结果

- 176（EnableHappyEyeballs 门控）：修好，codegen 绿；正式链路未验。
- 177..180（4 参数）：ON 落盘 / OFF 隐藏真值表绿；正式链路未验。
- 150..152（Packets/Lengths/Delays）：wire 与回退（`tlshello`/`50-100`/`10-20`）同原版一致，修前即绿（parity 锁）。
- 153（MaxSplit）：wire 首整数（`1-3`→1）绿；**Rust 校验门拒绝合法范围串被 G-02 阻塞**（`application/src/settings.rs:93`，锁外）。
- 154/155（legacy Length/Interval 迁移）：`domain` 锁外，未动；语义已在 SP-23 登记，无新证据。
- 129/130（Mihomo merge）：helper 与投影层 parity 绿（含新增回归）；**正式 custom 计划接入被 G-06 阻塞**（`engine.rs:4325-4339` 原文直通，锁外）。
- SRS 本域字段（086/087 类）：SP-25 归属 + 锁外（`local_srs_files` 无生产填充者 G-07），本卡仅登记。

## 阻塞（登记返回，主控接线）

1. G-02：`application/src/settings.rs:91-99` 按 `Utils.TryParseMaxSplit(input,0,10000)` 重写校验（接受空/单值/`from-to`且`from<=to`全域界内，非法保留旧值 `E_FIELD_FORMAT`）；需 domain/settings writer。
2. G-06：`engine.rs:4325-4339` native_custom 分支接入 `mixin_options_from_app` + `generate_mihomo`（坏 YAML 拒绝且旧计划保留，未知键保留）；需 engine writer（SP-00 整合）。
3. 本卡 application 侧编译验证：待他人在途 `application/src/dns.rs` 语法修复后补跑 `cargo test -p application --locked` + clippy。
4. 正式验收仍缺：DNS 窗→FRB→保存→restart_core→xray 二进制校验→独立重开（需 UI/运行时链路）。

## `git status --short`（本卡相关）

`M crates/config_codegen/src/xray/dns.rs`、`M crates/application/src/mixin.rs`（仅 tests）、
`?? crates/config_codegen/tests/sp24_happy_fragment.rs`、本证据目录新文件。
其余改动均为并行批次在途文件，本卡未碰。不 commit。
