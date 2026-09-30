# 项目 AGENTS.md — v2rayN Flutter + Rust 重构

本文件对本仓库内所有执行模型/子代理生效，优先级高于个人默认习惯。

## 项目目标

用 Flutter 做界面、Rust 做应用后端，完整迁移冻结版 v2rayN 7.25.4（commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`）的全部适用功能、设置与行为；界面结构、菜单、表格、设置分组保持原版，在此基础上改善视觉与性能。
规格见 `outputs/V2RAYN_FLUTTER_RUST_PLAN.md`，任务卡见 `docs/tasks/`，台账见 `compat/`，证据见 `docs/evidence/`。

## 环境（已安装，锁定）

| 工具 | 版本 | 路径 |
|---|---|---|
| Flutter | 3.47.5 stable（Dart 3.13.4） | `C:\Users\Colby\toolchains\flutter\bin\flutter.bat` |
| Rust | 1.98.1 stable-x86_64-pc-windows-msvc | `C:\Users\Colby\.cargo\bin\cargo.exe` |
| VS BuildTools | 2022 17.14（含 CMake/Ninja/Win10 SDK 10.0.26100） | VS 自带，另装 CMake/Ninja |
| FRB | 2.13.0（Dart/Rust/codegen 三处同版本） | T01 锁定 |
| Windows | 11 25H2 (build 26220) x64 | — |

## 硬性约束（测试与运行）

1. **禁止占用/修改 127.0.0.1:10808**：这是用户正在运行的代理端口。所有测试、内核启动、入站监听、系统代理操作必须避开它；测试端口一律 ≥ 11808，且测试前先探测端口可用。
2. **禁止修改宿主系统代理**：除经用户明确批准并在隔离测试环境运行的 T13/T14 专项外，任何测试不得调用 WinINET/WinHTTP 代理设置，不得改动注册表代理项。
3. **禁止杀不受本项目管理的进程**：只能停止本项目启动且持有句柄/会话记录的进程；不得按进程名批量终止 v2rayN/Xray/其他代理。
4. **不读取/不发送用户秘密**：订阅地址、节点凭据、cookie 一律不得写入日志、证据或测试夹具；夹具只用公开源码模板与合成数据。
5. `work/` 为 T00 冻结源码（只读）；`outputs/` 为方案原件（只读）；`compat/` 台账发现遗漏只追加条目，不删行、不擅自降低分母。
6. 真实代理流量不过 Flutter/FRB；UI 不拥有内核运行状态；`desired_revision` 与 `applied_runtime_revision` 分离。
7. 不要 commit 任何密钥/凭据；发布与分发遵守上游 GPL-3.0 归属要求。

## 构建与检查命令（发布门禁）

Rust workspace 根：
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Flutter 应用目录（`apps/desktop`）：
```
dart format --output=none --set-exit-if-changed lib test
flutter analyze
flutter test
flutter build windows --release
```

## 目录约定

```
apps/desktop/        Flutter 壳（lib/app、lib/features/*、lib/shared、lib/bridge）
crates/              domain/application/persistence/subscriptions/config_codegen/
                     core_adapters/runtime/platform/updater/bridge_api/ipc_contract
services/net_host/   普通权限运行管理（唯一内核进程持有者）
services/privileged_helper/  最小提权操作
compat/              四份台账 + upstream-lock + 平台矩阵
fixtures/            夹具（source/ 上游模板；其余为合成测试数据）
tests/system/        平台与故障测试
benchmarks/          测量数据
docs/tasks/          任务卡（固定格式见方案 §19）
docs/evidence/       每回合证据；审计报告
docs/decisions/      架构决策记录（ADR）
tools/               开发/审计脚本
```

依赖方向：UI → bridge_api → application → domain/专用模块。domain 不依赖 Flutter/窗口/平台 API；runtime 不调用 UI；平台模块不拥有订阅与节点业务。

## 子代理工作规则

1. 开始前先读对应任务卡、四份台账中的相关条目、上游源码定位；不凭记忆造功能。
2. 每个功能走完：UI → Rust → 持久化 → 重开 → 配置/平台效果 → 测试证据；mock 只用于故障注入。
3. 状态只用 `identified / implemented / verified / preserved_only / blocked / not_applicable`；未运行测试写“未运行”，未实测平台写“未验证”。
4. 代码注释从简，不写叙述性废话；遵循现有模块风格。
5. 完成后更新对应任务卡状态与 `docs/evidence/`，并汇报：改动文件、实际运行的命令与结果、未完成项、下一步前置。
