// SP-16 订阅窗多选批量删除（合成数据专用）。
//
// 唯一流程：订阅设置窗多选（Ctrl+单击切换、Shift+单击区间，普通单击仍收拢为
// 单选）后删除一次确认删全部选中行；单选行为保持不变。
// 取消/失败不写库、不换组；失败报出错误并保留各组与选中集。
// 不读用户秘密，不触网络/10808。
//
// 原版依据（冻结 7d6a967）：
// - SubSettingViewModel.SelectedSources（多选集）/ SelectedSource（主行）。
// - SubSettingViewModel.DeleteSubAsync（84-96）：一次 ShowYesNo 确认后删
//   `SelectedSources ?? [SelectedSource]` 全部行，再 RefreshSubItems。
// - ResUI.RemoveServer = "Are you sure you want to remove?"（单次确认）。
//
// 技术路径（已有桥接缝，未新增 FRB/Rust API）：选中集为 subs_controller 的
// selectedIds（primary 仍是 selectedId）；删除走已有 controller.delete（成功
// 后 profiles reload + resync，成功才裁剪选中集）；确认框为共享
// showAppConfirmDialog（与单删同键）。
import 'package:flutter/gestures.dart';
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

import '../support/subs_harness.dart';

/// 删除桥恒失败（delete 失败路径）。
class FailDeleteBridge extends SyntheticBridgePort {
  @override
  c.DeleteSubsResult deleteSubItems(List<String> ids) {
    return c.DeleteSubsResult(
      ok: false,
      removed: BigInt.zero,
      error: const c.ErrorDto(
        code: 'E_SYN_DELETE_FAIL',
        messageKey: 'error.syn_delete_fail',
        retryable: false,
      ),
    );
  }
}

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

/// 预置 A/B/C 三个合成组的容器。
ProviderContainer _threeSubContainer({BridgePort? bridge}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge ?? SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(50),
    ],
  );
  final subs = container.read(subsControllerProvider.notifier);
  subs.save(_synSub('A'));
  subs.save(_synSub('B'));
  subs.save(_synSub('C'));
  return container;
}

String _idOf(ProviderContainer container, String remarks) => container
    .read(subsControllerProvider)
    .items
    .firstWhere((s) => s.remarks == remarks)
    .id;

/// 捕获 WidgetRef 以便驱动需要 context/ref 的删除入口（确认框经注入覆盖，
/// 不弹真实对话框）。
Future<({BuildContext context, WidgetRef ref})> _pumpRef(
  WidgetTester tester,
  ProviderContainer container,
) async {
  BuildContext? capturedContext;
  WidgetRef? capturedRef;
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Scaffold(
          body: RefProbe(
            onRef: (context, ref) {
              capturedContext = context;
              capturedRef = ref;
            },
          ),
        ),
      ),
    ),
  );
  await tester.pump();
  return (context: capturedContext!, ref: capturedRef!);
}

void main() {
  group('SP-16 subs multi-select model (synthetic)', () {
    test('plain select collapses to single (single-select unchanged)', () {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');

      controller.select(aId);
      controller.toggleMultiSelected(bId);
      expect(
        container.read(subsControllerProvider).deleteIds,
        containsAll(<String>[aId, bId]),
      );
      // 普通单击收拢为单选。
      controller.select(aId);
      expect(container.read(subsControllerProvider).selectedId, aId);
      expect(container.read(subsControllerProvider).deleteIds, <String>[aId]);
    });

    test('toggle adds and removes, empty clears the primary', () {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');

      controller.toggleMultiSelected(aId);
      controller.toggleMultiSelected(bId);
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        aId,
        bId,
      ]);
      expect(container.read(subsControllerProvider).selectedId, bId);
      controller.toggleMultiSelected(aId);
      expect(container.read(subsControllerProvider).deleteIds, <String>[bId]);
      controller.toggleMultiSelected(bId);
      expect(container.read(subsControllerProvider).deleteIds, isEmpty);
      expect(container.read(subsControllerProvider).selectedId, isNull);
    });

    test('shift range covers the item order inclusive', () {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final cId = _idOf(container, 'C');

      controller.select(aId);
      controller.selectRange(aId, cId);
      expect(container.read(subsControllerProvider).deleteIds, <String>[
        aId,
        bId,
        cId,
      ]);
      expect(container.read(subsControllerProvider).selectedId, cId);
    });

    test('empty selection gates delete (no target, no confirm)', () {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      expect(container.read(subsControllerProvider).deleteIds, isEmpty);
    });

    test('successful delete prunes only the removed ids', () {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final cId = _idOf(container, 'C');

      controller.toggleMultiSelected(aId);
      controller.toggleMultiSelected(bId);
      controller.toggleMultiSelected(cId);
      controller.delete(<String>[aId, bId]);
      final state = container.read(subsControllerProvider);
      expect(state.items.map((s) => s.remarks), <String>['C']);
      expect(state.deleteIds, <String>[cId]);
      expect(state.selectedId, cId);
    });
  });

  group('SP-16 subs batch confirmAndDeleteSubs (synthetic)', () {
    testWidgets('cancel keeps all, confirm removes only the batch', (
      tester,
    ) async {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      controller.toggleMultiSelected(aId);
      controller.toggleMultiSelected(bId);
      var confirmCalls = 0;

      expect(
        await confirmAndDeleteSubs(
          captured.context,
          captured.ref,
          container.read(subsControllerProvider).deleteIds,
          confirmDelete: (_) async {
            confirmCalls++;
            return false;
          },
        ),
        isFalse,
      );
      expect(confirmCalls, 1);
      expect(container.read(subsControllerProvider).items.length, 3);

      expect(
        await confirmAndDeleteSubs(
          captured.context,
          captured.ref,
          container.read(subsControllerProvider).deleteIds,
          confirmDelete: (_) async {
            confirmCalls++;
            return true;
          },
        ),
        isTrue,
      );
      expect(confirmCalls, 2);
      // 只删选中：C 保留，选中集裁剪到空（无悬空）。
      expect(
        container.read(subsControllerProvider).items.map((s) => s.remarks),
        <String>['C'],
      );
      expect(container.read(subsControllerProvider).deleteIds, isEmpty);
    });

    testWidgets('single id through the batch path keeps the others', (
      tester,
    ) async {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      final aId = _idOf(container, 'A');

      expect(
        await confirmAndDeleteSubs(captured.context, captured.ref, <String>[
          aId,
        ], confirmDelete: (_) async => true),
        isTrue,
      );
      expect(
        container.read(subsControllerProvider).items.map((s) => s.remarks),
        containsAll(<String>['B', 'C']),
      );
      expect(
        container.read(subsControllerProvider).items.map((s) => s.id),
        isNot(contains(aId)),
      );
    });

    testWidgets('unknown ids gate delete without a confirm', (tester) async {
      final container = _threeSubContainer();
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      var confirmCalls = 0;

      expect(
        await confirmAndDeleteSubs(
          captured.context,
          captured.ref,
          const <String>['g-gone'],
          confirmDelete: (_) async {
            confirmCalls++;
            return true;
          },
        ),
        isFalse,
      );
      expect(confirmCalls, 0);
      expect(container.read(subsControllerProvider).items.length, 3);
    });

    testWidgets('batch delete failure keeps data, selection and reports', (
      tester,
    ) async {
      final container = _threeSubContainer(bridge: FailDeleteBridge());
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      final controller = container.read(subsControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      controller.toggleMultiSelected(aId);
      controller.toggleMultiSelected(bId);

      expect(
        await confirmAndDeleteSubs(
          captured.context,
          captured.ref,
          container.read(subsControllerProvider).deleteIds,
          confirmDelete: (_) async => true,
        ),
        isFalse,
      );
      expect(container.read(subsControllerProvider).items.length, 3);
      expect(
        container.read(subsControllerProvider).deleteIds,
        containsAll(<String>[aId, bId]),
      );
      expect(container.read(subsControllerProvider).status!.isError, isTrue);
    });
  });

  group('SP-16 subs-window batch dialog + row gestures (synthetic)', () {
    Future<ProviderContainer> pumpWindow(WidgetTester tester) async {
      final container = _threeSubContainer();
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

    testWidgets('ctrl+click multi-selects, delete button batch-confirms', (
      tester,
    ) async {
      final container = await pumpWindow(tester);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final controller = container.read(subsControllerProvider.notifier);

      await tester.tap(find.byKey(ValueKey('sub-row-$aId')));
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.control);
      await tester.tap(find.byKey(ValueKey('sub-row-$bId')));
      await tester.sendKeyUpEvent(LogicalKeyboardKey.control);
      await tester.pump();
      expect(
        container.read(subsControllerProvider).deleteIds,
        containsAll(<String>[aId, bId]),
      );
      expect(controller, isNotNull);

      // 批量确认框带计数；取消则全部保留。
      await tester.tap(find.byKey(const ValueKey('sub-delete')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('sub-delete-confirm')), findsOneWidget);
      expect(find.text('确认删除选中的 2 个订阅？'), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('sub-delete-cancel')));
      await tester.pumpAndSettle();
      expect(container.read(subsControllerProvider).items.length, 3);

      // 确认则只删选中，C 保留。
      await tester.tap(find.byKey(const ValueKey('sub-delete')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('sub-delete-confirm-ok')));
      await tester.pumpAndSettle();
      expect(
        container.read(subsControllerProvider).items.map((s) => s.remarks),
        <String>['C'],
      );
    });

    testWidgets('context menu on a selected row keeps the multi-select', (
      tester,
    ) async {
      final container = await pumpWindow(tester);
      final aId = _idOf(container, 'A');
      final bId = _idOf(container, 'B');
      final controller = container.read(subsControllerProvider.notifier);
      controller.toggleMultiSelected(aId);
      controller.toggleMultiSelected(bId);
      await tester.pump();

      final center = tester.getCenter(find.byKey(ValueKey('sub-row-$bId')));
      await tester.tapAt(center, buttons: kSecondaryButton);
      await tester.pumpAndSettle();
      // 右键已选行不坍缩多选：删除仍面向两行。
      expect(
        container.read(subsControllerProvider).deleteIds,
        containsAll(<String>[aId, bId]),
      );
      await tester.tap(find.byKey(const ValueKey('sub-menu-delete')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('sub-delete-confirm')), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('sub-delete-confirm-ok')));
      await tester.pumpAndSettle();
      expect(
        container.read(subsControllerProvider).items.map((s) => s.remarks),
        <String>['C'],
      );
    });
  });
}
