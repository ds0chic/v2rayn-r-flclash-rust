# 原版差异审查交付

当前实现与原版仍有较大功能、操作对象、提交/取消、生效和生命周期差距，不能按“已全部移植”交付。这次审查发现的不是一组 padding 问题；不少地方是入口调用了错误用例，或后端功能没有接到普通使用流程。

基线：当前应用 `1251cbc6821276e35d082f7739b8b9c15b6f93dd`；冻结 v2rayN **7.25.4** `7d6a967c18c697f28dc6917122ed3a4993fcf336`。本轮以冻结版对照，不称今天联网最新版。Windows 主合同是 WPF，Avalonia 平台差异单独记录。

用户授权的三名子代理分别负责节点/格式、设置/订阅/路由/DNS、运行时/桌面/更新，根代理负责主窗口/枚举和交叉复核。台账 **800 行**分配为 258 / 373 / 107 / 62；每行有原版预期、当前行为、差异、两边定位、证据层级、运行检查、未验证与下一步。完整性由 `coverage.json` 验证，不靠报告标题判断。

**800 是审查条目数，含字段、枚举、窗口与布局，不是 800 个独立功能，也不是完成度分母。** 全条目源码核对不等于全功能实机验收。原版期待来自冻结源码；没有启动原版双窗口逐事件比较。当前实测六条 Windows 流程，使用真实 Flutter/FRB/Rust/SQLite 和隔离合成数据；其它效果、重开、全部内核及平台没有因此获得“verified”。

## 先阅读这些

| 文件 | 用途 |
|---|---|
| [repair-queue.md](repair-queue.md) | 按正常使用流程排序的16个工作包及拆卡/验收要求，给后续模型实施 |
| [root-report.md](root-report.md) | 六个真窗口复现、入口/右键/布局/窗口与交叉纠错 |
| [profiles-report.md](profiles-report.md) | 29组节点、认证、编辑、导入导出、测速、组、快捷键差异 |
| [settings-report.md](settings-report.md) | 20组设置、订阅、路由、DNS、热键、备份差异及180字段消费分析 |
| [runtime-report.md](runtime-report.md) | 20组运行、生效、TUN、内核、监控、托盘、后台与更新差异 |
| [all-items.csv](all-items.csv) | 筛选全部行、负责人、关联问题与修复动作（UTF-8 BOM） |
| [all-items.json](all-items.json) | 机器可读完整记录，保留冻结合同/源定位 |
| [coverage.json](coverage.json) | 分配与结果key集合、缺漏/重复、基线和引用检查 |

领域报告编号会交叉指向同一个问题，**29+20+20+10 不能当作79个互不重叠缺陷**。大量字段共享一个未接消费者，也不能逐字段累加成独立bug。修复要按用户流程与接口依赖分卡。

## 当前已复现的六个差异

| 正常操作 | 当前结果 |
|---|---|
| 原版认可的完整 Xray JSON 粘贴导入 | 0节点，保存失败 E_FIELD_REQUIRED |
| 新增 TUIC | 只有一个 UUID 标签的password字段，缺独立 UUID/密码 |
| 新增仅备注普通分组 | 被URL必填拒绝 |
| 扫描屏幕二维码 | 打开分享已有节点的二维码 |
| 扫描图片二维码 | 打开文本导入框 |
| 右键生成全部策略组 | 菜单关闭后目标丢失，未生成，提示先选节点 |

完整六场景证据：[ui-run-04/observations.json](ui-run-04/observations.json)、[flutter-test.log](ui-run-04/flutter-test.log)。前四场景截图在[ui-run-02](ui-run-02/)；第二轮生成组前提不足，不用其结果证明差异。第三轮 native 崩溃记录在[ui-run-03](ui-run-03/)，部分观察未计入。第一轮完整配置夹具缺原版判别字段，已经排除；合法夹具重跑仍复现保存失败，纠正过程保留在root-report。不要复制第一轮 unsupported format 为产品证据。

源码另确认的主要缺口：特殊节点编辑走普通编辑器；启用同一节点会取消活动、切换不按原版自动生效；路由移动/导入提前落盘、清空规则不能保存；部分应用按钮不保存草稿、DNS取消不回滚导入；备份恢复未重建打开的业务状态且原版设置未进入活动配置；TUN开关/监控/经代理下载没有正常运行会话接线；12个适用代理core缺adapter；热键编码与回调、托盘动态动作、后台调度缺失；内核安装位置与runtime查找不一致、自身更新来源/runner/安装链不符。详细条件及尚未实测边界以各报告为准。

已修改善也保留：右键坐标转换/外点与Esc关闭、17根条目/4分隔/32px高度、顶部分组、本地化列标题、部分编辑器间距。当前不重复旧报告的已修问题。已有 Xray/sing-box 配置、持久化、受管runtime、Clash PUT/DELETE、ownership恢复、PGP验证器、upgrade_runner 也真实存在；报告定位的是具体缺链，不说这些全没做。

## 实际运行的验证

| 检查 | 结果及范围 |
|---|---|
| 现有Rust codec36 + settings26 + config/runtime99 | **161项通过**，纯/本地合成后端检查，具体命令见领域报告 |
| 新增原版 InnerFmt wire 回归 | 显式加 `-- --ignored` 运行时 **2项失败**：原版VLESS读为Vmess、当前导出缺PascalCase ConfigType，日志profiles-inner-regression-ignored.log。它们暂时标为 ignored，修复后应转绿并取消 ignored。 |
| subscriptions 默认门禁 | `cargo test -p subscriptions --locked` 退出0，97项通过、2项已知差异 ignored；此数与上方161项存在重叠，不相加。 |
| 菜单/右键模型widget测试 | **7项通过**；只验证模型结构，不能证明命令效果 |
| 新增Windows实际UI流程 | 构建成功；第四轮在有效分组前提下完整执行，**6项冻结合同检查失败**；不启动内核或写宿主设置 |
| 新增Flutter测试文件格式/analyze | 0格式变化，No issues found |
| 完整性检查 | tools/audit_parity_consolidate.py 校验全部800 key、schema与来源，具体结果见coverage.json |

本轮没有运行全部workspace门禁、Flutter全量测试/新release包、远端/TLS握手、真实TUN/代理/自启写入或macOS/Linux/ARM64。已知差异回归保留显式运行并在默认 subscriptions 测试中隔离；不能由此宣称原版兼容测试通过。

本轮只新增审查脚本、报告/任务卡和回归测试；没有改生产应用、冻结源码、方案、compat或发布包，没有commit。开始已有 dist/SHA256SUMS 与 dist/build-info.json 的改动不属于本审查。审查完成和产品完成是两件事，下一步应按修复队列推进，不能重新包装成“全功能完成”。

复核命令：`python -X utf8 tools/audit_parity_consolidate.py`；只重新生成汇总，不更改产品。真实UI复验用固定测试文件与全新data/evidence目录、AUTOSTART/AUTO_SMOKE=0，详细条件见root-report与测试源。证据哈希在evidence-sha256.json。
