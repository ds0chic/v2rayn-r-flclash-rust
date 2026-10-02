# T21-A — KCP finalmask 兼容修复 + 跨目标编译侦察

- 任务：T21-A（修复 `xray-kcp` finalmask 兼容，使 T06b 真实内核矩阵 36/36；跨目标编译侦察；平台矩阵 §6 回填）
- 承办：T21-A 子代理（deepseek-v4.1-flash）
- 时间：2026-10-02
- 允许写清单：`crates/config_codegen/**`、`docs/evidence/T21-kcp-cross.md`、`docs/evidence/T06b-validation.md`（追加）、`compat/platform-matrix.md` §6（追加）、`docs/evidence/T06b.runs/**`（追加）。未 commit。
- 内核：Xray `26.3.27`（go1.26.1, d2758a0，`tools/cores/xray/v26.3.27/xray.exe`）；sing-box `1.14.2`。

> 原则：不把“上游逐字一致”当“内核可运行”；所有内核结论均来自真实 `xray.exe run -test`，脚本与结果留档于 `docs/evidence/T06b.runs/t21-2026-10-02/probe/`。

---

## 1. 事实核查

### 1.1 上游冻结源码确认（无版本条件）

`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Services/CoreConfig/V2ray/V2rayOutboundService.cs:379-421`：

```csharp
case nameof(ETransport.kcp):
    var kcpFinalmask = new Finalmask4Ray();
    if (Global.KcpHeaderMaskMap.TryGetValue(headerType, out var header))
        kcpFinalmask.udp = [ new Mask4Ray { type = "mkcp-legacy",
                                           settings = new MaskSettings4Ray { header = header } } ];
    kcpFinalmask.udp ??= [];
    if (kcpSeed.IsNullOrEmpty())
        kcpFinalmask.udp.Add(new Mask4Ray { type = "mkcp-legacy" });
    else
        kcpFinalmask.udp.Add(new Mask4Ray { type = "mkcp-legacy",
                                            settings = new MaskSettings4Ray { value = kcpSeed } });
    kcpFinalmask.udp?.Reverse();
    streamSettings.finalmask = kcpFinalmask;
```

- 结论：上游 7.25.4 **无条件**写 `type: "mkcp-legacy"`，不存在内核版本判断。`Global.KcpHeaderMaskMap`（`Global.cs:350-358`）映射为
  `srtp→srtp, utp→utp, wechat-video→wechat, dtls→dtls, wireguard→wireguard, dns→dns`。
- 代码生成器修复前（`xray/outbound.rs:715-729`）与上游**逐字一致**，因此这是上游夹具与锁定内核之间的**版本间隙**，不是生成器逻辑错误。

### 1.2 Xray 26.3.27 接受矩阵（真实 `xray run -test`，端口 11890-11892）

脚本：`docs/evidence/T06b.runs/t21-2026-10-02/probe/kcp_probe.ps1`、`kcp_probe2.ps1`、`kcp_probe3.ps1`（每个变体一个最小 config，`xray run -test`，结果 JSON 见同目录 `probe*-results.json`）。

| 变体 | `finalmask.udp[].type` | 退出码 | 结论 |
|---|---|---|---|
| A | `mkcp-legacy`（header+value，上游原样） | **23** | `unknown config id: mkcp-legacy` |
| J/P13 | `mkcp` / `mkcp-legacy`（空 settings） | 23 | 未知 id |
| P1-P6 | `wechat`/`srtp`/`utp`/`dtls`/`wireguard`/`dns`（裸名） | 23 | 未知 id，需 `header-` 前缀 |
| P7/P8 | `header-wechat` / `header-srtp` | 0 | 接受 |
| B-F | `mkcp-aes128gcm` / `mkcp-original`（header/value/空） | 0 | 接受 |
| K/L | `finalmask.udp: []` / 省略 finalmask | 0 | 接受 |
| Q1-Q10 | `aes128gcm.settings.password` + `header-*` 组合 | 0 | 接受 |

二进制串定位（`xray.exe` ASCII）：存在 `mkcp-aes128gcm`、`mkcp-original`、`header-*` 相关符号；不存在 `mkcp-legacy`。

### 1.3 语义等价来源（Xray 上游 commit）

`mkcp-legacy` 由 Xray commit `aba22722a65d31c4c17ef5d9619e75a9ca474ce7`（"Finalmask: Add mkcp-legacy (UDP) to replace mkcp-* and legacy header-*", PR #6201, 2026-05-29）引入；26.3.27（2026-03-27）早于该提交。该提交的 `MkcpLegacy.Build` 定义了精确映射：

```go
type MkcpLegacy struct { Header string `json:"header"`; Value string `json:"value"` }
func (c *MkcpLegacy) Build() (proto.Message, error) {
    if len(c.Header) == 0 {
        if len(c.Value) == 0 { return &original.Config{}, nil }
        return &aes128gcm.Config{Password: c.Value}, nil
    }
    switch strings.ToLower(c.Header) {
    case "dns":      // domain = value or "www.baidu.com"
    case "dtls":     // header.Config{ID:1}
    case "srtp":     // ID:2
    case "utp":      // ID:3
    case "wechat":   // ID:4
    case "wireguard":// ID:5
    }
}
```

即 26.3.27 的等价形式：无 header/无 value → `mkcp-original`；无 header/有 value → `mkcp-aes128gcm(password=value)`；有 header → `header-<name>`（`dns` 可选 `domain`）。**所有上游语义均可无损表达**。

---

## 2. 最小修复

不改变上游“header 掩码 + 种子掩码两项并 Reverse”的链式结构，只把每项的 `type`/`settings` 翻译成 26.3.27 已登记的 legacy finalmask：

| 上游项 | 修复后项 |
|---|---|
| `{"type":"mkcp-legacy","settings":{"header":"<h>"}}` | `{"type":"header-<h>"}`（`dns` 无 settings，内核默认 `www.baidu.com`，与上游不设 value 一致） |
| `{"type":"mkcp-legacy","settings":{"value":"<seed>"}}` | `{"type":"mkcp-aes128gcm","settings":{"password":"<seed>"}}` |
| `{"type":"mkcp-legacy"}`（空） | `{"type":"mkcp-original"}` |

改动文件：

- `crates/config_codegen/src/util.rs`：`KCP_HEADER_MASK_MAP` → `KCP_HEADER_FINALMASK_MAP`，值改为 `header-srtp`/`header-utp`/`header-wechat`/`header-dtls`/`header-wireguard`/`header-dns`。
- `crates/config_codegen/src/xray/outbound.rs`：KCP 分支按上表生成；新增 Xray 侧 thread-local 诊断汇（镜像 sing-box 既有模式）`diagnostic_sink_reset/take/push_diagnostic`；每次 KCP finalmask 生成追加结构化 **Warning** `xray_kcp_finalmask_translated`（字段路径 `streamSettings.finalmask.udp`），明确“锁定内核未登记上游 `mkcp-legacy`，已按 `MkcpLegacy.Build` 无损翻译”，**不静默降级**。
- `crates/config_codegen/src/xray/config.rs`：`build()` 起始 `diagnostic_sink_reset()`，出站构建后 `state.diagnostics.extend(diagnostic_sink_take())`。
- `crates/config_codegen/tests/xray_transport_security.rs`：更新 `xray_kcp_header_and_seed_order`（断言新 type + `settings.password` + Warning），新增 `xray_kcp_finalmask_legacy_translation`（header-only / seed-only / neither 三种边界）与 `xray_kcp_dns_header_maps_to_header_dns`。

`xray--xray-kcp.json` 修复后 `finalmask`：

```json
{"udp":[{"type":"mkcp-aes128gcm","settings":{"password":"seed"}},{"type":"header-wechat"}]}
```

> 注：`docs/decisions/T07-xray-codegen-contract.md` D5 与 `compat/codegen-map.xray.yaml` 仍描述 `mkcp-legacy`，属本次修复后的**契约文档漂移**；两者不在 T21-A 允许写清单内，未改，登记于本文 §5 待办。

---

## 3. 真实内核矩阵结果（36/36 PASS）

命令：

```
cargo run -p config_codegen --example gen_matrix --locked -- target/t06b/matrix
pwsh -NoProfile -ExecutionPolicy Bypass -File tools/validate/t06b_validate.ps1
```

结果：**T06b matrix: 36 pass, 0 fail**（前值 35/36，唯一失败 `xray xray-kcp` 现 `exit 0`）。

证据（追加归档）：`docs/evidence/T06b.runs/t21-2026-10-02/`
- `results.json`（36 条，`xray-kcp` status=PASS exit=0）
- `manifest.json`
- `logs/`（36 例内核 stdout/stderr）
- `xray--xray-kcp.json`（修复后配置）

---

## 4. 跨目标编译侦察（只编译，不运行）

### 4.1 Windows ARM64（Rust 侧）

```
rustup target add aarch64-pc-windows-msvc        # 成功安装 rust-std
cargo check -p <pkg> --target aarch64-pc-windows-msvc --locked
```

| 包 | 结果 | 原因 |
|---|---|---|
| `domain` | ✅ exit 0 | — |
| `config_codegen` | ✅ exit 0 | 纯 Rust（serde/serde_json） |
| `ipc_contract` | ✅ exit 0 | — |
| `subscriptions` | ❌ exit 101 | 传递依赖 `ring 0.17.14`（`reqwest 0.12.28 → rustls 0.23.38 → ring`）build script：`failed to find tool "clang": program not found` |
| `updater` | ❌ exit 101 | 同上 `ring 0.17.14`（`reqwest → rustls → ring`） |

- 根因：`ring` 的 C/汇编构建在 `aarch64-pc-windows-msvc` 交叉时由 `cc-rs` 选择 `clang`；本机未安装 clang，VS 2022 也未安装 ARM64 C++ 工具链。
- 解除前置：安装 LLVM/clang（含 aarch64-windows 目标）或 VS 2022 组件 “MSVC v143 - VS 2022 C++ ARM64 build tools”，并设置 `CC_aarch64_pc_windows_msvc` / `AR_aarch64_pc_windows_msvc`（或使用 `cargo-xwin`）。
- 日志：`docs/evidence/T06b.runs/t21-2026-10-02/cross/*.log`（见 §5 说明）。

### 4.2 Linux / macOS（本机 blocked）

| 目标 | 现状 | 前置条件 |
|---|---|---|
| Linux x64/ARM64 | `blocked`：`wsl --status` 报“未安装 WSL 分发”；`docker` 命令不存在；`gpg` 命令不存在 | ① `wsl --install`（管理员，重启）或安装 Docker Desktop（WSL2 后端）② 发行版内 `rustup target add x86_64-unknown-linux-gnu`/`aarch64-unknown-linux-gnu` 并装 `build-essential`/`pkg-config` ③ 交叉还需 `gcc-aarch64-linux-gnu`（或直接 ARM64 主机） |
| macOS x64/ARM64 | `blocked`：需 Apple 硬件 + Xcode/SDK（`xcode-select --install`）；Windows 无法产出 macOS 产物 | Apple Silicon/Intel Mac + Xcode（或 CI macOS runner） |
| Flutter Windows ARM64 | `blocked`（维持 T20 结论）：`flutter build windows` CLI **无** `--target-platform`（仅 Android/iOS 支持）；且本地 x64 Flutter 引擎不含 ARM64 目标，需 ARM64 主机与引擎 | Windows ARM64 主机 + 支持 ARM64 的 Flutter 引擎 |

---

## 5. 未覆盖 / 待办

1. **契约文档漂移**：`docs/decisions/T07-xray-codegen-contract.md` D5、`compat/codegen-map.xray.yaml:265`、`compat/fields*.yaml` 仍写 `mkcp-legacy`；不在 T21-A 允许写清单内，未改。应由文档/台账负责人在 T21 汇总时同步为 `mkcp-original`/`mkcp-aes128gcm`/`header-*`。
2. **跨目标日志归档**：`cargo check` 原始日志置于 `docs/evidence/T06b.runs/t21-2026-10-02/cross/`（源：临时目录 `t21-cross/*.log`）。
3. **Windows ARM64 全量 check** 未通过，仅按包记录；未做 ARM64 真实运行（无 ARM64 主机）。
4. **macOS/Linux 未做任何构建**（无对应宿主/工具链），状态与前置见 §4.2。
5. **`ring` 交叉工具链**：若需在 T21 内推进 ARM64，需先装 clang 或 VS ARM64 工具链；本次仅侦察。

---

## 6. 门禁结果（本轮实际运行）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | ❌ 失败，但 diff 全部位于**并行代理的未提交文件**（`crates/application/src/lib.rs`、`crates/updater/{src/*.rs,examples/t21_verify_dgst.rs}`、`services/upgrade_runner/src/main.rs`）；`cargo fmt -p config_codegen -- --check` = ✅ exit 0 |
| `cargo clippy -p config_codegen --all-targets --locked -- -D warnings` | ✅ exit 0 |
| `cargo test -p config_codegen --locked` | ✅ 14 个测试二进制，**83 passed / 0 failed**（T06b 基线 81，本轮 KCP 用例 +2 净增） |

> `Cargo.lock` 在并行代理向 `crates/updater/Cargo.toml` 加入 `pgp = "=0.16.0"` 后一度失配，导致 `--locked` 首次失败；本轮以一次联网 `cargo run` 刷新共享锁文件（含 `pgp`）后恢复，未手动编辑 `Cargo.lock`，未 commit。
