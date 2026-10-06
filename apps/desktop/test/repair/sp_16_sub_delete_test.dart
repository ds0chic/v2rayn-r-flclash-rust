// SP-16 删除入口：主窗直接删 G + 订阅窗删除确认（合成数据专用）。
//
// 唯一流程：主窗工具栏删除当前订阅组（删当前组回 All）；订阅设置窗删除先确认。
// 取消/失败不写库、不换组；失败报出错误并保留该组。不读用户秘密，
// 不触网络/10808。
//
// 原版依据（冻结 7d6a967）：
// - ProfilesViewModel.DeleteSubAsync（885-900）：取 GetSubItem(SubIndexId)，
//   null 直接 return；ShowYesNoInteraction(RemoveServer)==false 直接 return；
//   DeleteSubItem 后 RefreshSubscriptions + SubSelectedChangedAsync。
// - SubSettingViewModel.DeleteSubAsync（84-96）：先 ShowYesNo 确认，再删选中行，
//   RefreshSubItems + 成功提示。
// - ResUI.RemoveServer = "Are you sure you want to remove?"。
//
// 技术路径（已有桥接缝，未新增 FRB/Rust API）：删除走
// subs_controller.delete（成功后 profiles reload + resyncGroupFromSubs，
// 删当前组内存回 All 且不写修复）；确认框为共享 showAppConfirmDialog。
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/sub_entry.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/sub_direct_edit.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import '../support/subs_harness.dart';

/// 删除桥恒失败（delete 失败路径）；订阅由用例按需播种，避免与播种重复。
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

/// 预置 A/B 两个合成组的容器。
ProviderContainer _twoSubContainer({BridgePort? bridge}) {
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
  group('SP-16 delete entry gating (synthetic)', () {
    test('live G resolves to a deletable target', () {
      final entry = resolveSubEntry(
        groupSubId: 'g-syn-1',
        existingIds: const ['g-syn-1', 'g-syn-2'],
      );
      expect(entry.kind, SubEntryKind.editCurrent);
      expect(entry.targetId, 'g-syn-1');
    });

    test('All view gates delete (upstream no-op, no confirm)', () {
      for (final gated in <String?>[null, '', '  ']) {
        final entry = resolveSubEntry(
          groupSubId: gated,
          existingIds: const ['g-syn-1'],
        );
        expect(entry.kind, SubEntryKind.gatedAll, reason: 'group=$gated');
      }
    });

    test('missing G gates delete instead of first group', () {
      final entry = resolveSubEntry(
        groupSubId: 'g-gone',
        existingIds: const ['g-syn-1'],
      );
      expect(entry.kind, SubEntryKind.gatedAll);
    });
  });

  group('SP-16 main-window deleteCurrentSub (synthetic)', () {
    testWidgets('All gate, then cancel, then confirm falls back to All', (
      tester,
    ) async {
      final container = _twoSubContainer();
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      final profiles = container.read(profilesControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      var confirmCalls = 0;
      Future<bool> never(String remarks) async {
        confirmCalls++;
        return false;
      }

      // All 视图门控：不弹确认框，无操作。
      expect(profiles.setGroupSubId(null), isTrue);
      expect(
        await deleteCurrentSub(
          captured.context,
          captured.ref,
          confirmDelete: (_) async {
            confirmCalls++;
            return true;
          },
        ),
        isFalse,
      );
      expect(confirmCalls, 0);
      expect(container.read(subsControllerProvider).items.length, 2);

      // 取消：一切保留。
      expect(profiles.setGroupSubId(aId), isTrue);
      expect(
        await deleteCurrentSub(
          captured.context,
          captured.ref,
          confirmDelete: never,
        ),
        isFalse,
      );
      expect(confirmCalls, 1);
      expect(container.read(subsControllerProvider).items.length, 2);
      expect(container.read(profilesControllerProvider).groupSubId, aId);

      // 确认：删当前组回 All，另一组保留。
      expect(
        await deleteCurrentSub(
          captured.context,
          captured.ref,
          confirmDelete: (_) async {
            confirmCalls++;
            return true;
          },
        ),
        isTrue,
      );
      expect(confirmCalls, 2);
      final ids = container.read(subsControllerProvider).items.map((s) => s.id);
      expect(ids, isNot(contains(aId)));
      expect(
        container.read(subsControllerProvider).items.map((s) => s.remarks),
        contains('B'),
      );
      expect(container.read(profilesControllerProvider).groupSubId, isNull);
    });

    testWidgets('delete failure surfaces an error and keeps the group', (
      tester,
    ) async {
      final container = _twoSubContainer(bridge: FailDeleteBridge());
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      final profiles = container.read(profilesControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      expect(profiles.setGroupSubId(aId), isTrue);

      expect(
        await deleteCurrentSub(
          captured.context,
          captured.ref,
          confirmDelete: (_) async => true,
        ),
        isFalse,
      );
      // 组与列表均保留，错误可见（窗内状态行 + 主窗提示）。
      expect(container.read(profilesControllerProvider).groupSubId, aId);
      expect(container.read(subsControllerProvider).items.length, 2);
      expect(container.read(subsControllerProvider).status!.isError, isTrue);
      expect(
        container.read(uiShellControllerProvider).message,
        contains('删除失败'),
      );
    });
  });

  group('SP-16 subs-window confirmAndDeleteSub (synthetic)', () {
    testWidgets('cancel keeps all, confirm removes the row', (tester) async {
      final container = _twoSubContainer();
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      final aId = _idOf(container, 'A');

      expect(
        await confirmAndDeleteSub(
          captured.context,
          captured.ref,
          aId,
          confirmDelete: (_) async => false,
        ),
        isFalse,
      );
      expect(container.read(subsControllerProvider).items.length, 2);

      expect(
        await confirmAndDeleteSub(
          captured.context,
          captured.ref,
          aId,
          confirmDelete: (_) async => true,
        ),
        isTrue,
      );
      expect(
        container.read(subsControllerProvider).items.map((s) => s.remarks),
        contains('B'),
      );
      expect(
        container.read(subsControllerProvider).items.map((s) => s.id),
        isNot(contains(aId)),
      );
    });

    testWidgets('delete failure surfaces an error and keeps the group', (
      tester,
    ) async {
      final container = _twoSubContainer(bridge: FailDeleteBridge());
      addTearDown(container.dispose);
      final captured = await _pumpRef(tester, container);
      final profiles = container.read(profilesControllerProvider.notifier);
      final aId = _idOf(container, 'A');
      expect(profiles.setGroupSubId(aId), isTrue);

      expect(
        await confirmAndDeleteSub(
          captured.context,
          captured.ref,
          aId,
          confirmDelete: (_) async => true,
        ),
        isFalse,
      );
      expect(container.read(subsControllerProvider).items.length, 2);
      expect(container.read(subsControllerProvider).status!.isError, isTrue);
      expect(container.read(profilesControllerProvider).groupSubId, aId);
    });
  });

  group('SP-16 subs-window real confirm dialog (synthetic)', () {
    testWidgets('cancel keeps rows, confirm removes the selected row', (
      tester,
    ) async {
      final bridge = SyntheticBridgePort();
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(bridge),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          profileRowCountProvider.overrideWithValue(50),
        ],
      );
      addTearDown(container.dispose);
      final subs = container.read(subsControllerProvider.notifier);
      subs.save(_synSub('A'));
      subs.save(_synSub('B'));
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));

      final aId = _idOf(container, 'A');
      subs.select(aId);
      await tester.pump();

      // 删除先弹确认框：取消则一切保留。
      await tester.tap(find.byKey(const ValueKey('sub-delete')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('sub-delete-confirm')), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('sub-delete-cancel')));
      await tester.pumpAndSettle();
      expect(container.read(subsControllerProvider).items.length, 2);
      expect(bridge.getSubItem(aId), isNotNull);

      // 确认则删除选中行。
      await tester.tap(find.byKey(const ValueKey('sub-delete')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('sub-delete-confirm')), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('sub-delete-confirm-ok')));
      await tester.pumpAndSettle();
      expect(bridge.getSubItem(aId), isNull);
      expect(
        container.read(subsControllerProvider).items.map((s) => s.remarks),
        contains('B'),
      );
    });
  });
}
