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

## 第三轮（round3-*，2026-10-04）

Wave H（提交 `593e289`）：
- R3-SET-01：AlreadyImported 幂等 no-op + 按批次配置，杜绝 A→B→A 跨源激活/悬空 active。
- R3-SET-02：原版 ZIP 恢复改为替换 DB/config（合并导入保留独立入口）。
- R3-SET-03：恢复前取消/排空在途订阅，epoch 保护拒绝换库后旧提交，停止超时阻断恢复。
- R3-SET-04：恢复后重载并重注册 GlobalHotkeys。
- R3-02：预 SOCKS 顺序改为“主核→等待端口→侧车”，侧车配置消费真实设置（入站本地端口、出站拨主核、TUN/DNS/路由）。
- R3-03：非 Xray/sing-box 原生 Custom 配置逐字保留（不再被 Xray 端点解析拒绝），补 mieru env 等合同。
- R3-05：helper 成功后所有失败分支统一清理 TUN lease/journal。
- R3-07：Custom API secret/listen/类型进入 AppliedFacts/MonitorSession；无 API 不访问默认统计端点。
- R3-PROF-01：多选主行合同（编辑/分享/设活动/完整导出用 primary，不再要求单选中）。
- R3-PROF-02：菜单命令目标不可变（漂移/隐藏对象拒绝；完整导出读捕获目标）。
- R3-PROF-03：失败/未测延迟两向沉底；结果排序后 reload 不回退；整组写 Sort（含过滤隐藏行）。
- R3-PROF-04：去重读 KeepOlderDedupl、删除含活动后回退、失败显示真实错误。

Wave I（提交 `d48e69e`）：
- R3-01：PAC 渲染 `PROXY/SOCKS5 host:port;DIRECT;` 指令串，与 WinINET 服务器串分离。
- R3-PROXY-UI：系统代理/PAC 按 AppliedSession 协议（SOCKS/HTTP）区分。
- R3-10：路由重载提示以新 applied 事实为准，失败呈现具体错误。
- R3-06：sing-box 统计 WS 后台任务置于持续 executor，生产 poll loop 合成非零验证。
- R3-PROF-05：UDP 测速经节点 session 的 SOCKS5 UDP ASSOCIATE，不再宿主直连冒充。
- R3-PROF-07：订阅策略组只保留有效叶子（Rust `is_valid` + Dart 同步）。
- R3-08：普通更新检查对 App 分流到本产品源；未配置时明确 blocked，不展示原版发行信息。
- R3-PROF-06：Custom 原始文本导出字节不变；扩展名按 address；缺 fileName 显式失败。
- R3-PROF-09：证书获取连接节点 Address:Port + `SecureSocket.secure(host: SNI)`。
- R3-SET-05：热键部分冲突后保持暂停，编辑结束前不恢复动作。
- R3-SET-06：主菜单 toast 复用逐组汇总，失败可查看详情。
- R3-ROOT-01：F5/重载重入补跑最后一次；空活动提示。
- 台账：ACT-MAIN-035 状态/实现定位更新（R3-02 root 漂移）。

第三轮仍未完成（登记）：R3-04 首次 TUN 设备创建（隔离 VM）；R3-09 托盘图标状态/“今日”范围；R3-PROF-08 选择器延迟/速度列与键盘合同；R3-PROF-10 默认选中/Esc 语义；R3-PROF-11 Reality 指纹保存固化；R3-02/03 root（台账其余漂移、管理员重启/UWP 回环/区域预置/核心网站等 preservedOnly 入口）；真实发行源与真实 OS 热键/TUN/系统代理整链。

## 第三轮收尾（Wave J/K + 真机复验，2026-10-04）

Wave J（提交 `baf2299`）：
- R3-PROF-08/10：选择器接 ProfileEx（延迟/速度/订阅备注）、默认当前组、Ctrl+A/Enter/双击/Esc、搜索仅 remarks/address、隐藏项不确认；主表默认选中 pending→活动→第一行、Esc 保留选择、Shift/方向键从主行扩展。
- R3-PROF-11：Reality 空指纹按保存时 `DefFingerprint` 固化（改默认不影响旧节点）。
- R3-09：托盘四状态图标映射；状态栏“今日”改用节点 today 聚合。
- R3-ROOT-03：管理员重启、UWP 回环命令、区域预置（默认/俄罗斯/伊朗）、核心网站入口全部去 `preservedOnly` 并接线。
- R3-04：TUN 延迟发现代码路径（核心创建接口→轮询发现→helper 配置→失败统一清理）；真实设备创建仍待隔离 VM。

Wave K（提交 `ff45519`，基于原版 WPF 三窗口对照）：
- 设置窗口：五页标题/分组/按钮（确定/取消）对齐上游、清除裸 key、补认证用户名/密码与默认 TLS 指纹、KCP 标历史保留。
- 路由窗口：顶部添加规则集/一键导入、区块标题、列头 别名/数量/排序/可选地址 (Url)/自定义图标、底部按钮集。
- 主窗口：行头去掉 `#`（40px 无标题，对齐 RowHeaderWidth）、状态栏 本地/局域网/启用 Tun 文案、根菜单分隔线渲染、`重载`→`重启服务`；工具栏额外按钮与深色主题登记为有意增强。

真机隔离复验（`real-os-2/`，当前代码）：系统代理写读复原 2/2、自启动写读删、提权路由增删、TUN 适配器创建/销毁且默认路由不变。
真实 OS 热键（`SR-REAL-HOTKEY/`）：Ctrl+Alt+F12 注册→SendInput→回调→注销全链通过，OS 组合测试后空闲。
原版 WPF 对照（`R3-WPF-COMPARE/`）：原版成功构建启动，三窗口截图与差异清单落盘。

仍未完成（诚实登记）：设置/路由窗口形态（内嵌 dialog vs 独立窗口，卡 `R3-WPF-WINDOW-FORM`）；路由“一键导入规则集”后端用例与多选；生产 TUN 全链路（隔离 VM）；12 个未锁定内核的下载与真实运行矩阵；真实发行源自更新（产品决策）；同组合多动作 OS 级派发（硬约束限制）；高 DPI/托盘视觉逐项验收。


