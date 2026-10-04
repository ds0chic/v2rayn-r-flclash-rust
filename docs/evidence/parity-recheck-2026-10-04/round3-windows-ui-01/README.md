# 第三轮 Windows 窗口复验

当前基线 `7edf1ee`；发布包 `dist/build-info.json` 表示源码 `a4ceab5`、dirty=false。运行现有 `apps/desktop/integration_test/parity_recheck_group_selection_test.dart`，命令：

```powershell
flutter test integration_test/parity_recheck_group_selection_test.dart -d windows -r expanded
```

使用全新 `%TEMP%/v2rayn-r-round3-<GUID>` 作为 `V2RAYN_R_DATA_DIR`，`V2RAYN_R_AUTOSTART=0`、`V2RAYN_R_AUTO_SMOKE=0`。只建合成空 URL 普通分组与 TUIC 节点；没有启动核心、监听端口或修改宿主代理/TUN/自启。Windows Debug 构建成功，测试 **exit 0，1/1用例通过**。两条细分冻结合同观察见 [observations.json](observations.json)：

- 选A后新增 TUIC，保存 `Subid` 等于A，节点仍在A可见列表。上轮 `Subid=""` 的失败已转绿。
- 将节点移到B并在B选中，切回A后 `selectedIds=[]` 且 A 无该节点。上轮隐藏选择残留已转绿。

这是 Flutter Windows 窗口→FRB→Rust→隔离SQLite 的自动集成测试，不是冻结原版双窗口人工逐事件对照；不证明粘贴/扫码、删除/测速后果、重开或发布版 exe。测试日志仍出现 Debug 环境 tray init `Bad Arguments` 与 runtime seq gap，不能由此断定正式包托盘失败，应单独复验。
