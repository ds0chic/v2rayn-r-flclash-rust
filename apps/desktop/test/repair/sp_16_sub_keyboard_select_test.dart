// SP-16 订阅窗多选键盘 parity（合成数据专用）。
//
// 唯一流程：订阅设置窗的键盘多选与上游 DataGrid Extended 默认语义对等
// （Shift+方向键扩展、Ctrl+A 全选、方向键移动当前行）；普通单击仍收拢单选。
// 不读用户秘密，不触网络/10808。
//
// 原版依据（冻结 7d6a967，只读 work/，本次复核）：
// - v2rayN/Views/SubSettingWindow.xaml（84-95）与
//   v2rayN.Desktop/Views/SubSettingWindow.axaml（32-43）的 DataGrid 均未声明
//   SelectionMode → WPF/Avalonia Extended 默认（含 Shift+方向键、Ctrl+A、
//   Shift/Ctrl+单击）；显式 KeyBinding 只有 Delete→SubDeleteCmd。
// - SubSettingViewModel.SelectedSources（多选集）/ SelectedSource（主行）；
//   只有 DeleteSubAsync（84-96）消费选中集，Edit/Share 取单行；SubSetting
//   内无 update 命令（update 只在主窗：全部 SubIndexId="" 或当前组）。
// - 两端右键菜单只有 新增/删除/编辑/分享：无“全选”菜单项，故本项目不加。
// - 主窗更新语义为单 id 过滤（""=全部，SubIndexId=当前组），不是选中集批量，
//   故订阅窗单行 update 保持现状，不伪造多选批量更新。
//
// 技术路径（已有桥接缝，未新增 FRB/Rust API）：键盘选择只改
// subs_controller 的 selectedIds/selectedId/selectionAnchorId；删除仍走已有
// 批量路径；update 触及桥 job 管线，不在本卡 scope 内改动。
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

c.SubItemDto _synSub(String remarks) => c.SubItemDto(
  id: '',
  remarks: remarks,
  url: 'https://example.com/${remarks.hashCode & 0xffff}',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 0,
  autoUpdateInterval: 0,
  updateTime: 0,
);

/// 预置 A/B/C/D 四个合成组的容器。
ProviderContainer _fourSubContainer() {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(50),
    ],
  );
  final subs = container.read(subsControllerProvider.notifier);
  subs.save(_synSub('A'));
  subs.save(_synSub('B'));
  subs.save(_synSub('C'));
  subs.save(_synSub('D'));
  return container;
}

String _idOf(ProviderContainer container, String remarks) => container
    .read(subsControllerProvider)
    .items
    .firstWhere((s) => s.remarks == remarks)
    .id;

void main() {
  group('SP-16 subs keyboard select model (synthetic)', () {
    test('selectAll selects every row and keeps the primary', () {
      final container = _fourSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final bId = _idOf(container, 'B');

      controller.select(bId);
      controller.selectAll();
      final state = container.read(subsControllerProvider);
      expect(state.selectedId, bId);
      expect(state.selectedIds, <String>[
        _idOf(container, 'A'),
        bId,
        _idOf(container, 'C'),
        _idOf(container, 'D'),
      ]);
    });

    test('selectAll without a primary ends on the last row', () {
      final container = _fourSubContainer();
      addTearDown(container.dispose);
      container.read(subsControllerProvider.notifier).selectAll();
      final state = container.read(subsControllerProvider);
      expect(state.selectedIds.length, 4);
      expect(state.selectedId, _idOf(container, 'D'));
    });

    test('arrow move collapses and clamps at the ends', () {
      final container = _fourSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final dId = _idOf(container, 'D');

      controller.select(bId);
      controller.toggleMultiSelected(dId);
      controller.movePrimary(1);
      // 普通方向键收拢为单行（上游行导航对等）。
      expect(container.read(subsControllerProvider).deleteIds, <String>[dId]);

      controller.select(aId);
      controller.movePrimary(-1);
      expect(container.read(subsControllerProvider).selectedId, aId);

      controller.select(dId);
      controller.movePrimary(1);
      expect(container.read(subsControllerProvider).selectedId, dId);
    });

    test('shift extend grows from a fixed anchor across repeats', () {
      final container = _fourSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final cId = _idOf(container, 'C');

      controller.select(aId);
      controller.extendKeyboardSelection(1);
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        aId,
        bId,
      ]);
      // 锚点不动：第二次扩展累加而不是漂移。
      controller.extendKeyboardSelection(1);
      final state = container.read(subsControllerProvider);
      expect(state.deleteIds, <String>[aId, bId, cId]);
      expect(state.selectedId, cId);
      expect(state.selectionAnchorId, aId);
    });

    test('shift extend upward and clamp keep the anchor range', () {
      final container = _fourSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final bId = _idOf(container, 'B');
      final cId = _idOf(container, 'C');
      final dId = _idOf(container, 'D');

      controller.select(dId);
      controller.extendKeyboardSelection(-1);
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        cId,
        dId,
      ]);

      // 顶端钳制：不再收缩为单行。
      controller.select(_idOf(container, 'A'));
      controller.extendKeyboardSelection(-1);
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        _idOf(container, 'A'),
      ]);
      expect(bId, isNotNull);
    });

    test('plain select after a keyboard extend re-seats the anchor', () {
      final container = _fourSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final cId = _idOf(container, 'C');

      controller.select(aId);
      controller.extendKeyboardSelection(1);
      controller.extendKeyboardSelection(1);
      controller.select(bId);
      controller.extendKeyboardSelection(1);
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        bId,
        cId,
      ]);
    });
  });

  group('SP-16 subs-window keyboard gestures (synthetic)', () {
    Future<ProviderContainer> pumpWindow(WidgetTester tester) async {
      final container = _fourSubContainer();
      addTearDown(container.dispose);
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      return container;
    }

    testWidgets('ctrl+A selects all rows in the real window', (tester) async {
      final container = await pumpWindow(tester);
      final aId = _idOf(container, 'A');

      await tester.tap(find.byKey(ValueKey('sub-row-$aId')));
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.control);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.control);
      await tester.pump();
      expect(container.read(subsControllerProvider).selectedIds.length, 4);
      expect(container.read(subsControllerProvider).selectedId, aId);
    });

    testWidgets('shift+Down extends twice from a fixed anchor', (tester) async {
      final container = await pumpWindow(tester);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final cId = _idOf(container, 'C');

      await tester.tap(find.byKey(ValueKey('sub-row-$aId')));
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shift);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        aId,
        bId,
      ]);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shift);
      await tester.pump();
      final state = container.read(subsControllerProvider);
      expect(state.deleteIds, <String>[aId, bId, cId]);
      expect(state.selectedId, cId);
    });

    testWidgets('plain Down collapses the multi-select to the neighbour', (
      tester,
    ) async {
      final container = await pumpWindow(tester);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');

      await tester.tap(find.byKey(ValueKey('sub-row-$aId')));
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.control);
      await tester.tap(find.byKey(ValueKey('sub-row-$bId')));
      await tester.sendKeyUpEvent(LogicalKeyboardKey.control);
      await tester.pump();
      expect(
        container.read(subsControllerProvider).deleteIds.length,
        greaterThan(1),
      );

      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        _idOf(container, 'C'),
      ]);
    });
  });
}
