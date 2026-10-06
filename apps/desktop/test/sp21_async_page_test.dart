import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

/// SP-21 data-layer prep: cancellable async page orchestration.
///
/// The real `QueryProfilesPageAsync` FRB entry is a registered gap (SP-00
/// integrator + SP-16 UI wiring); these contracts pin the Dart-side semantics
/// against a fake page source: frozen request params, immediate cancel, late
/// (superseded) result drop, and cursor invalidation on revision change.
void main() {
  ProfilePageResult page({
    required int generation,
    required int revision,
    required int? nextCursor,
    List<String> ids = const [],
  }) {
    return ProfilePageResult(
      ids: ids,
      nextCursor: nextCursor,
      datasetRevision: revision,
      total: ids.length,
      generation: generation,
    );
  }

  test('cancel returns synchronously and the late page is dropped', () async {
    final pager = AsyncProfilePager();
    final gate = Completer<ProfilePageResult>();
    final generation = pager.nextGeneration();
    final pending = pager.fetchPage(
      cursor: 0,
      pageSize: 200,
      expectedRevision: 7,
      generation: generation,
      fetch: (_) => gate.future,
    );
    // Cancel must be a synchronous call: no await, immediately effective.
    pager.cancelCurrentQuery();
    expect(pager.isCurrent(generation), isFalse);
    gate.complete(page(generation: generation, revision: 7, nextCursor: null));
    expect(await pending, isNull, reason: 'late page after cancel is dropped');
  });

  test(
    'superseded generation result never overwrites the latest view',
    () async {
      final pager = AsyncProfilePager();
      final oldGate = Completer<ProfilePageResult>();
      final oldGeneration = pager.nextGeneration();
      final oldPending = pager.fetchPage(
        cursor: 0,
        pageSize: 200,
        expectedRevision: 3,
        generation: oldGeneration,
        fetch: (_) => oldGate.future,
      );
      final newGeneration = pager.nextGeneration();
      final fresh = await pager.fetchPage(
        cursor: 0,
        pageSize: 200,
        expectedRevision: 3,
        generation: newGeneration,
        fetch: (req) async =>
            page(generation: req.generation, revision: 3, nextCursor: null),
      );
      expect(fresh, isNotNull);
      oldGate.complete(
        page(generation: oldGeneration, revision: 3, nextCursor: 200),
      );
      expect(await oldPending, isNull, reason: 'stale generation is dropped');
    },
  );

  test('cursor from an older revision must restart from zero', () async {
    final pager = AsyncProfilePager();
    final generation = pager.nextGeneration();
    final stale = page(generation: generation, revision: 8, nextCursor: 200);
    expect(
      pager.needsRefetchFromStart(stale, cursor: 200, expectedRevision: 9),
      isTrue,
    );
    expect(
      pager.needsRefetchFromStart(stale, cursor: 200, expectedRevision: 8),
      isFalse,
    );
    expect(
      pager.needsRefetchFromStart(
        page(generation: generation, revision: 9, nextCursor: null),
        cursor: 0,
        expectedRevision: 8,
      ),
      isFalse,
      reason: 'a fresh query adopts the current revision',
    );
  });

  test('request freezes filter/sort/cursor at call time', () async {
    final pager = AsyncProfilePager();
    ProfilePageRequest? seen;
    final generation = pager.nextGeneration();
    await pager.fetchPage(
      cursor: 40,
      pageSize: 50,
      expectedRevision: 1,
      generation: generation,
      filterText: 'hk',
      subid: 's-1',
      sortKey: 'Remarks',
      fetch: (req) async {
        seen = req;
        return page(generation: req.generation, revision: 1, nextCursor: null);
      },
    );
    expect(seen?.cursor, 40);
    expect(seen?.filterText, 'hk');
    expect(seen?.subid, 's-1');
    expect(seen?.sortKey, 'Remarks');
    expect(seen?.generation, generation);
  });
}
