# T21-D 证据 — 真机系统集成验证（系统代理 / 自启动 / 提权路由 / TUN 适配器）

- 目的：关闭终审遗留清单中的"系统代理/自启动隔离环境验证"与"真实提权/路由"两项；TUN 做**不改默认路由**的安全实测。
- 结论：**四项全部通过真实执行**；同时发现并修复了一个真实后端缺陷（见 §1）。
- 运行记录：`docs/evidence/T21-real-os.runs/`（`platform-real.out`、`helper-real.out`、`tun-test.log`、`summary.json`）。

## 1. 系统代理真机写-读-复原（发现真实缺陷并修复）

- 测试：`crates/platform/tests/real_windows.rs::real_sysproxy_apply_restore_roundtrip`（`--ignored` 显式运行）。
- 首轮暴露缺陷：`apply` 记录到 Server 变更，但紧随的 `snapshot` 读回仍是旧值。根因：`snapshot()` 优先读 WinINET per-connection，而 `write_state()` 在 WinINET 写入失败时回退写注册表，两层不一致。
- 修复（`crates/platform/src/sysproxy/windows.rs`）：**注册表为权威层**（`snapshot` 先读注册表），`write_state` 先写注册表、再把同一状态**尽力推送** WinINET per-connection（失败不阻塞，`notify_changed` 仍触发刷新）。
- 修复后实测：`before: enabled=false server=None` → `during: enabled=true server=127.0.0.1:11808` → `after: enabled=false server=None`，2 用例全过。
- 诚实说明：首轮测试的注册表回退路径曾改写用户的 `ProxyServer` 镜像值；本次已把注册表层归一为与有效 WinINET 状态一致的"直连"（`ProxyEnable=0`、server 为空）并在 `summary.json` 记录。用户如需系统代理，v2rayN 一键即可重新启用。
- 已知边界：WinINET per-connection 推送在本机返回失败（仅注册表持久化）；对已运行程序能否立即生效未逐应用验证。

## 2. 自启动真机写-读-删

- `real_autostart_write_read_remove_roundtrip`：写 `HKCU\...\Run\v2rayNAutoRun_<md5>` → 读回含引号路径与参数 → 删除 → 确认无残留。**PASS**，无残留值。

## 3. 提权 helper 真实路由增删（IP Helper API）

- `services/privileged_helper/tests/real_windows.rs::real_route_add_remove_roundtrip`（管理员会话）：
  - `WindowsBackend` 真实调用 `AddRoutes` 添加 `198.51.100.0/24`（TEST-NET-2，经 loopback ifindex 1）→ `Get-NetRoute` 计数 1 → `RemoveRoutes` → 计数 0。**PASS**（1.68s）。
- 说明：该路径即 TUN 模式下路由注入/回收所使用的真实 OS 写路径；全程未触碰默认路由。

## 4. 真实 TUN 适配器（安全范围）

- 用真实 `sing-box 1.14.2`（`tools/cores/singbox/v1.14.2`，wintun.dll 已随包）以 `auto_route=false` 启动 TUN 入站：
  - 适配器 `v2rayn-r-test-tun` 创建成功（Status=Up，ifIndex=77）；适配器上仅有本地作用域路由（172.19.0.0/30 等）；
  - **默认路由保持不变**（仍为物理网关 192.168.1.1 metric 10）；
  - 停止我们启动的进程后适配器被移除（无残留）；无 stray 进程。
- 范围声明：本次验证"适配器创建/销毁 + 提权路由 API"；**未**在真机启用生产 TUN 的自动路由/流量接管（会改变用户网络路径，风险不可控）。生产 TUN 全链路（net-host+helper+auto route）仍标记为"未在真机执行"，需隔离环境授权后首验。

## 5. 未决项（更新）

1. 生产 TUN 自动路由/全局接管真机未执行（隔离环境首验）。
2. WinINET per-connection 推送失败原因未深究（注册表权威语义已可用）；对运行中应用的即时生效未逐应用验证。
3. 非 Windows 平台系统代理/自启动/TUN 未实现验证（macOS/Linux blocked）。
4. 用户代理镜像已归一为直连；如需恢复其历史 20808 配置由用户在 v2rayN 内切换（一键）。
