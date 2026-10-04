# Windows 真实窗口安全复现

应用源码 `a2b6905`，冻结合同见 `profiles.md` RE-PROF-01/05。运行 `apps/desktop/integration_test/parity_recheck_group_selection_test.dart`，`flutter test ... -d windows -r expanded`；用全新 `%TEMP%/v2rayn-r-recheck-<GUID>` 作为 `V2RAYN_R_DATA_DIR`，`V2RAYN_R_AUTOSTART=0`、`V2RAYN_R_AUTO_SMOKE=0`。合成普通分组 A/B、合成 TUIC 节点地址 `192.0.2.77`；没有订阅 URL、内核启动、端口监听、系统代理/TUN/自启写入。测试通过真实 Windows Flutter 窗口→FRB→Rust→临时 SQLite，不是人工手动鼠标，也未运行冻结原版窗口。

命令 exit **1**，是有意保留的冻结合同断言失败；Windows Debug 程序构建成功、窗口流程完整执行。结构化观察在 [observations.json](observations.json)：

1. 先选 A，再通过主菜单“添加 [TUIC]”保存：`currentGroupId=s-18db3536afeb45ec-0`、`savedSubId=""`、A 视图 `visibleIds=[]`。节点已落库，但不属于眼前组。
2. 用真实桥接将节点移到 B，在 B 的节点表选中，再切回 A：A 视图 `visibleIds=[]`，`selectedIds` 仍含该隐藏节点。选中状态没有随可见范围清理。

程序启动有 `tray init failed: PlatformException(Bad Arguments, ...)` 和 runtime seq gap 日志；这次 Debug 测试环境不能据此认定发布包托盘或运行时故障，另案复验。没有运行删除/测速等潜在副作用，故第二项只证明隐藏选择残留，批量动作会命中哪一个目标仍需独立隔离测试。`flutter analyze` 与 `dart format --set-exit-if-changed` 对新增测试文件均通过。
