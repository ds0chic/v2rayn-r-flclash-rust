// SP-18 命令集合（先红后修，合成数据专用）。
//
// 唯一流程（可独立部分）：多选/右键冻结目标、子菜单 Esc 整链关闭、
// 立即 Enter 设为活动，命令作用冻结的正确节点集合，不回落首行。
// 不读用户秘密，不触网络/10808。
//
// 原版依据（冻结 7d6a967，只读 work/）：
// - ProfilesView.xaml.cs LstProfiles_PreviewKeyDown：Enter 即时 SetDefaultServer；
//   Esc 仅 ServerSpeedtestStop（从不清选择）；Ctrl+D 编辑、Ctrl+A 全选。
// - ProfilesViewModel.cs EditSubAsync/RefreshServersBiz/RefreshSubscriptions。
// - 子菜单逐级 Esc 尚未真机对照，当前合同为整链关闭（见 SP-16 证据）。
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/command_context.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart'
    show resolveSingleTarget;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/sub_entry.dart'
    show isPrimaryTargetLive, restoreCommandTargets;
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/shared/widgets/context_menu_session.dart';

import '../support/profiles_harness.dart';

CommandContext _ctx({
  required List<String> ids,
  String? primary,
  String? group,
}) => CommandContext(
  targetIds: ids,
  primaryId: primary,
  groupSubId: group,
  menuOpenPosition: Offset.zero,
  viewContext: ContextMenuRegion.data,
);

void main() {
  group('SP-18 immediate Enter command set (synthetic)', () {
    String? map(LogicalKeyboardKey key, {bool ctrl = false}) =>
        actionForKey(key: key, ctrl: ctrl, shift: false, alt: false);

    test('Enter and numpad Enter both activate immediately', () {
      expect(map(LogicalKeyboardKey.enter), ProfileAction.activate);
      expect(map(LogicalKeyboardKey.numpadEnter), ProfileAction.activate);
    });

    test('Ctrl+D edits, Ctrl+A selects all, Esc stops test only', () {
      expect(map(LogicalKeyboardKey.keyD, ctrl: true), ProfileAction.edit);
      expect(map(LogicalKeyboardKey.keyA, ctrl: true), ProfileAction.selectAll);
      expect(map(LogicalKeyboardKey.escape), ProfileAction.escape);
    });

    test(
      'single-object command prefers frozen primary over live first row',
      () {
        final container = makeContainer(rows: 4);
        addTearDown(container.dispose);
        final state = container.read(profilesControllerProvider);
        final ids = state.visible.map((r) => r.id).toList();
        // Frozen menu target wins even when the live view starts elsewhere.
        expect(resolveSingleTarget(state, ids[2]), ids[2]);
      },
    );

    test('multi-selection acts on its main row, never rejected', () {
      final container = makeContainer(rows: 4);
      addTearDown(container.dispose);
      final controller = container.read(profilesControllerProvider.notifier);
      final ids = container
          .read(profilesControllerProvider)
          .visible
          .map((r) => r.id)
          .toList();
      controller.selectRow(ids[0]);
      controller.selectRow(ids[1], ctrl: true);
      final state = container.read(profilesControllerProvider);
      expect(state.selected.length, 2);
      expect(resolveSingleTarget(state, null), isNotNull);
    });
  });

  group('SP-18 frozen command validation (synthetic)', () {
    test('live context passes, group switch invalidates', () {
      final ok = _ctx(ids: const ['n2'], primary: 'n2', group: 'g-syn-1');
      expect(
        isCommandContextLive(
          command: ok,
          currentGroupSubId: 'g-syn-1',
          visibleIds: const ['n1', 'n2'],
        ),
        isTrue,
      );
      expect(
        isCommandContextLive(
          command: ok,
          currentGroupSubId: 'g-syn-2',
          visibleIds: const ['n1', 'n2'],
        ),
        isFalse,
      );
    });

    test('hidden or empty targets refuse without first-row fallback', () {
      final cmd = _ctx(ids: const ['n9'], primary: 'n9', group: 'g-syn-1');
      expect(
        isCommandContextLive(
          command: cmd,
          currentGroupSubId: 'g-syn-1',
          visibleIds: const ['n1', 'n2'],
        ),
        isFalse,
      );
      expect(
        isCommandContextLive(
          command: _ctx(ids: const <String>[], group: 'g-syn-1'),
          currentGroupSubId: 'g-syn-1',
          visibleIds: const ['n1'],
        ),
        isFalse,
      );
      // Null command never resolves to a substitute row.
      expect(
        isCommandContextLive(
          command: null,
          currentGroupSubId: 'g-syn-1',
          visibleIds: const ['n1'],
        ),
        isFalse,
      );
    });

    test('blank group normalizes to the All view on both sides', () {
      final cmd = _ctx(ids: const ['n1'], primary: 'n1', group: '');
      expect(
        isCommandContextLive(
          command: cmd,
          currentGroupSubId: null,
          visibleIds: const ['n1'],
        ),
        isTrue,
      );
      expect(
        restoreCommandTargets(
          frozenIds: const ['n1'],
          frozenPrimary: 'n1',
          frozenGroup: '',
          currentGroup: null,
          visibleIds: const ['n1'],
        ),
        isNotNull,
      );
    });

    test('hidden primary refuses single-object commands', () {
      expect(
        isPrimaryTargetLive(primaryId: 'n9', visibleIds: const ['n1']),
        isFalse,
      );
      expect(
        isPrimaryTargetLive(primaryId: 'n1', visibleIds: const ['n1']),
        isTrue,
      );
    });
  });
}
