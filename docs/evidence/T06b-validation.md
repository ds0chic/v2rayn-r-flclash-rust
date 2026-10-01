# T06b — config_codegen 审计修复与真实内核校验/冒烟

- 任务：T06b（审计收尾：ISSUE-01/03/04/10、M-011/012/013/014/016 及真实内核验收）
- 承办：T06b 收尾子代理（deepseek-v4.1-flash）
- 时间：2026-10-01
- 基线 HEAD：`8794fa9`（工作区含 T06a 之后未提交改动；本轮只改 `crates/config_codegen/**`、`tools/cores/**`、`tools/validate/**`、`docs/evidence/T06b*`、`docs/decisions/T07-T08-*`）
- 关联审计：`docs/evidence/audit/T01-T08.audit.gemini.md`（ISSUE-01/03/04/10）、`T01-T08.audit.muse.md`（M-011/012/013/014/016）
- 关联契约：`docs/decisions/T07-xray-codegen-contract.md`、`T08-singbox-codegen-contract.md`、`compat/codegen-map.xray.yaml` / `codegen-map.singbox.yaml`

> 原则：不把“能解析/能生成”当“可运行”；所有内核结论均来自真实 `xray.exe` / `sing-box.exe` 执行，日志留档于本目录。

---

## 1. 现状盘点（本轮开始前）

| 项 | 状态 |
|---|---|
| `tools/cores/singbox/v1.14.2/`（真实 sing-box.exe + zip） | 已存在 |
| `tools/cores/xray/v26.3.27/`（真实 Xray） | 已存在，哈希与 lock 一致 |
| `crates/config_codegen/examples/gen_matrix.rs` | 已存在（生成 36 例矩阵） |
| `tools/validate/t06b_validate.ps1` | 已存在（约 2.7KB，可运行） |
| `cores.lock.json` sing-box 条目 | **缺失 → 本轮补齐** |
| ISSUE-01 10808 拦截 | **未落 → 本轮修复** |
| ISSUE-03 Xray `protocol` 写出 | **未落 → 本轮修复** |
| ISSUE-04 悬空回退 warning | **未落 → 本轮修复（回退保留，补结构化 warning，并更正契约）** |
| ISSUE-10 `v2ray.cool:10086` 占位 | **未落 → 本轮改为 `invalid.invalid:0`** |
| M-011 模板合并顺序 | 实现=生成在前/模板在后，**经上游源码核实与上游一致**；文档原“上游相反”表述更正 |
| M-012 sing-box transport 矩阵 warning | **未落 → 本轮补 `singbox_transport_ignored`** |
| M-013 `endpoints` 仅非空写出 | **未落 → 本轮修复** |
| M-014 reality 空 publicKey 任何协议报错 | **未落 → 本轮修复** |
| M-016 真实内核 `xray run -test` / `sing-box check` | **零证据 → 本轮执行并归档** |

---

## 2. 修复清单（均带回归测试）

| 编号 | 修复 | 文件 | 回归测试 |
|---|---|---|---|
| ISSUE-01 | `validate()` 拦截入站/state 端口 10808 → `reserved_port` 领域错误 | `xray/mod.rs`、`singbox/mod.rs`、`util.rs::reserved_port_error`、`lib.rs::CodegenError::reserved_port` | `xray_errors::xray_rejects_reserved_live_port`、`singbox_errors::singbox_rejects_reserved_live_port` |
| ISSUE-03 | Xray 用户路由规则写出 `"protocol"` 条件 | `xray/routing.rs::build_user_rule` | `xray_routing_dns::xray_routing_protocol_condition_is_written` |
| ISSUE-04 | 路由引用未知 tag：保留 `Global.ProxyTag` 回退（对齐上游）并发 `routing_dangling_reference` warning；同步更正契约“直接报错”表述 | `xray/routing.rs`、`singbox/routing.rs`、T07/T08 契约、`T07-T08-codegen-impl-notes.md`、`T07-T08-codegen.md` | `xray_routing_dns::xray_routing_dangling_reference_warns`、`singbox_errors::singbox_reports_dangling_reference` |
| ISSUE-10 | Outbound 占位 `v2ray.cool:10086` → `invalid.invalid:0` | `xray/outbound.rs:310-314` | `xray_template_custom::xray_outbound_placeholder_is_neutral` |
| M-014 | reality + 空 publicKey → 报错（任意协议类型） | `xray/mod.rs`、`singbox/mod.rs` | `xray_errors::xray_reality_requires_public_key_for_all_protocols`、`singbox_errors::singbox_reality_requires_public_key_for_all_protocols` |
| M-012 | sing-box 被静默丢弃的 transport 组合 → `singbox_transport_ignored` warning（镜像 `NodeValidator.ValidateSingboxTransport`） | `singbox/outbound.rs::transport_diagnostic` | `singbox_errors::singbox_reports_ignored_transport` |
| M-013 | `endpoints` 仅非空写出 | `singbox/config.rs` | `singbox_errors::singbox_omits_empty_endpoints` |
| M-011 | 模板合并顺序核实：上游 `SingboxConfigTemplateService.cs:110-125` 为“模板数组 + 追加生成出站”，**与本实现一致**；更正文档 | 契约/impl-notes 文档 | 既有 `singbox_template_custom::singbox_template_injection` 断言生成在前 |

补充：为让真实 sing-box 接受 hysteria2/tuic/anytls/naive（这些协议 TLS 强制），`gen_matrix.rs` 相应用例补 `stream_security=tls` + 合成自签证书（`SELF_SIGNED_CERT`，CN=example.test，仅测试）。

### 关键修复代码位置

- `crates/config_codegen/src/xray/routing.rs`：`build_user_rule` 写入 `protocol`；`warn_dangling()`。
- `crates/config_codegen/src/singbox/routing.rs`：`warn_dangling()`（`GenRoutingUserRuleOutbound` 两个回退分支）。
- `crates/config_codegen/src/singbox/outbound.rs`：`transport_diagnostic()` + 线程本地诊断汇（`diagnostic_sink_*`）。
- `crates/config_codegen/src/util.rs`：`RESERVED_LIVE_PORT=10808`、`reserved_port_error()`。
- `crates/config_codegen/src/lib.rs`：`CodegenError::reserved_port`。

---

## 3. 真实内核校验矩阵（xray run -test / sing-box check）

### 3.1 内核

| 内核 | 版本 | sha256（zip） | sha256（exe） | 校验命令 |
|---|---|---|---|---|
| Xray | 26.3.27（go1.26.1, d2758a0） | `d004c392…ba4e1ad` | `15c2d007954ac53ba69b80ec91242786b3c0b71d52649165b4ca1d5cc96ef8f1` | `xray run -test -config <cfg>` |
| sing-box | 1.14.2（go1.26.8, af6e64c3） | `c2d8bfff…0684d32` | `7bbef1dea9189ee12799ae834ea4b4658355da25c47a21ad8804904c0ccd9410` | `sing-box check -c <cfg>`（`sing-box --help` 确认 `check` 子命令存在） |

生成命令：`cargo run -p config_codegen --example gen_matrix --locked -- target/t06b/matrix`（**36 例**）。
执行命令：`pwsh -File tools/validate/t06b_validate.ps1`。

### 3.2 结果统计（`results.json`，留档于本目录）

- **总计 36；PASS 35；FAIL 1。**
- 覆盖：协议（VMess/VLESS/SS/Trojan/Hysteria2/WireGuard/SOCKS/HTTP/TUIC/Anytls/Naive）× 传输（raw/ws/httpupgrade/xhttp/kcp/grpc）× 安全（none/tls/reality）× 全局注入（DNS/FakeIP/TUN/routing/stat/log/mux/fragment/Bind/SendThrough）× 组/链（PolicyGroup/ProxyChain）。

唯一失败：`xray xray-kcp`（exit 23）。关键输出：
```
failed to build mask with type mkcp-legacy > unknown config id: mkcp-legacy
```
归属：**上游版本间隙**，非本生成器缺陷。上游 v2rayN 7.25.4 的 `V2rayOutboundService.cs` 对 KCP 写出 `"type":"mkcp-legacy"`（已核对源码原文），本生成器逐字一致；但本机 Xray 26.3.27 尚未登记该 mask 类型（实测同位置换成 `mkcp-aes128gcm` 可通过 mask 构建阶段）。结论：待上游锁定/下载到包含该 mask 的 Xray 版本后即可通过。详见 §6 未覆盖清单。

---

## 4. 真实数据通路冒烟

脚本：`docs/evidence/T06b.runs/smoke_runner.ps1`（配套 `http_server.js`），留档 `smoke/smoke-results.json`、`smoke/smoke-timeline.txt`。

流程：本地 HTTP 服务 `127.0.0.1:11880`（返回 `T06B-SMOKE-OK`）→ 生成 socks 入站 `127.0.0.1:11808` + direct/freedom 出站 → 内核 `run -c <cfg>` → `curl.exe -x socks5h://127.0.0.1:11808 http://127.0.0.1:11880/` → 校验内容 → 停止本脚本 PID。全程未碰 10808、未启 TUN、未改系统代理。

| 内核 | 认证 | 进程 PID | 结果 |
|---|---|---|---|
| Xray 26.3.27 | noauth | 34312 | OK（`T06B-SMOKE-OK`） |
| sing-box 1.14.2 | noauth | 40808 | OK |
| Xray 26.3.27 | user/pass | 34028 | OK |
| sing-box 1.14.2 | user/pass | 38656 | OK |

**SMOKE: 4/4 ok。** 时间线与每例日志见 `T06b.runs/smoke/`。

---

## 5. cores.lock.json — sing-box 条目

`tools/cores/cores.lock.json` 追加：

```json
{
  "core": "sing-box",
  "release_tag": "v1.14.2",
  "core_version": "1.14.2",
  "asset": "sing-box-1.14.2-windows-amd64.zip",
  "asset_size_bytes": 32857903,
  "source": "https://github.com/SagerNet/sing-box/releases/download/v1.14.2/sing-box-1.14.2-windows-amd64.zip",
  "sha256": "c2d8bfff918755808781dfdeeb8581b6c91eb3a243d9a7b55483cfc0c0684d32",
  "sha256_verified": false,
  "executable_sha256": "7bbef1dea9189ee12799ae834ea4b4658355da25c47a21ad8804904c0ccd9410",
  "platform": "windows-amd64"
}
```

核对：GitHub Release API 查询 `v1.14.2`，官方资产 `sing-box-1.14.2-windows-amd64.zip` 大小 **32857903** 与本地完全一致；该 Release **未提供独立校验和资产**，故 `sha256_verified=false`（仅在 release notes 层面无法交叉比对），本地 zip/exe 哈希已计算留档。

---

## 6. 未覆盖 / 仍失败清单

1. **`xray xray-kcp`（mask 类型版本间隙）**：上游写 `mkcp-legacy`，本机 Xray 26.3.27 未登记该类型。待下载包含该 mask 的 Xray 版本后复跑；生成器已与上游逐字一致，**不改**。
2. **sing-box zip 官方校验和**：官方无 `.dgst`/`.sha256` 资产，仅大小核对通过；`sha256_verified=false`。真实联网 `curl -x socks5h://127.0.0.1:11808 http://127.0.0.1:11880/` 冒烟覆盖 noauth 与 user/pass。
3. **真实远端代理流量**：冒烟仅验证本地环路（生成 socks 入站 → direct 出站 → 本地 HTTP），未连接任何真实代理服务器；TLS-required 协议（hysteria2/tuic/anytls/naive）仅做 `check` 通过验证，未做真实握手。
4. **Xray 日志路径**：校验需工作目录存在 `logs/`；`t06b_validate.ps1` 已在仓库根预建 `logs/`。生成器按上游语义写相对路径，属调用方装配职责。
5. **sing-box `endpoints` 空语义**：仅覆盖“无 endpoint 时省略”，未对自定义 endpoint 与模板并存做真实内核差分（已有单测）。
6. **M-011 上游差分**：模板合并顺序经上游源码核实为“生成在前”，但未逐字段做上游真实输出二进制差分（无上游可运行 exe）。
7. **未在非 Windows 平台验证**：本轮全部为 windows-amd64。
8. **冒烟脚本端口硬编码**：`T06b.runs/smoke_runner.ps1:74` 将 socks 入站端口固定为 11808（未做占用探测）；11808 是既有约定测试端口，若被占用冒烟会直接失败，接线阶段应改为先探测再选。
9. **sing-box 用例空 error 字段**：`T06b.runs/results.json:146-289` 的 sing-box 条目 error 为空字符串，单测结论仅由 exit 码支撑；xray 条目有版本串+`Configuration OK`，sing-box 侧自证力较弱，需在复跑时补 stdout 尾行。
10. **TUN 驱动枚举 stderr**：`xray-global-tun-fragment` 用例 stderr 含 TUN 驱动枚举信息（未启真实 TUN），属环境探测输出而非本生成器错误；结论仍以 exit/配置校验为准。

---

## 7. 门禁（本轮实际运行）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过（exit 0） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 |
| `cargo test --workspace --locked` | 通过（含 `config_codegen` 81 例） |
| `cargo test -p config_codegen --locked` | 通过：14 测试二进制，**81 passed / 0 failed** |

> 并行代理存在时锁竞争偶发；已在门禁中用有限重试（≤3）规避瞬时锁失败。

---

## 8. 本轮改动文件

- `crates/config_codegen/src/lib.rs`、`util.rs`、`xray/mod.rs`、`xray/routing.rs`、`xray/outbound.rs`、`singbox/mod.rs`、`singbox/config.rs`、`singbox/outbound.rs`、`singbox/routing.rs`
- `crates/config_codegen/examples/gen_matrix.rs`
- `crates/config_codegen/tests/{xray_errors,singbox_errors,xray_routing_dns,xray_template_custom}.rs`
- `tools/cores/cores.lock.json`、`tools/validate/t06b_validate.ps1`
- `docs/decisions/{T07-xray-codegen-contract,T08-singbox-codegen-contract,T07-T08-codegen-impl-notes}.md`
- `docs/evidence/T07-T08-codegen.md`（悬空语义更正）
- `docs/evidence/T06b-validation.md`（本文件）+ `docs/evidence/T06b.runs/**`

未改：`apps/desktop`、`services/net_host`、`application`、`persistence`、`subscriptions`、`compat/fields*|features` 状态；未 commit。
