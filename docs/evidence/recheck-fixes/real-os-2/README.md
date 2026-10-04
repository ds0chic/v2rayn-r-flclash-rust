# 真机隔离复验（当前代码，2026-10-04）

按此前授权的 T21-D 隔离实测范围，在当前 HEAD（Wave J `baf2299`）上复跑四项真机测试。全程未触碰 10808；系统代理/注册表/路由均写读复原；TUN 只创建/销毁测试适配器，不改默认路由。

| 项 | 命令 | 结果 |
|---|---|---|
| 系统代理写-读-复原 | `cargo test -p platform --test real_windows -- --ignored --nocapture --test-threads=1` | 2 passed；before `enabled=false` → during `enabled=true 127.0.0.1:11808` → after `enabled=false`（基线复原） |
| 自启动写-读-删 | 同上（第二个 ignored 测试） | passed；HKCU Run 值写入/读回/删除，无残留 |
| 提权 helper 路由增删 | `cargo test -p privileged_helper --test real_windows -- --ignored --nocapture` | 1 passed；`198.51.100.0/24` 添加后 present=1、删除后 present=0 |
| TUN 适配器（安全范围） | `sing-box 1.14.2 run -c tun-test-config.json`（`auto_route=false`，适配器 `v2rayn-r-test-tun`） | 适配器 Up、ifIndex=77；默认路由全程 `192.168.1.1 metric 10 if=14` 不变；停止后适配器移除、无 stray 进程 |

日志：`platform-real.{out,err}`、`helper-real.{out,err}`、`tun-test.out`、`tun-stderr.log`、`tun-test-config.json`。

边界：生产 TUN 全链路（net-host+helper+auto route）仍未在真机执行（会改默认路由，需隔离 VM）；真实 OS 全局热键派发、真实发行源自更新未在此轮执行。
