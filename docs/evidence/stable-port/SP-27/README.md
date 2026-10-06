# SP-27 本产品自更新链路 — 证据

状态：implemented（定向检查绿；受控真实发行源/真实产品 key、正式包、OS 隔离验收未跑，不标 verified）。
基线：`393fafd`。不 commit。合成数据 + 本地 loopback stub（OS 分配端口，断言 ≥11808 且 ≠10808）；
不访问真实互联网，不读用户秘密。`work/`、`outputs/` 只读未动；`compat/` 未动。
未改：engine.rs、lib.rs 导出、IPC/stable DTO、FRB 生成物（`lib/bridge/api/t16.dart` /
`frb_generated.dart` 只读复用已有 `t16ApplyAppUpdateSpecWithFlags` 绑定）、Cargo 锁、
config_codegen、subscriptions。
改动（相对基线新增）：`lib/bridge/bridge_port.dart` 仅加 seam 透传（abstract +
Frb/Synthetic 实现，无 FRB 再生成），见 §6。

## 0. 缺陷复现（红，正确预期未改弱）

- 新增 `apps/desktop/test/repair/sp_27_update_option_rollback_test.dart` 修前 3 fail / 2 pass：
  1. `setPrerelease` 在 `saveGroup` 失败后 state 已翻转且无错误位（先乐观改 state）；
  2. `toggleCore` 同上；
  3. 同上（widget 层：checkbox 已勾选且无 `更新选项保存失败` 文案）。
- 审计第三合同 `settings_retry_contract_test.dart` 之
  `update option failure must be visible or rolled back` 修前红（SP-12 证据 §5 登记）。
- Rust 侧审计缺口（静态确认）：`t16.rs` 进程全局 `UpdateFlags`，
  `t16_apply_app_update_spec`（no-arg）复用“上一次检查” flags，
  检查后改开关或直接点“应用自身更新”会用旧选择；`.sig` 下载未透传
  `via_proxy`（与 artifact 下载不一致）。

## 1. 改动文件

- `apps/desktop/lib/features/update/update_controller.dart`（本卡范围）：
  `_persistCheckUpdate` 改为先存后改并返回是否成功；`toggleCore` /
  `setPrerelease` / `setViaProxy` 仅持久成功才改 `state`，失败保留旧值并置
  `status.kind='error'`（`更新选项保存失败` + `code / messageKey` 可重试）；
  同值调用直接返回，不重复写（幂等）。
- `crates/bridge_api/src/api/t16.rs`（当次 flags 语义，A10 范围；无签名/DTO/生成物变更）：
  删除进程全局 `UpdateFlags` + `remember/last`；`t16_check_updates` /
  `t16_apply_core_update` 仅用当次显式参数；no-arg
  `t16_apply_app_update_spec` 固定安全默认（stable + 直连）且永不复用历史
   选择；`t16_apply_app_update_spec_with_flags` 为当次显式入口（Bridge/UI
   接线见 §6）；验签两路径均不可跳过；`app_repo=None`
  两路径均先报 `error.update_app_source_unconfigured`（无网络动作）。
- `crates/application/src/update_service.rs`：`.sig` 下载透传
  `request.proxy`（与 artifact/`.dgst` 一致，无直连接口泄漏）。
- 新增 `apps/desktop/test/repair/sp_27_update_option_rollback_test.dart`（9 项：5 回滚 + 4 当次 flags 透传，见 §6）。
- 新增 `crates/application/tests/sp27_app_verify.rs`（3 项，loopback stub + 合成信任根）。
- 未动 `updater` / `upgrade_runner` 源码（verify→stage→install→rollback→restart
  链已有覆盖，本卡只跑其测试作证据）。

## 2. 命令与 exit

- `flutter test test/repair/sp_27_update_option_rollback_test.dart`：修前 2/5 pass（3 红）；
  修后 5/5 pass（exit 0）。
- `flutter test test/repair/sp_27_update_option_rollback_test.dart test/t16_update_test.dart test/r3_08_update_source_test.dart test/recheck_rr04_app_update_test.dart`：17/17 pass（exit 0）。
- 审计合同 `docs/evidence/complete-port-audit-2026-10-06/settings/settings_retry_contract_test.dart`：
  修前 2/3；修后 3/3 pass（exit 0）。
- `flutter analyze`（apps/desktop）：No issues found（exit 0）。
- `dart format --set-exit-if-changed`（本卡 3 个 Dart 文件）：exit 0。
- `cargo test -p updater -p bridge_api --locked`：bridge_api 81 pass（含新增
  `app_update_spec_reports_unconfigured_source_without_network`），updater
  69 lib + 全部集成（含 `r4_29_release_chain`）pass（exit 0）。
- `cargo test -p application --locked --test t16_update --test sp27_app_verify`：
  9/9 pass（exit 0）。
- `cargo test -p upgrade_runner --locked`：9/9 pass（exit 0）。
- `rustfmt`（本卡 3 个 Rust 文件）：零 diff；`cargo fmt --all --check` 仍 fail，
  剩余 diff 全在他卡脏文件（`tun_plan.rs`、`net_host` `lifecycle.rs`/`session.rs`），本卡零 diff。
- `cargo clippy -p updater -p bridge_api -p application --all-targets --locked -- -D warnings`：
  fail，唯一 error 在禁止改的 `application/src/engine.rs`（`needless_option_as_deref:4915`；
   另一次跑出同文件 `dead_code:4639`，均为基线已有、他卡未动文件），本卡文件零告警（已登记，见 §7）。
- `cargo test -p application --locked`（全包）：lib 342 pass；1 失败
  `r3_03_native_custom::mihomo_yaml_custom_is_preserved_verbatim`（mihomo custom
   合并行为，他 lane 在改区，非本卡文件；已登记，见 §7）。

## 3. 新增测试（不得改弱）

`test/repair/sp_27_update_option_rollback_test.dart`（合成桥，`_RejectGroupBridge` 故障注入）：

1. 持久失败时 `setPrerelease` 保留旧值 + `status.kind='error'`。
2. 持久失败时 `toggleCore` 保留旧选中 + 错误位。
3. widget 层：失败时 checkbox 保持关闭且显示“更新选项保存失败”。
4. 成功路径：持久成功仍应用新 toggle（防过矫）。
5. 无自有源：`error.update_app_source_unconfigured` 明确阻断 + `busy` 清除可重试（连点两次，两次调用均记录）。
6.–9. 当次 flags 透传（本轮新增，详见 §6）：默认 `false:true` 经 with-flags；
   翻转后 `true:false`；`applyAppUpdateWithDefaults` 只走 no-arg；
   `checkAndApply` check/apply 同一份冻结 flags。

`crates/application/tests/sp27_app_verify.rs`（loopback stub ≥11808 + `PrefixHashVerifier` 合成信任根）：

1. verified 分发→stage→安装→重开命令→回滚全链（helper 永不执行/替换）。
2. 错签名 `E_PERMISSION_DENIED`，零安装。
3. 缺签名 `E_UNAVAILABLE` / `error.update_signature_missing`（验签不可跳过）。

## 4. 语义保证

- 当次 flags 只作用于当次调用：check/apply-core 全显式参数；no-arg 应用更新
  用固定安全默认，不读历史；显式入口 `with_flags` 已就绪待接线。
- 本项目包：hash（sha256/`.dgst`）+ detached 签名双重验证后才 stage；
  `.sig` 与 artifact 同走当次 proxy；安装经外部 runner 原子替换并保留
  `app.previous`，失败恢复旧版，状态可重试；重启命令只构造不 spawn。
- `app_repo=None`（出厂默认）：check 与 spec 均报未配置源，不伪装成功、不触网。

## 6. UI 接线（本轮新增）：当次 flags 经 with-flags 入口

- `lib/bridge/bridge_port.dart`（seam 透传，非 FRB 再生成）：`BridgePort` 新增
  `t16ApplyAppUpdateSpecWithFlags(prerelease, viaProxy)`；`FrbBridgePort` 委托已有生成绑定
  `t16.t16ApplyAppUpdateSpecWithFlags`；`SyntheticBridgePort` 记录
  `app_update_spec_with_flags:<prerelease>:<viaProxy>` 后委托 no-arg 合成成功体，
  使仅覆盖 no-arg 的旧 fake 继续有效、flags 仍可断言。no-arg 入口保留给真正要默认的调用方。
- `lib/features/update/update_controller.dart`：`checkOnly` / `checkAndApply` /
  `installCores` 在操作开始冻结当次 `prerelease`/`viaProxy`，check 与 apply 共用同一份，
  不再两次重读 live state；`applyAppUpdate` 改走 with-flags（冻结后传入）；
  新增 `applyAppUpdateWithDefaults` 走 no-arg 安全默认。`check_update_view.dart`
  无需改动（按钮已委托 controller，当前选择经 state 传入）。
- `test/repair/sp_27_update_option_rollback_test.dart` 5 项回滚测试保持原样全绿，
  新增 4 项：默认选择 `app_update_spec_with_flags:false:true`；开关翻转后
  `true:false`；`applyAppUpdateWithDefaults` 只记 `app_update_spec`、
  无 with-flags 记录；`checkAndApply` 的 check/apply 同为 `:true:true`（冻结一致）。
- 本轮命令（apps/desktop，合成数据，无真实下载；未碰 127.0.0.1:10808）：
  `dart format`（3 文件，1 重排）；`flutter analyze`：No issues found；
  `flutter test`（sp_27 9/9 + t16 7/7 + r3_08 1/1 + rr04 4/4 = 21/21；
  r4_29 合同+repro 10/10；审计 `settings_retry_contract` 3/3，以临时拷贝跑后删除）。
- §7 第 1 项关闭：with-flags 的 Bridge/UI 接线已落在本卡 seam + controller，
  FRB 生成物未动；剩余仍阻塞：真实产品发行源 + 自有信任 key（受控真实端到端未跑，
  不标 verified）。

## 7. 接口需求 / 阻塞（登记，不私改他卡文件）

1. ~~`with_flags` 的 Bridge/UI 接线（待 SP-00）~~已闭环，见 §6：
   `bridge_port.dart` seam + `update_controller` 当次传入，FRB 生成物沿用已有绑定未再生成。
2. 真实产品发行源 + 自有信任 key：`app_repo` 出厂 `None` 为正确默认；
   真实端到端（真实发行端、错公钥/锁/中断/独立重开）待源就绪后验，不标 verified。
3. `application/src/engine.rs` clippy（`needless_option_as_deref` / `dead_code`）
   与全量 `cargo fmt` 剩余 diff、`r3_03_native_custom` mihomo 失败属他卡脏区，
   由对应卡/整合者处理；本卡文件 fmt/clippy 干净。
4. 并行 lane 曾一次覆写 `update_controller.dart`（已重新应用并经 `git diff` +
   全量重测确认）；`update_controller` 当前内容以本证据为准。
