// Audit round 2 regressions: node-table ordering inside a group / under a
// hidden sort column, subscription refresh of the server list, and the status
// bar inbound text.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'recheck01_group_inheritance_test.dart'
    show StoredBridge, makeStoredContainer;
import 'support/subs_harness.dart';

List<String> _visibleIds(ProviderContainer container) => container
    .read(profilesControllerProvider)
    .visible
    .map((r) => r.id)
    .toList();

/// Seeded subscription bridge that counts node-table snapshot reads and can
/// simulate the Rust scheduler touching a subscription's `UpdateTime`.
class _CountingSubsBridge extends SeededSubsBridge {
  int snapshotReads = 0;
  bool schedulerRunning = false;

  @override
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  }) {
    snapshotReads++;
    return super.fetchProfileSnapshot(count, text: text, subid: subid);
  }

  @override
  bool subSchedulerRunning() => schedulerRunning;

  @override
  c.SimpleResult startSubScheduler() {
    schedulerRunning = true;
    return const c.SimpleResult(ok: true);
  }

  @override
  c.SimpleResult stopSubScheduler() {
    schedulerRunning = false;
    return const c.SimpleResult(ok: true);
  }

  void touchUpdateTime(String id, int stamp) {
    final item = listSubItems().items.firstWhere((s) => s.id == id);
    saveSubItem(
      c.SubItemDto(
        id: item.id,
        remarks: item.remarks,
        url: item.url,
        moreUrl: item.moreUrl,
        enabled: item.enabled,
        userAgent: item.userAgent,
        sort: item.sort,
        autoUpdateInterval: item.autoUpdateInterval,
        updateTime: stamp,
      ),
    );
  }
}

void main() {
  group('node ordering', () {
    test('move up inside a group moves across the visible rows only', () {
      final bridge = StoredBridge();
      final container = makeStoredContainer(bridge: bridge);
      addTearDown(container.dispose);
      final controller = container.read(profilesControllerProvider.notifier);

      final all = container.read(profilesControllerProvider).all;
      final a1 = all[0].id;
      final b1 = all[1].id;
      final a2 = all[2].id;
      expect(controller.moveProfilesToGroup(<String>[a1, a2], 'A'), isTrue);
      expect(controller.moveProfilesToGroup(<String>[b1], 'B'), isTrue);
      controller.setGroupSubId('A');
      expect(_visibleIds(container), <String>[a1, a2]);

      controller.selectRow(a2);
      bridge.appliedProfileOrders.clear();
      controller.emitAction(ProfileAction.moveUp);

      expect(_visibleIds(container), <String>[a2, a1]);
      expect(bridge.appliedProfileOrders.last, <String>[a2, a1]);
    });

    test('move up under an active sort starts from the displayed order', () {
      final bridge = SyntheticBridgePort();
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(bridge),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          profileRowCountProvider.overrideWithValue(6),
        ],
      );
      addTearDown(container.dispose);
      final controller = container.read(profilesControllerProvider.notifier);

      controller.sortBy('Remarks');
      controller.sortBy('Remarks');
      final shown = _visibleIds(container);
      controller.selectRow(shown[2]);
      controller.emitAction(ProfileAction.moveUp);

      final expected = List<String>.of(shown)
        ..removeAt(2)
        ..insert(1, shown[2]);
      expect(_visibleIds(container), expected);
    });

    test('sort by result uses DelayVal even when that column is hidden', () {
      final bridge = SyntheticBridgePort();
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(bridge),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          profileRowCountProvider.overrideWithValue(6),
        ],
      );
      addTearDown(container.dispose);
      final controller = container.read(profilesControllerProvider.notifier);
      final rows = container.read(profilesControllerProvider).visible;
      bridge.seedSpeedResult(rows[3].id, 50, 1.0);
      bridge.seedSpeedResult(rows[4].id, 20, 1.0);
      controller.reload();
      controller.toggleColumnVisibility('DelayVal');
      expect(
        container
            .read(profilesControllerProvider)
            .visibleColumns
            .any((col) => col.key == 'DelayVal'),
        isFalse,
      );

      controller.sortByResult();

      final ordered = _visibleIds(container);
      expect(ordered.take(2), <String>[rows[4].id, rows[3].id]);
    });

    test('sort by result persists the whole group, not the filtered rows', () {
      final bridge = SyntheticBridgePort();
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(bridge),
          uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
          profileRowCountProvider.overrideWithValue(6),
        ],
      );
      addTearDown(container.dispose);
      final controller = container.read(profilesControllerProvider.notifier);
      final rows = container.read(profilesControllerProvider).visible;
      controller.setFilter(rows.first.remarks);
      controller.submitFilter();
      expect(container.read(profilesControllerProvider).visible.length, 1);
      bridge.appliedProfileOrders.clear();

      controller.sortByResult();

      expect(bridge.appliedProfileOrders.last.length, rows.length);
    });
  });

  group('subscription refresh', () {
    test('a single-subscription update refreshes the server list', () async {
      final bridge = _CountingSubsBridge();
      final container = makeSubsContainer(bridge: bridge);
      addTearDown(container.dispose);
      container.read(profilesControllerProvider);
      final subs = container.read(subsControllerProvider.notifier);
      final id = container.read(subsControllerProvider).items.single.id;
      final before = bridge.snapshotReads;

      await subs.update(subIds: <String>[id]);

      expect(bridge.snapshotReads, greaterThan(before));
    });

    testWidgets('a scheduled update refreshes subs and the server list', (
      tester,
    ) async {
      final bridge = _CountingSubsBridge();
      final container = makeSubsContainer(bridge: bridge);
      addTearDown(container.dispose);
      container.read(profilesControllerProvider);
      final subs = container.read(subsControllerProvider.notifier);
      final id = container.read(subsControllerProvider).items.single.id;
      subs.startScheduler(silent: true);

      // A quiet tick reloads nothing.
      var reads = bridge.snapshotReads;
      await tester.pump(SubsController.schedulerWatchPeriod);
      expect(bridge.snapshotReads, reads);

      // The Rust scheduler touched UpdateTime: the next tick reloads both.
      bridge.touchUpdateTime(id, 1700000000);
      await tester.pump(SubsController.schedulerWatchPeriod);
      expect(bridge.snapshotReads, greaterThan(reads));
      expect(
        container.read(subsControllerProvider).items.single.updateTime,
        1700000000,
      );

      // Stopping the scheduler stops the watch.
      subs.stopScheduler(silent: true);
      reads = bridge.snapshotReads;
      bridge.touchUpdateTime(id, 1700000060);
      await tester.pump(SubsController.schedulerWatchPeriod);
      expect(bridge.snapshotReads, reads);
    });
  });

  group('status bar inbound', () {
    Map<String, dynamic> doc(Map<String, dynamic> inbound) => <String, dynamic>{
      'Inbound': <dynamic>[inbound],
    };

    test('local shows mixed and the optional socks2 port', () {
      expect(
        inboundDisplay(doc(<String, dynamic>{'LocalPort': 11808})).local,
        '[mixed:11808]',
      );
      expect(
        inboundDisplay(
          doc(<String, dynamic>{
            'LocalPort': 11808,
            'SecondLocalPortEnabled': true,
          }),
        ).local,
        '[mixed:11808,socks2:11809]',
      );
    });

    test('LAN is none unless allowed, then the shared or +2 port', () {
      expect(
        inboundDisplay(doc(<String, dynamic>{'LocalPort': 11808})).lan,
        'none',
      );
      expect(
        inboundDisplay(
          doc(<String, dynamic>{'LocalPort': 11808, 'AllowLANConn': true}),
        ).lan,
        '[mixed:11808]',
      );
      expect(
        inboundDisplay(
          doc(<String, dynamic>{
            'LocalPort': 11808,
            'AllowLANConn': true,
            'NewPort4LAN': true,
          }),
        ).lan,
        '[mixed:11810]',
      );
    });

    test('a document without a usable inbound shows --', () {
      final display = inboundDisplay(const <String, dynamic>{});
      expect(display.local, '--');
      expect(display.lan, '--');
    });
  });

  test('preserved_error counts as failed, not also as preserved', () async {
    final container = makeSubsContainer(bridge: _PreservedErrorBridge());
    addTearDown(container.dispose);

    await container.read(subsControllerProvider.notifier).update();

    expect(
      container.read(subsControllerProvider).status?.message,
      '更新完成：成功 1，失败 1',
    );
  });
}

class _PreservedErrorBridge extends SeededSubsBridge {
  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async => const c.SubUpdateResult(
    ok: true,
    success: 1,
    cancelled: false,
    entries: <c.SubUpdateEntryDto>[
      c.SubUpdateEntryDto(subId: 'a', remarks: 'a', status: 'updated'),
      c.SubUpdateEntryDto(subId: 'b', remarks: 'b', status: 'preserved_error'),
    ],
  );
}
