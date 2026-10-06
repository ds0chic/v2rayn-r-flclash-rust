import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

/// SP-21 Dart adoption: the profiles list consumes the async
/// `queryProfilesPageAsync` FRB entry (worker-pool page source, generation
/// echo + `datasetRevision` per page). The synthetic port mirrors the same
/// contract, so these contracts pin the Dart consumption semantics without the
/// native DLL: incremental load with event-loop yields, generation guards +
/// stale-echo drop, synchronous cancel, revision restart, cross-page
/// select-all.
///
/// The base [SyntheticBridgePort] async source only seeds 500 editor DTOs, so
/// these tests walk through a recording fake that serves the full summary
/// window through the async seam (same ids/cursors as the sync slices, real
/// next-cursor, echoed generation). That keeps the no-dup/miss assertions
/// honest at 1200 rows while proving the controller never walks the sync
/// summary seam for the load itself.

/// Full-coverage async fake: serves the whole summary window through the
/// async seam and records every async call. The sync page seam is only
/// counted (never used by the controller under test); the async body reads
/// via `super` so the sync counter stays at zero.
class RecordingAsyncBridge extends SyntheticBridgePort {
  RecordingAsyncBridge({super.count});

  int asyncCalls = 0;
  final List<int> asyncCursors = <int>[];
  final List<int> asyncGenerations = <int>[];
  int syncPageCalls = 0;

  void resetCounts() {
    asyncCalls = 0;
    asyncCursors.clear();
    asyncGenerations.clear();
    syncPageCalls = 0;
  }

  @override
  ProfileSummaryPage querySummaryPage({
    int cursor = 0,
    int pageSize = kProfileQueryPageSize,
    String? text,
    String? subid,
  }) {
    syncPageCalls++;
    return super.querySummaryPage(
      cursor: cursor,
      pageSize: pageSize,
      text: text,
      subid: subid,
    );
  }

  c.ProfileDto _toDto(ProfileSummary s) {
    return c.ProfileDto(
      indexId: s.id,
      configType: s.configType,
      coreType: s.coreType,
      configVersion: 4,
      subid: s.subRemarks,
      isSub: true,
      preSocksPort: null,
      displayLog: true,
      remarks: s.remarks,
      address: s.address,
      port: s.port,
      password: '',
      username: '',
      network: s.network,
      muxEnabled: null,
      finalmask: null,
      security: c.SecurityDto(
        streamSecurity: s.streamSecurity == 'none' ? null : s.streamSecurity,
      ),
      protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );
  }

  @override
  Future<c.ProfilePageDto> queryProfilesPageAsync({
    required c.ProfileFilterDto filter,
    required c.ProfileSortDto sort,
    required int cursor,
    required int pageSize,
    required int requestGeneration,
  }) async {
    asyncCalls++;
    asyncCursors.add(cursor);
    asyncGenerations.add(requestGeneration);
    // Full-window slice via the base sync slices (bypasses the counting
    // override through `super`, so [syncPageCalls] only observes controller
    // sync-seam use, which must stay at zero).
    final sync = super.querySummaryPage(
      cursor: cursor,
      pageSize: pageSize,
      text: filter.text,
      subid: filter.subid,
    );
    final dtos = sync.items.map<c.ProfileDto>(_toDto).toList(growable: false);
    return c.ProfilePageDto(
      items: dtos,
      total: BigInt.from(sync.total),
      nextCursor: sync.nextCursor == null
          ? null
          : BigInt.from(sync.nextCursor!),
      datasetRevision: BigInt.from(profileRevision()),
      requestGeneration: BigInt.from(requestGeneration),
    );
  }
}

/// Async fake that echoes a wrong generation, modelling a stale page that the
/// controller must drop instead of merging.
class StaleGenerationBridge extends RecordingAsyncBridge {
  StaleGenerationBridge({super.count});

  @override
  Future<c.ProfilePageDto> queryProfilesPageAsync({
    required c.ProfileFilterDto filter,
    required c.ProfileSortDto sort,
    required int cursor,
    required int pageSize,
    required int requestGeneration,
  }) async {
    final page = await super.queryProfilesPageAsync(
      filter: filter,
      sort: sort,
      cursor: cursor,
      pageSize: pageSize,
      requestGeneration: requestGeneration,
    );
    return c.ProfilePageDto(
      items: page.items,
      total: page.total,
      nextCursor: page.nextCursor,
      datasetRevision: page.datasetRevision,
      requestGeneration: BigInt.from(requestGeneration + 1000),
    );
  }
}

/// Async fake that bumps the page revision after the first page, modelling a
/// dataset write landing mid-walk. The controller must restart from zero and
/// still cover every row exactly once.
class RevisionBumpBridge extends RecordingAsyncBridge {
  RevisionBumpBridge({super.count});

  int _pages = 0;

  @override
  Future<c.ProfilePageDto> queryProfilesPageAsync({
    required c.ProfileFilterDto filter,
    required c.ProfileSortDto sort,
    required int cursor,
    required int pageSize,
    required int requestGeneration,
  }) async {
    final page = await super.queryProfilesPageAsync(
      filter: filter,
      sort: sort,
      cursor: cursor,
      pageSize: pageSize,
      requestGeneration: requestGeneration,
    );
    final rev = _pages == 0 ? BigInt.zero : BigInt.one;
    _pages++;
    return c.ProfilePageDto(
      items: page.items,
      total: page.total,
      nextCursor: page.nextCursor,
      datasetRevision: rev,
      requestGeneration: page.requestGeneration,
    );
  }
}

ProviderContainer makeAsyncContainer(BridgePort bridge, {int rows = 1200}) {
  return ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(rows),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
}

void main() {
  test('reloadPaged streams every page with no dup or miss', () async {
    final bridge = RecordingAsyncBridge();
    final container = makeAsyncContainer(bridge, rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    bridge.resetCounts();

    final pending = controller.reloadPaged(pageSize: 500);
    expect(
      container.read(profilesControllerProvider).pagedLoading,
      isTrue,
      reason: 'loading flag is set synchronously before the first yield',
    );
    await pending;

    final state = container.read(profilesControllerProvider);
    expect(state.pagedLoading, isFalse);
    expect(state.all.length, 1200);
    final ids = state.all.map((r) => r.id).toList();
    expect(ids.toSet().length, 1200, reason: 'no duplicate rows across pages');
    expect(state.visible.length, 1200);
    expect(state.profiles, isNotEmpty, reason: 'DTO set loads with the pages');
  });

  test('reloadPaged walks through queryProfilesPageAsync', () async {
    final bridge = RecordingAsyncBridge();
    final container = makeAsyncContainer(bridge, rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    bridge.resetCounts();

    await controller.reloadPaged(pageSize: 500);

    expect(bridge.asyncCalls, greaterThan(0), reason: 'async seam is used');
    expect(bridge.asyncCursors.first, 0);
    expect(
      bridge.syncPageCalls,
      0,
      reason: 'summary load never walks the sync page seam',
    );
    expect(
      bridge.asyncGenerations.toSet().length,
      1,
      reason: 'one walk owns one echoed generation',
    );
    final state = container.read(profilesControllerProvider);
    expect(state.all.length, 1200);
    expect(state.all.map((r) => r.id).toSet().length, 1200);
  });

  test('stale-generation async pages are dropped', () async {
    final bridge = StaleGenerationBridge();
    final container = makeAsyncContainer(bridge, rows: 200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    final before = container
        .read(profilesControllerProvider)
        .all
        .map((r) => r.id)
        .toList();
    bridge.resetCounts();

    await controller.reloadPaged(pageSize: 50);

    expect(bridge.asyncCalls, greaterThan(0));
    final after = container.read(profilesControllerProvider);
    expect(after.pagedLoading, isFalse);
    expect(
      after.all.map((r) => r.id).toList(),
      before,
      reason: 'wrong-generation echo never merges a partial walk',
    );
  });

  test('async revision change mid-walk restarts without dup or miss', () async {
    final bridge = RevisionBumpBridge();
    final container = makeAsyncContainer(bridge, rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    bridge.resetCounts();

    await controller.reloadPaged(pageSize: 500);

    expect(bridge.asyncCalls, greaterThan(3));
    final state = container.read(profilesControllerProvider);
    expect(state.pagedLoading, isFalse);
    expect(state.all.length, 1200);
    expect(
      state.all.map((r) => r.id).toSet().length,
      1200,
      reason: 'restarted walk covers every row exactly once',
    );
  });

  test('cancelProfilePageQuery drops late pages and clears the flag', () async {
    final bridge = RecordingAsyncBridge();
    final container = makeAsyncContainer(bridge, rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    bridge.resetCounts();

    final pending = controller.reloadPaged(pageSize: 500);
    // Let the first async page land (incremental publish), then cancel at a
    // safe point: late pages must never overwrite after cancel.
    await Future<void>.delayed(Duration.zero);
    await Future<void>.delayed(Duration.zero);
    controller.cancelProfilePageQuery();
    await pending;

    final partial = container.read(profilesControllerProvider);
    expect(partial.pagedLoading, isFalse);
    expect(
      partial.all.length,
      lessThan(1200),
      reason: 'late pages never overwrite after cancel',
    );

    bridge.resetCounts();
    await controller.reloadPaged(pageSize: 500);
    expect(container.read(profilesControllerProvider).all.length, 1200);
    expect(bridge.asyncCalls, greaterThan(0));
  });

  test('a newer paged query supersedes the in-flight one', () async {
    final bridge = RecordingAsyncBridge();
    final container = makeAsyncContainer(bridge, rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    final stale = controller.reloadPaged(pageSize: 100);
    await controller.reloadPaged(pageSize: 1200);
    await stale;

    final state = container.read(profilesControllerProvider);
    expect(state.pagedLoading, isFalse);
    expect(state.all.length, 1200);
    expect(
      state.all.map((r) => r.id).toSet().length,
      1200,
      reason: 'stale walk left no partial overwrite',
    );
  });

  test('a group switch cancels the in-flight paged walk', () async {
    final bridge = RecordingAsyncBridge();
    final container = makeAsyncContainer(bridge, rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    final pending = controller.reloadPaged(pageSize: 100);
    expect(controller.setGroupSubId(null), isTrue);
    await pending;

    expect(container.read(profilesControllerProvider).pagedLoading, isFalse);
    await controller.reloadPaged(pageSize: 500);
    expect(container.read(profilesControllerProvider).all.length, 1200);
  });

  test(
    'selectAllAcrossPages walks the whole cursor without dup or miss',
    () async {
      final bridge = RecordingAsyncBridge();
      final container = makeAsyncContainer(bridge, rows: 1200);
      addTearDown(container.dispose);
      final controller = container.read(profilesControllerProvider.notifier);
      bridge.resetCounts();

      final ids = await controller.selectAllAcrossPages(pageSize: 500);

      expect(bridge.asyncCalls, greaterThan(0));
      final state = container.read(profilesControllerProvider);
      expect(ids.length, 1200);
      expect(state.selected.length, 1200);
      expect(
        state.selected,
        state.visible.map((r) => r.id).toSet(),
        reason: 'cross-page select matches the current view, nothing hidden',
      );
    },
  );

  test('selectAllAcrossPages cancelled keeps the previous selection', () async {
    final bridge = RecordingAsyncBridge();
    final container = makeAsyncContainer(bridge, rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    final pending = controller.selectAllAcrossPages(pageSize: 100);
    controller.cancelProfilePageQuery();
    final ids = await pending;

    expect(ids, isEmpty);
    expect(container.read(profilesControllerProvider).selected, isEmpty);
  });

  test('async page seam echoes generation and slices honestly', () async {
    // SP-00 integrator closed the FRB gap (`query_profiles_page_async`); the
    // synthetic port mirrors the same contract so the Dart consumption can be
    // pinned without the native DLL.
    final bridge = SyntheticBridgePort(count: 5);
    final first = await bridge.queryProfilesPageAsync(
      filter: c.ProfileFilterDto(
        text: null,
        configTypes: const [],
        subid: null,
      ),
      sort: c.ProfileSortDto.indexId,
      cursor: 0,
      pageSize: 2,
      requestGeneration: 7,
    );
    expect(first.requestGeneration, BigInt.from(7));
    expect(first.datasetRevision, BigInt.from(bridge.profileRevision()));
    expect(first.items.length, 2);
    expect(first.total, BigInt.from(5));
    expect(first.nextCursor, BigInt.from(2));

    final last = await bridge.queryProfilesPageAsync(
      filter: c.ProfileFilterDto(
        text: null,
        configTypes: const [],
        subid: null,
      ),
      sort: c.ProfileSortDto.indexId,
      cursor: 4,
      pageSize: 2,
      requestGeneration: 8,
    );
    expect(last.requestGeneration, BigInt.from(8));
    expect(last.items.length, 1);
    expect(last.nextCursor, isNull, reason: 'end of list is real, not faked');
  });
}
