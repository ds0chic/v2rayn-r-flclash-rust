// R4-17 subscription protection & stable identity — Dart contract test.
//
// Covers the command-scope half of the card (D12 / UF-PROF-05) that is owned by
// the Flutter layer:
//   * "update current group" freezes the node page's current group, not the
//     subscription-settings selected row;
//   * All (empty group) passes an empty SubIndexId so the backend updates every
//     valid subscription;
//   * a plain group (empty URL) is still targeted and reported as skipped, not
//     faked as success;
//   * a failed update reports the real per-group failure and never removes the
//     existing subscription rows (candidate-first is the backend's job; the UI
//     never invents success).
//
// The IsSub retention, replace range and active/stats remap are verified on the
// Rust side (cargo tests) and by the real-pipeline evidence; this file uses the
// synthetic bridge only, with no network, ports, native library or user data.
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/subs_harness.dart';

class RecordingSubsBridge extends SeededSubsBridge {
  final List<List<String>> calls = <List<String>>[];

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async {
    calls.add(List<String>.of(subIds));
    return super.updateSubscriptions(subIds, viaProxy);
  }
}

/// Fails every targeted subscription with a real per-group error.
class FailingSubsBridge extends RecordingSubsBridge {
  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async {
    calls.add(List<String>.of(subIds));
    final entries = <c.SubUpdateEntryDto>[
      for (final id in subIds.isEmpty ? const <String>[] : subIds)
        c.SubUpdateEntryDto(
          subId: id,
          remarks: id,
          status: 'preserved_error',
          code: 'E_TIMEOUT',
          message: 'error.timeout',
        ),
    ];
    return c.SubUpdateResult(
      ok: false,
      success: 0,
      cancelled: false,
      entries: entries,
      error: const c.ErrorDto(
        code: 'E_TIMEOUT',
        messageKey: 'error.timeout',
        retryable: true,
      ),
    );
  }
}

/// Holds the update open so the test can prove the target is frozen at command
/// start (a later group switch must not retarget the in-flight request).
class DelayedRecordingBridge extends RecordingSubsBridge {
  final Completer<void> release = Completer<void>();

  @override
  Future<c.SubUpdateResult> updateSubscriptions(
    List<String> subIds,
    bool viaProxy,
  ) async {
    calls.add(List<String>.of(subIds));
    await release.future;
    return super.updateSubscriptions(subIds, viaProxy);
  }
}

c.SubItemDto _sub(String remarks, String url) => c.SubItemDto(
  id: '',
  remarks: remarks,
  url: url,
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 0,
  autoUpdateInterval: 0,
  updateTime: 0,
);

Future<void> _pumpProbe(
  WidgetTester tester,
  ProviderContainer container,
  void Function(BuildContext, WidgetRef) onRef,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(home: RefProbe(onRef: (ctx, ref) => onRef(ctx, ref))),
    ),
  );
  await tester.pump();
}

void main() {
  testWidgets('current group wins over the subscription settings selection', (
    tester,
  ) async {
    final bridge = RecordingSubsBridge();
    final a = bridge.saveSubItem(_sub('A', 'https://example.com/a')).item!;
    final b = bridge.saveSubItem(_sub('B', 'https://example.com/b')).item!;
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    container.read(profilesControllerProvider.notifier).setGroupSubId(a.id);
    container.read(subsControllerProvider.notifier).select(b.id);

    BuildContext? context;
    WidgetRef? ref;
    await _pumpProbe(tester, container, (ctx, widgetRef) {
      context = ctx;
      ref = widgetRef;
    });
    await updateCurrentGroup(context!, ref!, viaProxy: false);
    await tester.pump();
    expect(bridge.calls.last, <String>[a.id]);
  });

  testWidgets('All group passes an empty SubIndexId', (tester) async {
    final bridge = RecordingSubsBridge();
    bridge.saveSubItem(_sub('A', 'https://example.com/a'));
    final b = bridge.saveSubItem(_sub('B', 'https://example.com/b')).item!;
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    container.read(profilesControllerProvider.notifier).setGroupSubId(null);
    container.read(subsControllerProvider.notifier).select(b.id);

    BuildContext? context;
    WidgetRef? ref;
    await _pumpProbe(tester, container, (ctx, widgetRef) {
      context = ctx;
      ref = widgetRef;
    });
    await updateCurrentGroup(context!, ref!, viaProxy: false);
    await tester.pump();
    expect(bridge.calls.last, isEmpty);
  });

  testWidgets('the request target is frozen at command start', (tester) async {
    final bridge = DelayedRecordingBridge();
    final a = bridge.saveSubItem(_sub('A', 'https://example.com/a')).item!;
    final b = bridge.saveSubItem(_sub('B', 'https://example.com/b')).item!;
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final profiles = container.read(profilesControllerProvider.notifier);
    profiles.setGroupSubId(a.id);

    BuildContext? context;
    WidgetRef? ref;
    await _pumpProbe(tester, container, (ctx, widgetRef) {
      context = ctx;
      ref = widgetRef;
    });

    final future = updateCurrentGroup(context!, ref!, viaProxy: false);
    await tester.pump();
    // The user switches the group while the download is in flight.
    profiles.setGroupSubId(b.id);
    bridge.release.complete();
    await future;
    await tester.pump();
    expect(bridge.calls.last, <String>[
      a.id,
    ], reason: 'a later group switch must not retarget the in-flight update');
  });

  test('a skipped plain group is never summarized as success', () {
    // The backend reports an empty-URL plain group as `skipped`
    // (`error.url_required`); the shared summary must not turn that into a
    // success, and a skip is not a failure that would hide a real error.
    final result = c.SubUpdateResult(
      ok: false,
      success: 0,
      cancelled: false,
      entries: <c.SubUpdateEntryDto>[
        const c.SubUpdateEntryDto(
          subId: 'sub-plain',
          remarks: '普通分组',
          status: 'skipped',
          message: 'error.url_required',
        ),
      ],
    );
    final summary = subsUpdateSummary(result, viaProxy: false);
    expect(result.success, 0);
    expect(summary, contains('成功 0'));
    expect(summary, contains('跳过 1'));
    expect(hasSubUpdateFailures(result), isFalse);
  });

  testWidgets(
    'a failed update never removes the subscription or fakes success',
    (tester) async {
      final bridge = FailingSubsBridge();
      final a = bridge.saveSubItem(_sub('A', 'https://example.com/a')).item!;
      final container = makeSubsContainer(bridge: bridge);
      addTearDown(container.dispose);
      final subs = container.read(subsControllerProvider.notifier);

      final result = await subs.update(subIds: <String>[a.id], viaProxy: false);
      await tester.pump();

      expect(bridge.calls.last, <String>[a.id]);
      expect(result.ok, isFalse);
      expect(result.success, 0);
      expect(hasSubUpdateFailures(result), isTrue);
      final state = container.read(subsControllerProvider);
      expect(state.items.any((s) => s.id == a.id), isTrue);
      expect(state.status?.isSuccess ?? false, isFalse);
      expect(state.status?.isError ?? false, isTrue);
    },
  );
}
