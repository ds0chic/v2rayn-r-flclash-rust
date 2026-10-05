# R4-13.S23 evidence (UiItem)

- 状态：implemented（存储链 + 窗口/列状态源 + DPI 矩阵）；实时三布局尺寸接线为 R4-12 文件所有权 hand-off → blocked。
- 字段：FLD-CFG-067..083、117..119、156..158（三布局 MainGirdOrientation、MainGirdHeight1/2、列宽/列布局 MainColumnItem、窗口尺寸 WindowSizeItem、字体/主题/隐藏列等）。
- 存储合同：`test/r4_13_s23_contract_test.dart`（3 例，pass：UiItem 字段保存→重开；MemoryUiStateStore 窗口几何+列布局重开；100% DPI 三布局）。
- 复现护栏：`test/repair/r4_13_s23_repro_test.dart`（三窗口几何/两表列布局全文档重开；150% DPI 三布局）。
- R4-12 衔接：`MainGirdOrientation` 状态源已由 R4-12 消费（HEAD cbce523，`r4_12_contract_test.dart` 覆盖）；本实例不重复断言。
- blocked：`MainGirdHeight1/2` 的实时主区尺寸接线位于 `lib/app/shell/ui_shell_controller.dart`，属 R4-12 文件所有权（本卡禁止修改），登记为接口 hand-off，未在本实例重写。
