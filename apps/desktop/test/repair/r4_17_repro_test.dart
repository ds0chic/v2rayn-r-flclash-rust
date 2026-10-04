// R4-17 repro: "update current subscription" must target the node page's
// current group, not the subscription-settings selected row (D12 / UF-PROF-05),
// and the All group must update every valid subscription (empty sub_ids).
//
// These assertions express the R4-17 contract and intentionally fail on the
// pre-fix implementation, which reads `subsState.selected`. Synthetic only:
// in-memory bridge, no native library, no network, no user data, no ports.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import '../support/subs_harness.dart';

/// Records every `updateSubscriptions` target set so the test can assert the
/// exact scope the command captured.
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
  testWidgets(
    'update current group targets the node-page group, not the subs selection',
    (tester) async {
      final bridge = RecordingSubsBridge();
      final a = bridge.saveSubItem(_sub('A', 'https://example.com/a')).item!;
      final b = bridge.saveSubItem(_sub('B', 'https://example.com/b')).item!;

      final container = makeSubsContainer(bridge: bridge);
      addTearDown(container.dispose);
      // Node page shows group A; the subscription window selected B instead.
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

      expect(
        bridge.calls,
        isNotEmpty,
        reason: 'the command must issue an update request',
      );
      expect(bridge.calls.last, <String>[
        a.id,
      ], reason: 'must update the node-page current group, not subs.selected');
    },
  );

  testWidgets(
    'update current group on the All group targets every subscription',
    (tester) async {
      final bridge = RecordingSubsBridge();
      bridge.saveSubItem(_sub('A', 'https://example.com/a')).item;
      final b = bridge.saveSubItem(_sub('B', 'https://example.com/b')).item!;

      final container = makeSubsContainer(bridge: bridge);
      addTearDown(container.dispose);
      // All group = no group filter (null), while the subs window still has a
      // stale selection; upstream "current" with an empty SubIndexId updates all.
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

      expect(
        bridge.calls.last,
        isEmpty,
        reason:
            'All passes an empty SubIndexId so every valid subscription runs',
      );
    },
  );
}
