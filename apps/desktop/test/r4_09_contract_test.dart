import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

ProfileSummary _row(int i) => ProfileSummary(
  id: 'id-$i',
  configType: ConfigType.vmess,
  remarks: 'r$i',
  address: '10.0.0.${i % 256}',
  port: 443,
  network: 'raw',
  streamSecurity: '',
  subRemarks: '',
  delay: -1,
  speed: '-',
  todayUp: BigInt.zero,
  ipInfo: '-',
  todayDown: BigInt.zero,
  totalUp: BigInt.zero,
  totalDown: BigInt.zero,
  coreType: CoreType.xray,
);

/// A virtual paged store that never materializes the whole table: each page is
/// produced on demand and the real cursor is advanced, so a >100k store proves
/// the seam follows the store cursor instead of one truncated 100000-row page.
class _VirtualPagedBridge extends SyntheticBridgePort {
  _VirtualPagedBridge(this.virtualTotal);

  final int virtualTotal;
  int pageCalls = 0;
  int maxPageSize = 0;

  @override
  ProfileSummaryPage querySummaryPage({
    int cursor = 0,
    int pageSize = kProfileQueryPageSize,
    String? text,
    String? subid,
  }) {
    pageCalls++;
    if (pageSize > maxPageSize) maxPageSize = pageSize;
    final start = cursor < 0 ? 0 : cursor;
    if (start >= virtualTotal) {
      return ProfileSummaryPage(
        items: const <ProfileSummary>[],
        total: virtualTotal,
        nextCursor: null,
      );
    }
    final end = (start + pageSize).clamp(start, virtualTotal);
    return ProfileSummaryPage(
      items: <ProfileSummary>[for (var i = start; i < end; i++) _row(i)],
      total: virtualTotal,
      nextCursor: end < virtualTotal ? end : null,
    );
  }
}

/// Counting wrapper over the synthetic bridge so the controller's read pattern
/// is observable: a structural read must hit [fetchProfileSnapshot] once, and
/// the speedtest poll must never call it again.
class _CountingBridge extends SyntheticBridgePort {
  int snapshotCalls = 0;
  int profilesCalls = 0;
  int overlayCalls = 0;
  int pageCalls = 0;
  int maxPageSize = 0;
  int activeJobs = 0;

  @override
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  }) {
    snapshotCalls++;
    return super.fetchProfileSnapshot(count, text: text, subid: subid);
  }

  @override
  List<c.ProfileDto> queryAllProfiles() {
    profilesCalls++;
    return super.queryAllProfiles();
  }

  @override
  List<ProfileSummary> applyLiveOverlay(List<ProfileSummary> base) {
    overlayCalls++;
    return super.applyLiveOverlay(base);
  }

  @override
  ProfileSummaryPage querySummaryPage({
    int cursor = 0,
    int pageSize = kProfileQueryPageSize,
    String? text,
    String? subid,
  }) {
    pageCalls++;
    if (pageSize > maxPageSize) maxPageSize = pageSize;
    return super.querySummaryPage(
      cursor: cursor,
      pageSize: pageSize,
      text: text,
      subid: subid,
    );
  }

  @override
  int speedTestActiveJobs() => activeJobs;
}

ProviderContainer _container(BridgePort bridge, {int rows = 100}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(rows),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

ProfilesState _state(ProviderContainer container) =>
    container.read(profilesControllerProvider);

void main() {
  test('querySummaryPage uses the real store cursor across bounded pages', () {
    final bridge = _VirtualPagedBridge(100001);
    final page = bridge.querySummaryPage(cursor: 0, pageSize: 1000);
    expect(page.items.length, 1000);
    expect(page.total, 100001);
    expect(page.nextCursor, 1000, reason: 'cursor comes from the store');
    expect(
      bridge.querySummaryPage(cursor: page.nextCursor!).nextCursor,
      isNotNull,
    );
  });

  test('queryAllSummaries follows the cursor past 100k with no truncation', () {
    final bridge = _VirtualPagedBridge(100001);
    final all = bridge.queryAllSummaries();
    expect(all.length, 100001, reason: 'no silent page-cap truncation');
    expect(all.first.id, 'id-0');
    expect(all.last.id, 'id-100000');
    expect(
      bridge.maxPageSize,
      lessThanOrEqualTo(kProfileQueryPageSize),
      reason: 'page size stays bounded (no 100000 single request)',
    );
    expect(bridge.maxPageSize, lessThan(100000));
    expect(
      bridge.pageCalls,
      (100001 + kProfileQueryPageSize - 1) ~/ kProfileQueryPageSize,
    );
  });

  test('reload reads the profile table once (no double full read)', () {
    final bridge = _CountingBridge();
    final container = _container(bridge, rows: 200);
    container.read(profilesControllerProvider);
    expect(bridge.snapshotCalls, 1, reason: 'build does one structural read');
    expect(bridge.profilesCalls, 0);

    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();
    expect(bridge.snapshotCalls, 2);
    expect(
      bridge.profilesCalls,
      0,
      reason: 'reload must not issue a second full query',
    );
  });

  test('group switch / filter / sort do not re-read the store', () {
    final bridge = _CountingBridge();
    final container = _container(bridge, rows: 300);
    final controller = container.read(profilesControllerProvider.notifier);
    final before = bridge.snapshotCalls;

    controller.setGroupSubId('g1');
    controller.setFilter('r1');
    controller.submitFilter();
    controller.sortBy('Remarks');

    expect(bridge.snapshotCalls, before, reason: 'recompute is local');
    expect(bridge.profilesCalls, 0);
  });

  test('speedtest poll is incremental and does not lose results', () async {
    final bridge = _CountingBridge()..activeJobs = 1;
    final container = _container(bridge, rows: 60);
    final controller = container.read(profilesControllerProvider.notifier);
    final snapshotsBefore = bridge.snapshotCalls;
    final overlaysBefore = bridge.overlayCalls;

    controller.startSpeedTest(ProfileAction.mixedTest);
    final firstId = _state(container).visible.first.id;
    bridge.seedSpeedResult(firstId, 42, 1.5);
    await Future<void>.delayed(const Duration(milliseconds: 400));

    expect(
      bridge.snapshotCalls,
      snapshotsBefore,
      reason: 'the 150 ms poll must not re-read the profile table',
    );
    expect(bridge.overlayCalls, greaterThan(overlaysBefore));
    final row = _state(container).all.firstWhere((r) => r.id == firstId);
    expect(row.delay, 42, reason: 'a fresh result is merged, not lost');

    bridge.activeJobs = 0;
    await Future<void>.delayed(const Duration(milliseconds: 250));
  });

  test('a stale search result cannot overwrite a newer one', () async {
    final bridge = _CountingBridge();
    final container = _container(bridge, rows: 40);
    final controller = container.read(profilesControllerProvider.notifier);

    final first = controller.search('a');
    final second = controller.search('b');
    await Future.wait(<Future<void>>[first, second]);

    expect(_state(container).filter, 'b');
    expect(_state(container).filterInput, 'b');
  });

  test('shift selection across a page boundary keeps the full range', () {
    final bridge = _CountingBridge();
    final container = _container(bridge, rows: 1200);
    final controller = container.read(profilesControllerProvider.notifier);
    final rows = _state(container).visible;
    expect(rows.length, 1200);

    controller.selectRow(rows[400].id);
    controller.selectRow(rows[900].id, shift: true);

    expect(_state(container).selected.length, 501);
    expect(_state(container).selected.contains(rows[400].id), isTrue);
    expect(_state(container).selected.contains(rows[900].id), isTrue);
  });
}
