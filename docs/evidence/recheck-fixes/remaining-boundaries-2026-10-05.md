# 剩余事项的阻塞性质复核（2026-10-05）

审查 HEAD `3893e01`，冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。本页只对“哪些确实不能在当前宿主直接验收”作分类，不把代码存在、单测通过写成完整用户流程 `verified`。没有读取用户配置或修改宿主网络、热键、路由。

| 用户列项 | 准确分类 | 当前证据与下一步 |
| --- | --- | --- |
| 1. 生产 TUN 全链路 | **需要隔离 VM 与明确授权才能做自动路由真机验收**。代码路径 `implemented`，全链 `blocked`/未验证。 | [R3-ROOT-03-R3-04](R3-ROOT-03-R3-04/README.md) 已实现“核心创建接口→发现→helper→ready/清理”；[real-os-2](real-os-2/README.md) 仅实测 `auto_route=false` 的适配器创建/销毁、独立提权路由增删。生产 net-host+helper+auto route 没有跑，不能称 TUN 功能 `verified`。 |
| 2. 真实发行源自更新 | **外部发布资产/产品决策阻断真实远端整链**；本地合成 runner、校验、回滚可继续验证。 | [FIX-12B](../UX-PARITY-FIX-12B/README.md) 与 `docs/tasks/RR-04.md` 记录 `app_repo=None`、未配置自有签名发行源；不能拿原版 WPF 包替代。需要真实仓库、版本/资产命名和签名公钥后才能验收远端下载→验签→替换→重启。 |
| 3. 其余 12 核下载与运行矩阵 | **独立工程/验证工作，非“本机无法完成”**。当前只是候选 adapter `implemented`，未锁定的 12 核真实会话 `未验证`。 | `tools/cores/cores.lock.json` 只有 Xray、sing-box；`crates/runtime/src/adapter.rs` 有 14 个代理核候选。应按冻结原版核心清单逐核选官方资产、钉版本/hash/来源、跑 version/test args、真实配置与最小会话、错误/退出/清理；每核单卡，端口先探测且≥11808。版本/来源可形成可审阅方案后再确定，不能把 adapter 数等同 14 核运行验收。 |
| 4. 设置/路由独立窗口 | **本地尚未实现的界面/架构工作，非外部授权阻断**。原始需求要求原版界面结构，默认应继续实施。 | [WPF 对照](R3-WPF-COMPARE/README.md) 已实拍：原版两个独立顶层窗口，当前 `OptionSettingWindow.show` / `showRoutingSettingWindow` 使用 `showDialog`。标题/字段等 Wave K 改进不改变窗口形态。已分为[设置窗口卡](../../tasks/R3-WPF-OPTION-WINDOW.md)和[路由窗口卡](../../tasks/R3-WPF-ROUTING-WINDOW.md)，先验证最小多窗口宿主，再验焦点/关闭/打包。 |
| 5. 同组合多动作 OS 派发 | **当前宿主受安全约束，不能用会写系统代理的动作做完整 OS 真机测试**；分组逻辑已有单测、单一无害动作 OS 全链 `verified`。 | [SR-REAL-HOTKEY](SR-REAL-HOTKEY/README.md) 实测 `Ctrl+Alt+F12` 显示/隐藏窗口注册→按键→回调→注销。另有**独立的本地代码缺口**：所用 `hotkey_manager_windows` 忽略 `RegisterHotKey` 失败，真机冲突可能被误报成功；不能以本项安全限制掩盖该缺陷。可用应用侧原生返回值探测/替换注册实现，并用被外部占用的无害组合验证冲突。 |
| 6. 高 DPI/托盘视觉 | **本地验收工作，非决策或授权阻断**。 | [WPF 对照](R3-WPF-COMPARE/README.md) 只在 96 DPI 拍三窗口；Wave J/K 已改托盘状态图标和主窗口结构，但未在 125%/150%/200% 缩放、浅深主题、托盘展开/折叠和窗口重开逐项截图比对。按隔离数据目录和不触系统代理的方式补视觉/交互证据。 |

这六项**不是完整剩余缺口清单**。[收尾摘要](README.md) 还登记路由“一键导入规则集”后端用例与多选；[T21-D](../T21-real-os.md) 记录 WinINET per-connection 推送失败、对运行中应用的即时效果未逐应用确认。两者也不能归入“仅等用户授权”。

当前 Windows ZIP 已从干净源码提交 `ff45519` 构建，`dist/build-info.json` 为 `git_dirty=false`；SHA-256 `d3b4a07a09d0425c73769cdef5de447692c29c557e86b8bb2ec0171d0069048f` 与 `dist/SHA256SUMS` 一致。这纠正第三轮审查时旧包 `git_dirty=true` 的历史结论；它不提升上述未实测功能的状态。

## 2026-10-05 本轮解决（Wave L/M）

- 第 3 项：12 核已下载并按官方资产钉版本/hash 到 `tools/cores/cores.lock.json`；`tools/cores/session_matrix.ps1` 全矩阵 **14/14 真实回环会话**（hysteria2/overtls 用临时自签 TLS 对会话解除 blocked），逐核 version 14/14；adapter 合同按实测修正（v2fly `-config`、v2fly_v5 去 `-format jsonv5`、hysteria 工作目录）。见 `R3-CORE-MATRIX/`。
- 第 4 项：设置与路由均已迁移为原生第二顶层窗口（owner=主窗口、模态、第二 Flutter engine、草稿回传主侧保存），HWND 探针验证两个独立窗口与关闭/重开；`flutter build windows --release` 通过。见 `R3-WPF-OPTION-WINDOW/`、`R3-WPF-ROUTING-WINDOW/`；逐事件脚本与高 DPI 真机对照仍登记。
- 第 5 项：热键冲突已如实检测（自持 `RegisterHotKey` 探针 + 1409 语义），真机“占用→冲突→释放→成功→注销→组合空闲”全链通过。见 `SR-HOTKEY-CONFLICT/`。
- 第 6 项：四状态托盘 ico 已生成并打包；DPI widget 矩阵 100/125/150/200%（1280×800 与 800×600）无溢出；宿主 96 DPI 真机浅/深主题与托盘图标/菜单截图完成；真机高 DPI、coreRunning/proxyActive 真机态登记为边界。见 `R3-VISUAL-DPI-TRAY/`。
- 当前包：干净构建 `73da06e`，zip `45a702b7…`、setup `a8fd08e2…`，`git_dirty=false`。

仍为外部条件/产品决策：第 1 项生产 TUN 自动路由（隔离 VM 授权）、第 2 项真实发行源自更新（发布仓库/签名资产/公钥）。
