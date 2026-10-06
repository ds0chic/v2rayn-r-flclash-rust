import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

/// SP-21 continuation: the profiles list consumes the existing bounded
/// `querySummaryPage` cursor API asynchronously (incremental load with
/// event-loop yields, generation guards, synchronous cancel, cross-page
/// select-all) against the synthetic bridge. The true background-worker page
/// source needs the `QueryProfilesPageAsync` FRB entry (registered gap,
// SP-00 integrator); these contracts pin the Dart-side consumption semantics.
void main() {
  test('reloadPaged streams every page with no dup or miss', () async {
    final container = makeContainer(rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

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

  test('cancelProfilePageQuery drops late pages and clears the flag', () async {
    final container = makeContainer(rows: 1200);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    final pending = controller.reloadPaged(pageSize: 500);
    // Synchronous cancel: the walk published page one, then must drop.
    controller.cancelProfilePageQuery();
    await pending;

    final partial = container.read(profilesControllerProvider);
    expect(partial.pagedLoading, isFalse);
    expect(
      partial.all.length,
      lessThan(1200),
      reason: 'late pages never overwrite after cancel',
    );

    await controller.reloadPaged(pageSize: 500);
    expect(container.read(profilesControllerProvider).all.length, 1200);
  });

  test('a newer paged query supersedes the in-flight one', () async {
    final container = makeContainer(rows: 1200);
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
    final container = makeContainer(rows: 1200);
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
      final container = makeContainer(rows: 1200);
      addTearDown(container.dispose);
      final controller = container.read(profilesControllerProvider.notifier);

      final ids = await controller.selectAllAcrossPages(pageSize: 500);

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
    final container = makeContainer(rows: 1200);
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
