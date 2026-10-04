# 第三轮 P1 修复验证（用户复核项）

日期：2026-10-04。对应用户复核消息中列出的每项，在生产入口重跑验证。
基线：`3fc49c4`（wave H=`593e289`、wave I=`d48e69e` 已合入）。

| 复核项 | 验证 | 结果 |
|---|---|---|
| PAC 规则格式 | `flutter test test/r3_01_07_10_test.dart`（断言 `PROXY/SOCKS5 host:port;DIRECT;` 与协议区分） | 9/9 通过 |
| 预 SOCKS 启动顺序 | `cargo test -p net_host rr06_apply_tracks_sidecar_after_the_core_is_ready --locked` | 1 passed（主核 ready 后才启动侧车） |
| 备份 A→B→A | `cargo test -p application --lib merge_import_a_b_a_is_idempotent_and_active_stays_resolvable --locked` | 1 passed（第三次导入 no-op、active 在库中存在） |
| 重复导入幂等 | `cargo test -p persistence --lib reimport_of_earlier_source_is_noop_and_keeps_scoped_config --locked` | 1 passed（保留该来源批次配置） |
| 恢复后热键重载 | `flutter test test/r3_set_restore_test.dart` | 1/1 通过（K→恢复 J→立即 J→取消仍 J） |
| UDP 经节点 | `cargo test -p bridge_api --lib udp_ping_routes_through_the_session_port --locked` | 1 passed（走 session.port 的 SOCKS5 UDP ASSOCIATE） |
| 右键菜单目标不漂移 | `recheck_r3_prof_menu_target_{a,b,c}_test.dart`（多选主行 / 导出捕获目标 / 隐藏拒绝） | 各 1/1 通过（原合并文件命中 flutter_tester 原生崩溃，按仓库惯例拆分） |
| 发布包干净构建 | `dist/build-info.json` 与 ZIP 内 `build-info.json` | commit `3fc49c4`、`git_dirty=false`、sha256 `6cdd438f…`（旧 `ffe8e0a4` 包 dirty=true 已作废） |

日志：本目录 `aba.log`、`reimport.log`、`presocks_order.log`、`udp_session.log`、`r3_01_07_10_test.dart.log`、`r3_set_restore_test.dart.log`、`recheck_r3_prof_menu_target_*.dart.log`。

仍未完成（与本轮复核项无关，保持登记）：R3-04 首次 TUN 设备创建（隔离 VM）、R3-09 托盘图标状态/“今日”范围、R3-PROF-08/10/11、部分 preservedOnly 菜单入口、真实发行源/OS 热键/系统代理整链。
