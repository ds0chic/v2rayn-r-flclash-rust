# R3-02/03/05/07 修复证据

日期：2026-10-04。仓库根：`C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn`。
开始 HEAD：`6699c31`（工作树仅有其它子代理的 `round3-profiles.md`/`round3-settings.md` 未提交改动，未触碰）。
对照冻结：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。复核来源：`docs/evidence/parity-recheck-2026-10-04/round3-runtime.md`。

本子代理只改了允许清单内的文件；未 `git add/commit`；未跑全量 workspace 测试/全仓 fmt；未占用/修改 10808；测试端口全部 ≥11808；未创建真实 TUN 设备、未写宿主系统代理/注册表/路由。

## 结论

| 项 | 状态 | 关键改动 |
|---|---|---|
| R3-02 | implemented | 侧车配置消费真实设置（入站=本地端口、socks 出站=主核端口、TUN/DNS/路由/统计）；net-host 顺序改为主核→ready→侧车 |
| R3-03 | implemented | 非 Xray/sing-box 核的原生 Custom 逐字保留；逐核 env（mieru `MIERU_CONFIG_JSON_FILE`）/工作目录合同 |
| R3-05 | implemented | helper 成功后侧车失败统一走 `rollback`（释放 TUN lease、finalize journal、清 lease/link/session_id） |
| R3-07 | implemented | Custom 端点补全 listen/认证/Clash secret/API 类型；无 API 置零不探测默认统计 API；bridge 同步 secret |
| R3-04 | blocked | 首次 TUN 设备创建需隔离 VM，本轮不做真实设备创建（保持登记） |

## 改动文件

- `crates/application/src/codegen.rs`：`generate_pre_socks_config(core, dial_address, dial_port, opts, settings, routing, dns)`，用 `settings_from_app` 组装真实设置。
- `crates/application/src/engine.rs`：`core_uses_json_endpoints`；Custom 原生配置逐字保留；侧车生成传入 opts/settings/routing/dns；`AppliedFacts`/`MonitorSession` 增加 `api_kind`/`api_secret`；Custom 无 API 端口置零；新增 3 个单测。
- `crates/runtime/src/adapter.rs`：`CoreAdapter::env_vars`/`working_dir` 默认实现；`MieruAdapter` 注入 `MIERU_CONFIG_JSON_FILE`；新增 2 个单测。
- `crates/runtime/src/endpoints.rs`：`ResolvedInbound{listen,authentication}`、`ApiKind`、`CustomApi{kind,listen,port,secret}`、`CustomEndpoints::api`/`api_port()`、`split_host_port`；重写单测。
- `crates/runtime/src/lib.rs`：导出新端点类型。
- `crates/bridge_api/src/api/monitor.rs`：`sync_from_engine_session` 发布 `hub.secret` 并纳入 `SessionSignature`；停止时清 secret。
- `services/net_host/src/session.rs`：侧车启动移至主核 ready 之后（新增 `start_sidecars`/`start_one_sidecar`）；侧车失败经 `rollback`；主核/侧车 spawn 应用 adapter env/workdir；新增 R3-05 单测并更正旧测试名。
- `crates/application/tests/r3_02_presocks_sidecar.rs`、`crates/application/tests/r3_03_native_custom.rs`（新增）。
- `docs/tasks/R3-02.md`、`R3-03.md`、`R3-05.md`、`R3-07.md`（新增）。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p runtime -p application -p bridge_api -p config_codegen -- --check` | exit 0（`fmt-check.log`） |
| `cargo clippy -p runtime -p application -p bridge_api -p net_host -p config_codegen --all-targets --locked -- -D warnings` | exit 0（`clippy.log`） |
| `cargo test -p runtime --lib --locked` | 55 passed（`runtime-tests.log`） |
| `cargo test -p application --lib --locked` | 219 passed（`application-lib-tests.log`） |
| `cargo test -p bridge_api --lib --locked` | 55 passed（`bridge-tests.log`） |
| `cargo test -p application --test r3_02_presocks_sidecar --test r3_03_native_custom --test fix13_tun_presocks_plan --locked` | 2/4/6 passed（`application-integration-tests.log`） |
| `cargo test -p net_host --locked` | 58 passed（`net-host-tests.log`，含 `r305_...`） |

未运行（超出可实测边界）：真实 Xray/sing-box/mihomo 双核运行、真实 Custom API/Clash 连接、真实 UAC/TUN 接口创建与路由/DNS 写入、系统代理/PAC 写入、Flutter 构建/GUI。未读用户真实配置/节点/秘密。

## 上游对照结论

- R3-02：`CoreManager.cs:94-96` 明确为 main→`WaitForProxyPort`→`CoreStartPreService`；`GetPreSocksItem` 的 `Address=Loopback`/`Port=主核端口`直接映射为侧车 socks 出站。Rust 侧车入站现取 `AppManager.GetLocalPort` 对应的 `opts.local_port`，与冻结语义一致。
- R3-03：`CoreInfoManager.cs:282-289` 的 mieru 环境注入已还原；`CoreConfigHandler.cs:16-83` 的“原生文件复制”语义现只在 Xray/sing-box 结构化核上解析端点，其它核逐字透传。
- R3-05：与冻结的逆向清理语义一致（helper 资源先于 journal）。
- R3-07：`experimental.clash_api.external_controller` + `secret` 映射为 `CustomApi(ClashApi)`；`metrics.listen`/`api` dokodemo-door 映射为 `XrayStats`。secret 仅作数据传递，未入日志。

## Blocked / 未完成 / 接口缺口

- R3-04（首次 TUN 创建）：`blocked`，需隔离 VM 实测核心创建接口→发现→设地址/路由→ready→回收（含 UAC 拒绝与重开）。
- R3-07 的“系统代理/PAC 入口按协议区分（SOCKS/HTTP）”：入口在 `apps/desktop/lib/features/**`（`platform_controller.dart`/`proxy_settings_view.dart`），在本子代理禁止修改清单内；Rust 侧已按 `ProxyProtocol` 产出 scheme/listen/auth 事实，待 UI 子代理消费（PAC 需 `PROXY`/`SOCKS5` 指令串，关联 R3-01）。
- R3-03：原生核（mihomo 等）实际监听端口无法通用解析，计划暂用配置本地端口并在事实中标记“原生/无端点”；精确获客端口需逐核解析器。
- R3-02：net-host 对非 TUN（Custom）侧车未二次探测其本地获客端口；当前依赖配置正确性与主核 ready。若需严格验收，应在 plan 中区分 sidecar 的 listen 端口与 dial 端口。
- 真实双核隔离测试与 14 核可运行性未验证；14 adapter 存在不等于 14 核已实测。
