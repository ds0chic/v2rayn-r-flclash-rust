// RE-PROF-09: the node table's four traffic columns (today/total up/down) must
// be filled from the monitor `ServerStatItem` rows, joined by `IndexId`, not
// left at the hard-coded zero of `dtoToSummary`.
//
// The production read chain is `FrbBridgePort.fetchSummaries` ->
// `monitor.statsSnapshot()` -> `applyNodeStatsOverlay`. Here the same overlay
// runs against a controlled fixture (`SyntheticBridgePort.seedNodeStat`); the
// real SQLite write/reload is proven by the Rust `application` monitor tests.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

ProviderContainer _container(SyntheticBridgePort bridge) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(20),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

ProfileSummary _row(String id) => ProfileSummary(
  id: id,
  configType: ConfigType.vmess,
  remarks: id,
  address: '192.0.2.1',
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

NodeStat _stat(String id, int base) => NodeStat(
  indexId: id,
  todayUp: BigInt.from(base),
  todayDown: BigInt.from(base + 1),
  totalUp: BigInt.from(base + 2),
  totalDown: BigInt.from(base + 3),
);

void main() {
  test('applyNodeStatsOverlay joins ServerStatItem by IndexId', () {
    final rows = [_row('a'), _row('b'), _row('c')];
    final out = applyNodeStatsOverlay(rows, [
      _stat('a', 1000),
      _stat('c', 2000),
    ]);

    final a = out.firstWhere((r) => r.id == 'a');
    expect(a.todayUp, BigInt.from(1000));
    expect(a.todayDown, BigInt.from(1001));
    expect(a.totalUp, BigInt.from(1002));
    expect(a.totalDown, BigInt.from(1003));
    expect(a.remarks, 'a', reason: 'non-traffic columns are preserved');

    final b = out.firstWhere((r) => r.id == 'b');
    expect(
      b.todayUp,
      BigInt.zero,
      reason: 'unmatched row keeps unknown default',
    );
    expect(b.totalDown, BigInt.zero);

    final c = out.firstWhere((r) => r.id == 'c');
    expect(c.totalUp, BigInt.from(2002));
  });

  test('a stat for a deleted id never attaches to a remaining row', () {
    // After a node is deleted the row set no longer contains its id; the join
    // must not re-use that stat for any other node.
    final out = applyNodeStatsOverlay([_row('b')], [_stat('a', 777)]);
    expect(out.single.id, 'b');
    expect(out.single.todayUp, BigInt.zero);
    expect(out.single.totalUp, BigInt.zero);
  });

  test('traffic columns render raw byte counters with 1024-based units', () {
    final out = applyNodeStatsOverlay([_row('a')], [_stat('a', 1024)]);
    expect(formatBytes(out.single.todayUp), '1.0 KB');
  });

  test('node table consumes controlled monitor stats across reloads', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = container
        .read(profilesControllerProvider)
        .visible
        .map((r) => r.id)
        .toList();

    bridge.seedNodeStat(
      ids[0],
      todayUp: 4194304,
      todayDown: 8192,
      totalUp: 1073741824,
      totalDown: 5368709120,
    );
    bridge.statsEnabled = true;
    controller.reload();

    ProfileSummary row() => container
        .read(profilesControllerProvider)
        .visible
        .firstWhere((r) => r.id == ids[0]);

    expect(row().todayUp, BigInt.from(4194304));
    expect(row().todayDown, BigInt.from(8192));
    expect(row().totalUp, BigInt.from(1073741824));
    expect(row().totalDown, BigInt.from(5368709120));

    // A re-read (manual refresh / speedtest poll / reopen) stays consistent.
    controller.reload();
    expect(row().todayUp, BigInt.from(4194304));
    expect(row().totalDown, BigInt.from(5368709120));
  });

  test('statistics disabled leaves the traffic columns untouched', () {
    final bridge = SyntheticBridgePort();
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    final id = container.read(profilesControllerProvider).visible.first.id;

    bridge.seedNodeStat(id, todayUp: 999, totalDown: 999);
    bridge.statsEnabled = false;
    controller.reload();

    final row = container
        .read(profilesControllerProvider)
        .visible
        .firstWhere((r) => r.id == id);
    expect(
      row.todayUp,
      isNot(BigInt.from(999)),
      reason: 'disabled statistics must not surface seeded values',
    );
  });
}
