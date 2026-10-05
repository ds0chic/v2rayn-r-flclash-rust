# R4-20 证据 — 策略组和代理链

状态：implemented（配置层已实现并验证；真实内核请求路径未验证/blocked）。
HEAD：7e9a4b49de8c287abc920b5a3eff1c1b2cb74a1b（工作树改动未提交）。
armed=false（未武装包/未写系统代理/未动注册表/TUN/路由/自启）。
测试端口：无内核监听；未占用 127.0.0.1:10808（该端口由用户 Desktop 的 v2rayN/xray 占用，未触碰）。

## 本轮唯一用户流程
引用多个节点形成策略组或代理链后验证真实请求路径。

## 对照上游（冻结 UP：work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/）
- `ServiceLib/Manager/GroupProfileManager.cs`
  - `GetChildProfileItemsByProtocolExtra` = `GetSubChildProfileItems`（SubChildItems+Filter，只保留 `IsValid()` 且 `!IsComplexType()||Outbound`）先，`GetSelectedChildProfileItems`（ChildItems 保序）后。
  - `HasCycle` 为保存前环检测。
- `ServiceLib/Handler/ConfigHandler.cs`：策略组生成 balancer/observatory；代理链生成 dialerProxy/detour，ChildItems 逆序。
- 结论：本仓库 config_codegen 的 xray/sing-box 组/链展开与上游一致（字段/顺序/引用）；缺口在“订阅子组”（SubChildItems）从未被生成消费者解析——自动/区域策略组此前无法生成配置（`group node has no child items`）。

## 改动文件
- apps/desktop/lib/features/profiles/group_editor_dialog.dart：草稿预览的 `_isProfileValid` 补 Shadowsocks 方法校验，镜像 `domain::Profile::is_valid`（`Global.SsSecuritiesInSingbox`），修正“预览与生成不一致”。
- crates/application/src/codegen.rs：`build_input` 生成前用 `groups::resolve_sub_children`+`groups::child_index_ids` 解析组有效 `ChildItems`（订阅子组在前、显式子项保序；悬空显式 id 保留以让生成器报可读错误）。
- crates/config_codegen/src/xray/outbound.rs、singbox/outbound.rs：新增组递归栈守卫，环数据返回 `invalid_reference` 可读错误而非栈溢出。
- apps/desktop/test/repair/r4_20_repro_test.dart（新增，先失败后通过）。
- apps/desktop/test/r4_20_contract_test.dart（新增）。
- crates/config_codegen/tests/r4_20_groups.rs（新增，环/嵌套拓扑）。
- crates/application/src/codegen.rs（新增 3 个 `r4_20_*` 单测）。

## 必须通过场景
- 组模式（五模式 LeastPing/Fallback/Random/RoundRobin/LeastLoad）：既有 FIX-03/03C 分派不回退；本轮新增“订阅子组→balancer/observatory”生成断言。
- 区域生成/顺序/嵌套/循环/成员删除/跨订阅ID变化：见 contract + 单测（区域 filter、index 顺序、嵌套保留、环可读错误、subid/filter 不依赖 id）。
- 链（ProxyChain）顺序：dialerProxy/detour 逆序，R4-19 模板合并不回退（workspace 测试全绿）。
- port0 默认规则：组草稿 port=0（`startAddGroupProfile`），`is_valid` 对 complex 直接放行。
- 重开无悬空引用：保存路径 `validate_group` 拒绝悬空/环；生成路径对悬空显式 id 报 `dangling_reference`。
- 取消不写：contract widget 测试断言。

## 命令与结果
| 命令 | 结果 |
|---|---|
| flutter analyze | No issues found |
| flutter test test/repair/r4_20_repro_test.dart | 修复前 FAIL（实际含 ss-null/ss-plain），修复后 +1 通过 |
| flutter test test/r4_20_contract_test.dart | 7 passed |
| 组相关既有测试（7 文件） | 24 passed |
| cargo fmt --all -- --check | exit 0 |
| cargo clippy --workspace --all-targets --locked -- -D warnings | exit 0 |
| cargo test --workspace --locked | exit 0，全绿 |
| cargo test -p application --lib r4_20 | 3 passed |
| cargo test -p config_codegen --test r4_20_groups | 3 passed |
| flutter build windows --release | Built build\windows\x64\runner\Release\v2rayn_desktop.exe |

## 未完成 / 未验证
- 真实内核请求路径（真实流量经生成配置、策略切换、链路顺序）**未验证**：需授权隔离环境（≥11808 测试端口、独立内核进程）本回合未具备，登记 blocked。
- R4-20 台账（coverage.csv ACT-MAIN-014/015、LAY-ADDGROUP-001、LAY-PROFILESELECT-001）状态未在 compat 中提升，仅追加不改分母，待真实运行证据。
- 五模式在真实内核的策略切换效果未实测。

## 清理记录
- 无测试内核进程启动；无端口监听；无系统代理/注册表/TUN/路由/Run-key 改动。
- 用户桌面 v2rayN.exe / xray.exe 未被启动、未被停止、未被触碰。
