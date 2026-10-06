// SP-16/SP-18 独立准备合同（先红后修，合成数据专用）。
//
// 唯一流程（可独立部分）：选 G 后退出重开仍选 G，编辑当前订阅直达 G
// （All/缺失门控无操作）；键鼠/右键/子菜单 Esc/立即 Enter 作用冻结的
// 正确节点集合，不回落首行，不读用户秘密，不触网络/10808。
//
// 原版依据（冻结 7d6a967）：ProfilesViewModel.EditSubAsync(true=新建，
// false=GetSubItem(_config.SubIndexId)，null 直接 return)；
// RefreshServersBiz(361-375)：pending > IndexId > 首行；
// RefreshSubscriptions：SubIndexId 命中恢复，否则 All 首项；
// SubSelectedChangedAsync：_config.SubIndexId = SelectedSub?.Id。
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/sub_entry.dart';

void main() {
  group('SP-16 edit-current entry (synthetic G)', () {
    test('group G resolves to direct edit of G', () {
      final entry = resolveSubEntry(
        groupSubId: 'g-syn-1',
        existingIds: const ['g-syn-1', 'g-syn-2'],
      );
      expect(entry.kind, SubEntryKind.editCurrent);
      expect(entry.targetId, 'g-syn-1');
    });

    test('All view gates edit (upstream no-op)', () {
      final entry = resolveSubEntry(
        groupSubId: null,
        existingIds: const ['g-syn-1'],
      );
      expect(entry.kind, SubEntryKind.gatedAll);
      expect(entry.targetId, isNull);
    });

    test('missing/deleted G gates edit instead of first group', () {
      final entry = resolveSubEntry(
        groupSubId: 'g-gone',
        existingIds: const ['g-syn-1'],
      );
      expect(entry.kind, SubEntryKind.gatedAll);
      expect(entry.targetId, isNull);
    });

    test('blank G gates edit', () {
      final entry = resolveSubEntry(
        groupSubId: '  ',
        existingIds: const ['g-syn-1'],
      );
      expect(entry.kind, SubEntryKind.gatedAll);
    });

    test('create entry never reuses G identity', () {
      final entry = resolveCreateEntry();
      expect(entry.kind, SubEntryKind.createNew);
      expect(entry.targetId, isNull);
    });
  });

  group('SP-18 frozen command targets (synthetic nodes)', () {
    test('Enter/LMB acts on frozen primary, not live first row', () {
      final targets = restoreCommandTargets(
        frozenIds: const ['n2', 'n3'],
        frozenPrimary: 'n2',
        frozenGroup: 'g-syn-1',
        currentGroup: 'g-syn-1',
        visibleIds: const ['n1', 'n2', 'n3'],
      );
      expect(targets, isNotNull);
      expect(targets!.primaryId, 'n2');
      expect(targets.targetIds, const ['n2', 'n3']);
    });

    test('group switch invalidates frozen command', () {
      expect(
        restoreCommandTargets(
          frozenIds: const ['n2'],
          frozenPrimary: 'n2',
          frozenGroup: 'g-syn-1',
          currentGroup: 'g-syn-2',
          visibleIds: const ['n2'],
        ),
        isNull,
      );
    });

    test('hidden primary refuses single-object command', () {
      expect(
        isPrimaryTargetLive(primaryId: 'n9', visibleIds: const ['n1']),
        isFalse,
      );
      expect(
        isPrimaryTargetLive(primaryId: 'n1', visibleIds: const ['n1']),
        isTrue,
      );
    });

    test('Esc closes whole chain (current contract, no per-level guess)', () {
      // 子菜单逐级 Esc 尚未真机对照：当前实现为整链关闭。
      // 本断言锁定该诚实行为，防止未来凭空改成逐级而不留证据。
      expect(menuEscClosesWholeChain, isTrue);
    });
  });
}
