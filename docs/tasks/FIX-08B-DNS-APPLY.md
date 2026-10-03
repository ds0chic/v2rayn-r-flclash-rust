# FIX-08B — DNS 导入/应用完整流（FIX-08 拆分）

状态：`identified`

本次唯一用户流程：DNS 窗口内“导入默认（Xray/sing-box）→ 预览确认 →
保存/应用”与“区域预设 → 应用 → 重开”，以及自定义启用的联动。
FIX-08 已保证导入默认只预览、取消无残留、保存先验后写；
本卡补齐导入后的确认/保存/应用闭环与预设远端链。

前置：`docs/tasks/FIX-08.md`（草稿/保存/合并语义已落地）、
冻结 `DNSSettingViewModel.cs:49-61`（导入只改窗口属性）、
`ConfigHandler.cs:2762-2795`（`GetExternalDNSItem` 外部模板下载链）。

对应 ID：`ACT-DNS-002`、`ACT-DNS-003`、`F-DNS-002`、`F-DNS-005`、
`FLD-ENT-118/121/122`。

必读上游符号：`DNSSettingViewModel.ImportDefConfig4V2ray/SingboxCompatibleCmd`、
`IsSimpleDNSEnabled`（双核自定义同时启用时禁用普通区，本实现缺联动）、
`AppManager.GetDNSItem`、区域预设远端模板（`Global.*Sources`）。

输入/提交/取消/错误/生效：
- 导入默认先预览（FIX-08 已做），本卡加：缺行时建行 vs 更新行的语义、
  保存时校验、应用时按原版时机生效并刷新。
- 区域预设：离线 pending 展示已存在；远端模板下载/校验/事务落盘、
  失败保留旧行、取消不替换，均未实现。
- `IsSimpleDNSEnabled` 联动：双自定义启用时普通区禁用（只读提示），
  避免用户改了普通区却被自定义覆盖的假保存。
- 导入/预设造成的其它页草稿覆盖问题：预设应用后只刷新受影响页，
  未保存的其它页编辑不丢失（需明确语义并测试）。

允许修改：`dns_window.dart`、`dns_controller.dart`、DNS 相关 bridge/Rust
（不改生成文件；需新 API 则登记缺口）、测试与证据。

禁止：改动 subscriptions/profiles/main_shell/生成文件；全仓 format；
release 构建（根代理统一跑）。

测试：widget（导入→保存→重开、导入→取消无残留、预设 pending/失败分支、
联动禁用）＋ Rust（缺行建行、非法模板拒绝、远端失败保留旧行）；
真实窗口集成证据归 `docs/evidence/UX-PARITY-FIX-08B/`。

完成条件：导入/预设任一分支取消不留痕、错误不关窗不报成功、
成功后重开一致、远端失败保留旧数据。
