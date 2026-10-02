# T18 缺陷与断链登记

本文件登记 T18 测量/核对中暴露的问题、根因、处理与状态。状态用词与 AGENTS.md 一致。

## T18-F01（已修复，已验证）设置保存失败不回滚内存

- 现象：`engine.save_settings` / `save_settings_group` 先把内存树改成新值、递增 revision，再写盘；
  写盘失败时返回错误，但内存仍是新值，与磁盘/文件不一致。
- 暴露方式：`crates/application/tests/t18_stability.rs::settings_write_failure_is_reported_and_old_value_kept`
  （把 `guiNConfig.json` 变成目录制造写失败）。
- 修复：`crates/application/src/engine.rs` 保存前快照 `(settings, revision, group_revisions)`，
  写盘失败时回滚并返回错误。
- 验证：修复前断言 `left=Some("trace") != right=Some("debug")` 失败；修复后 5/5 测试通过，
  输出 `T18_IO_ERROR {"phase":"settings_save","code":"E_INTERNAL","old_value_kept":true}`。

## T18-F02（已修复，已验证）进程身份把“已退出”误判为存活

- 现象：net-host 强杀后重启恢复时，日志出现 `cleaned=false`，journal 停在 `applied`；
  T03 case d 间歇/复现失败。Xray 已随 Job 退出，但 `matches_identity` 仍返回 true，
  因为进程内核对象在句柄/Job 关闭前仍可查询到创建时间。
- 定位：`crates/runtime/src/identity.rs::process_creation_time_ms` 只看创建时间，未看退出时间；
  recovery 的 `terminate_identity` 对正在退出的进程失败，`gone` 又为 false → `cleaned=false`。
- 修复：读取 `GetProcessTimes` 的 exit FILETIME，非 0 视为已退出，返回 `None`。
- 验证：
  - 新增单测 `identity::tests::an_exited_process_is_not_a_live_identity` 通过；
  - 修复前 `t03_faults.ps1` 的 `d-net-host-kill` 连续 2/2 失败，修复后日志
    `recovery session=... stage=Applied pid=Some(...) cleaned=true`，6/6 故障用例通过。
- 边界说明：`services/net_host` 不在 T18 可写范围，修复落在其依赖的 `crates/runtime`（允许范围）。

## T18-F03（未决，重大断链）生产 `apply_runtime` 使用硬编码冒烟配置

- 现象：`crates/bridge_api/src/api/engine.rs:674 apply_runtime → smoke_plan (634) → smoke_body (603)`
  始终生成固定的 `mixed`/`socks` 入站（端口硬编码 `SMOKE_PORT=11808`）与 `freedom` 出站，
  **不读取**已存储的 profile、settings、routing、DNS；`AppEngine::build_codegen_input`
  的真实生成链只被单测/预览逻辑调用。
- 影响：所有 §10 设置字段的“运行时消费”未接通；当前只能验证到
  `存储 → settings_from_app → config_codegen` 的可测输出层。
- 状态：未决（需要一条 T07/T03 级别的接线任务；超出 T18 “最小修复”范围，未强行修改）。
- 证据：代码行如上；T12b 对照表统一标注“runtime: 未接线”。

## T18-F04（未决）`Inbound.Protocol` 存储但无消费

- 现象：`domain::AppSettings.inbound[i].protocol` 可存可改，但 `settings_from_app` 未映射，
  `config_codegen` 的 Xray/sing-box 入站始终生成 `"protocol":"mixed"`（`xray/inbound.rs:45`）。
- 证据/回归：`crates/application/tests/t18_settings_chain.rs::inbound_protocol_is_persisted_but_generator_always_emits_mixed`
  以特征测试固定当前行为并输出 `T18_SETTINGS_GAP {"field":"FLD-CFG-036",...}`。
- 状态：未决（接线需改生成器语义，超出最小修复）。

## T18-F05（未决）`TunModeItem.EnableLegacyProtect` 存储但无消费

- 现象：`settings_from_app`（`crates/application/src/codegen.rs:268-350`）未引用
  `enable_legacy_protect`；`config_codegen` 入站/TUN 生成也不读它。
- 状态：未决（需确认上游该字段在当前 Xray/sing-box 是否仍有可观察效果，再决定接线或标注 obsolete）。

## T18-F06（提示，非缺陷）脚本化 `jumpTo` 滚动是整表重建上界

- 现象：10k 掉帧率 0.556、50k 掉帧率 1.0，但 raster p95 仅约 3.3ms。
- 解释：S1 骨架的 `jumpTo` 触发整表重建，build 时间被放大；不能据此判定真实拖拽滚动不达标。
- 状态：保留为已知测量限制（见 `docs/evidence/T18.md` 未覆盖项）。
