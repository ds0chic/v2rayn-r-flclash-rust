# SP-24 FLD-CFG-103 Rust 侧：TUN route-exclude 非法项警告过滤（2026-10-07，未 commit）

状态：implemented（生成器/plan 侧正确合同绿；`generated.diagnostics` 接线仍在锁定的
`engine.rs` TUN attach 位，正式 UI→FRB→保存→restart_core→二进制校验→重开未跑，不写 verified。
manifest SP-24 保持 `identified`，由整合者按本证据提升）。

基线：`36e7472`（工作树有并行批次在途改动，本卡仅动下列锁内文件）。

## 上游行为（冻结 `7d6a967`，只读 `work/`，本次复核）

- `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs:100-118`（`Build`）：
  TUN 开且 `RouteExcludeAddress is { Count: > 0 }` 时，深拷贝 config 后逐项
  `IPNetwork2.Parse(addr)`；成功进 `routeExcludeAddressList`，抛异常则
  `validatorResult.Warnings.Add(string.Format(ResUI.MsgTunRouteExcludeInvalidAddress, addr))`。
  结论：**非法项只警告过滤，有效项保留，绝不整单拒绝**；全非法→空列表回填。
- `ServiceLib/Resx/ResUI.resx:1803-1805`：
  `MsgTunRouteExcludeInvalidAddress = "Invalid address in TUN route exclude list: {0}"`
  （本卡 warning message 逐字镜像）。
- `ServiceLib/Common/Utils.cs:44-54`（`String2List`）：
  `Split(',', RemoveEmptyEntries)`，空项（尾逗号）在切分层已丢弃，不进校验；
  空白项（`" "`）保留→`Parse` 抛异常→警告过滤（本卡 `parse_cidr` 对应：无 `/` 即非法）。
- `ServiceLib/Services/CoreConfig/V2ray/V2rayInboundService.cs:93-99`：
  下游消费时 `.Select(IPNetwork2.Parse).Where(x => x != null)`，即消费侧同样是
  “跳过非法、保留有效”语义，与 builder 过滤一致。
- codegen 侧 Rust 已天然一致，无需改动：
  `crates/config_codegen/src/util.rs:816-835`（`tun_route_table`）对解析失败项
  静默跳过（`if let Some`），sing-box `inbound.rs:67-70` 原样透传（校验归 plan 边界）。

## 改动文件（本卡锁内）

- `crates/application/src/tun_plan.rs`（唯一生产改动）：
  - 新增 `TUN_ROUTE_EXCLUDE_INVALID_CODE = "tun_route_exclude_invalid"`
   （沿用 `routing_dangling_reference` 式 snake_case 诊断码）与
    `TUN_ROUTE_EXCLUDE_FIELD = "RouteExcludeAddress"`（对齐上游符号，
    同文件 `IPv4Address` 字段名先例）。
  - 新增 `filter_route_exclude(&[String]) -> (Vec<String>, Vec<config_codegen::Diagnostic>)`：
    `ipc_contract::parse_cidr`（≈ `IPNetwork2.Parse`：`address/prefix_len`，
    两侧空白容忍）逐项判定；合法保留原串，非法逐条
    `Diagnostic::warning(code, "Invalid address in TUN route exclude list: {entry}",
    Some("RouteExcludeAddress"))`。
  - `build_tun_spec_fields` 改走 `build_tun_spec_fields_with_warnings`：
    resolved/deferred 两路统一过滤，有效项落盘。
  - 新增 `tun_spec_from_settings_with_warnings` /
    `tun_deferred_spec_from_settings_with_warnings`（返回 spec + warnings）；
    原 `tun_spec_from_settings` / `tun_deferred_spec_from_settings` 签名不变
    （`engine.rs` 锁定，调用方零改动），内部同过滤、warnings 待接线。
  - 现有警告通道确认：plan 构建返回 `Result<RuntimePlan, DomainError>`，
    无 warnings 出口；已存在通道是 `GeneratedConfigs.diagnostics`
   （`build_codegen_input_with_warnings` 链式 warnings 已有
    `generated.diagnostics.extend(chain_warnings)` 先例，`engine.rs:4905`）。
    未发明新全局通道。
- `crates/application/tests/sp24_tun_route_exclude_warn_filter.rs`（新建，3 则）：
  engine 级——混合表仍成 plan 且只带有效项；全非法表仍成 plan 且 exclude 为空；
  `filter_route_exclude` 逐条结构化警告（code/field 断言）。
- 本文件 + README 续节 + manifest SP-24 note（诚实追加）。
- 未碰（硬约束遵守）：`engine.rs`、`lib.rs`、`crates/bridge_api/**`、
  `crates/ipc_contract/**`、`services/**`、workspace Cargo 文件；
  `127.0.0.1:10808` 未触碰；合成夹具；无 OS 副作用。不 commit。

## 登记返回（主控/整合者接线，`engine.rs` 解锁后）

`build_runtime_plan_with_hints` TUN attach 位（`engine.rs:5103-5113`）切换：

```rust
// resolved 路：
let (spec, tun_warnings) =
    tun_plan::tun_spec_from_settings_with_warnings(&settings.tun_mode_item, tun_hints)?;
generated.diagnostics.extend(tun_warnings);
// deferred 路同理（tun_deferred_spec_from_settings_with_warnings）。
```

切换前：非法项已被过滤生效，但警告不可见（静默丢弃，属已知缺口）；
切换后：警告进 `generated.diagnostics`，与 chain warnings 同通道可见。
`TunSpec::validate`（`runtime::tun`）保持严格——手造非法 spec 仍拒绝
（`rejects_bad_route_exclude` 未动），过滤只发生在 settings→spec 边界，
与上游 builder/下游消费分层一致。

有意偏离记录：`""` 空串上游在 `String2List` 切分层已丢（不到校验），
本过滤器将其计为一条警告（持久化可直写空串，上游 UI 路径到不了）。
行为方向一致（不生效），仅警告计数差一，见单测 `filter_route_exclude_unit_matrix`。

## 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `cargo fmt -p application -- --check`（修后复查） | 0 | 绿（修前 3 处 diff，已 `cargo fmt -p application` 落盘） |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | 0 | 绿 |
| `cargo test -p application --locked --lib tun_plan` | 0 | 24 pass（含新 4 则；`bare_ip`/`zero_interface`/`out_of_range_mtu` 严格项仍红线绿） |
| `cargo test -p application --locked --test sp24_tun_route_exclude_warn_filter` | 0 | 3/3 |
| `cargo test -p application --locked` 全包 | 非 0（1 项，他人在途） | lib 350 + 全部集成套件绿；唯一红 `t10_core_matrix_live`（并行批次新增 untracked 文件，本卡未碰；`overtls` 真实二进制 `server_listened=true client_listened=true proxied=None`，与 route_exclude 零引用，见下） |

`t10_core_matrix_live` 无关性验证（只读）：
`Select-String route_exclude|tun_plan|RouteExclude` 在该文件 0 命中；
失败点 `t10_core_matrix_live.rs:815` 为 overtls live-core 透传阻塞，
13 ok / 1 blocked。非本卡回归。

## 仍阻塞

1. 上述 engine.rs diagnostics 接线（本卡锁外，需整合者）。
2. 正式验收链：TUN 窗→FRB→保存→plan→core 校验/真实路由→重开（需 UI/运行时链路，
   FLD-CFG-103 完成条件，最终门禁 SP-34）。
3. README 续节 gap 3（FLD-CFG-103 Rust 侧）本次由“缺警告通道”推进为
   “待 engine 接线”，G-06/G-07/正式验收链不变。
