# 一键还原工具（测试/验收工具层）

只用于手动验收，不进入生产主界面。

## 最短流程（推荐）

1. 双击 `tools/acceptance/acceptance_restore.cmd` 打开小窗口。
2. 点 **「1. 准备测试（保存快照）」** —— 只读采集当前测试环境基线（保存到
   `%LOCALAPPDATA%\v2rayn-r\acceptance`，ACL 仅当前用户）。
3. 手动验收（应用内操作、你自有的真实节点测试等）。
4. 点 **「2. 一键还原（恢复测试前状态）」** —— 先显示将恢复的类别并让你确认，
   再幂等回滚**本工具记录的类别**；重复点击为 no-op。

命令行等价（可选）：
```powershell
. tools/acceptance/AcceptanceRestore.ps1
New-AcceptanceSnapshot -Backend real          # 只读采集
Get-AcceptanceRestorePlan -Backend real       # 预览类别（不写入）
Restore-AcceptanceSnapshot -Backend real      # 需在 GUI 中确认后使用
```

## 能恢复 / 不能恢复

| 类别 | 能否恢复 | 说明 |
|---|---|---|
| WinINET 代理开关/服务器/旁路/PAC | 仅在"当前值与基线不同且任一侧不含 20808"时恢复 | 幂等；相等即 no-op |
| Run-key（自启） | **只补写基线中记录的值；从不删除条目** | 避免盲删注册表 |
| 路由 / TUN | **不恢复** | 需隔离 VM 的 checkpoint 还原 |
| 全量网络重置 | **不做** | 明确禁止 |
| 应用数据目录（隔离测试目录） | 由测试脚本自行删除 | 工具不触碰 |

## 安全规则（fail-closed）

- `127.0.0.1:20808` 硬禁区：不连接/探测/绑定/停止/重启该端口，不改其配置或流量路径。
  若**基线或当前值**任一引用 20808，还原**直接拒绝**并提示改用隔离 VM checkpoint。
- 快照校验：schema + SHA256；损坏/缺失快照一律拒绝。
- 日志与证据不打印代理地址、节点凭据或任何秘密（只记录类别名、动作、短哈希）。
- 宿主代理/路由/TUN 的真实写入测试不在本机执行。

## 自测

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/acceptance_restore_selftest.ps1
```
覆盖：创建/恢复/重复恢复（幂等）、损坏与缺失快照拒绝、基线含 20808 拒绝、
当前态含 20808 拒绝、无路由/TUN 回滚类别、真实 WinINET 指纹前后一致（证明无宿主写入）。
仅使用 fake 内存后端；真实代理只做只读比对。
