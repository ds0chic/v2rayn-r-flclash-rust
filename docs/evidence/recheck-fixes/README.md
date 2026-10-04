# 第二轮复核修复记录（2026-10-04）

基线：`a2b6905`（复核报告 `docs/evidence/parity-recheck-2026-10-04/`）→ 本轮 HEAD 见 `git log`。
每波均通过完整门禁（cargo fmt/clippy/test workspace；flutter format/analyze/逐文件测试/release 构建）后提交。

## 已修复（按复核编号）

| 编号 | 修复内容 | 提交 |
|---|---|---|
| RE-PROF-01 | 新增/粘贴/扫码继承当前分组（命令时快照，异步不漂移） | eeb2932 |
| RE-PROF-05 | 换组/过滤后选择与可见集求交，隐藏节点不再成为动作目标 | eeb2932 |
| RE-PROF-02 / R-01 | 编辑/删除活动节点触发共享 Reload 与删除回退；F5/菜单重载接共享用例 | eeb2932 |
| RR-01 | 下载与运行共用 engine cores 根（子进程显式传递），真实 spawn 验证 | eeb2932 |
| SR-01 | 订阅逐组真实结果（成功/失败/跳过/空URL），不再由 Done 推断全部成功 | eeb2932 |
| RR-02 / RR-03 | 托盘/状态栏切路由=设默认并生效；代理/PAC 三入口统一、启动恢复保存模式 | e9b8e74 |
| SR-02 / SR-04 | 原版 ZIP 资源配置安装（哈希校验）；提交后失败两阶段回滚 | e9b8e74 |
| RE-PROF-03 | Ctrl+C=导出分享链接、Ctrl+F=分享窗口（对齐上游） | e9b8e74 |
| SR-03 | 恢复生命周期：停运行/调度→quiesce→交换→reopen→全 provider 刷新→按活动恢复 | 472ca54 |
| RE-PROF-04 | 排序按持久化 Sort 读回、同列两向、写失败反馈 | 472ca54 |
| RR-04 / RR-11 | runner 入包(v2rayN-upgrade.exe)+平铺覆盖布局+CLI 对齐+Dart 启动与退出交接；Inno 卸载保留用户文件 | 472ca54 / 256c3dd |
| RE-PROF-06 | 去重/移除无效按整组（不叠文字过滤），失败保留证据、其它组不受影响 | 256c3dd |
| RE-PROF-09 | 节点表四统计列接 ServerStatItem（今日/累计） | 256c3dd |
| RE-PROF-07 | 逐协议后端归一/校验（TUIC 强制 sing-box、清字段、TLS/ALPN/拥塞默认等）；Mux/Finalmask/UOT 控件 | 842520b |
| RE-PROF-10 | 策略组预览解析当前草稿（取消无副作用、新建可预览） | 842520b |
| SR-05 / SR-06 | 热键编辑暂停旧绑定；同组合注册一次分发多动作 | 842520b |
| RR-07 / RR-10 | Custom 真实端点解析（协议/端口）；切换先预检、失败保留/恢复上一良好会话 | cbf4fa7 |
| RE-PROF-08 | 完整客户端配置两导出（文件/剪贴板）；UDP 测速按支持矩阵启用 | cbf4fa7 |
| RE-PROF-13 / RE-PROF-14 | 活动行独立标记；自动列宽实现、拖动开关消费、搜索 Enter/清空语义 | cbf4fa7 |
| RR-05 / RR-06 | TUN 接口发现+runas helper 授权/取消+预 SOCKS 真配置与多进程执行+12 核 adapter | 6a16271 |
| RR-08 / RE-PROF-11 | 托盘 read model 持续同步；选择器恢复（分组/搜索/列/排序/类型约束/批量移除） | 6a16271 |
| RE-PROF-12 / RR-MON-REBIND | 证书 SNI 握手、叶子如实提示、非法 base64 保护；监控仅会话变化重绑、store 错误可见可重试 | 6a16271 |

真实窗口回归：`apps/desktop/integration_test/parity_recheck_group_selection_test.dart` 在隔离数据目录下 exit 0（原先两项失败断言转绿，证据 `windows-ui-02/`）。

## 仍未完成 / blocked（诚实登记）

- RR-05/06：真实 TUN 设备创建/路由下发、真实 UAC 提权、逐核完整参数、真实双进程预 SOCKS 整链（需授权隔离主机）。
- RR-04：应用自身发行源未配置（`app_repo=None`），真实远端更新端到端 blocked。
- RR-09：隐藏时刷新门控/连接列状态/“今日”范围仍待收尾（部分已由 16C 系列覆盖）。
- RE-PROF-11：选择器延迟/速度列需接 ProfileEx 叠加层（同 RE-PROF-09 源）。
- 监控风险 #2：current-thread Tokio + 阻塞 sleep 的 sing-box WS pump 驱动节奏未实测。
- R-02：向上拖选测试的 flutter_tester 原生退出未根因化（产品侧已由真实窗口测试覆盖选择行为）。
- 全部卡未做原版实机双窗口逐事件对照，保持 `implemented`。

安全边界：全程未占用 10808、未改宿主系统代理/注册表/路由/TUN、未读用户凭据；系统写读仅经授权路径并复原。
