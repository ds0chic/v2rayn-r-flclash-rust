import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

import 'recheck01_group_inheritance_test.dart' show StoredBridge;
import 'support/counting_runtime_bridge.dart';

class _Probe extends ConsumerWidget {
  const _Probe({required this.onPressed});

  final void Function(BuildContext context, WidgetRef ref) onPressed;

  @override
  Widget build(BuildContext context, WidgetRef ref) => Material(
    child: TextButton(
      onPressed: () => onPressed(context, ref),
      child: const Text('go'),
    ),
  );
}

ProviderContainer makeActiveContainer(
  BridgePort bridge,
  CountingRuntimeBridge runtime,
) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    runtimeBridgeProvider.overrideWithValue(runtime),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(20),
  ],
);

void main() {
  test(
    'nextActiveAfterRemoval picks the first Port>0 node of the current view',
    () {
      const empty = ProfilesState(
        all: <ProfileSummary>[],
        visible: <ProfileSummary>[],
        filter: '',
        sort: SortSpec(),
        selected: <String>{},
        events: <TableEvent>[],
        doubleClick2Activate: false,
        rustCount: 0,
        columns: <ProfileColumn>[],
      );
      expect(nextActiveAfterRemoval(empty), isNull);
    },
  );

  testWidgets('editing the active node re-applies once; non-active does not', (
    tester,
  ) async {
    final bridge = StoredBridge();
    final runtime = CountingRuntimeBridge();
    final container = makeActiveContainer(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(profilesControllerProvider.notifier);
    final all = container.read(profilesControllerProvider).all;
    final a = all[0].id;
    final b = all[1].id;
    controller.setActive(a);

    var target = a;
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: _Probe(
            onPressed: (context, ref) {
              applyAfterEditIfActive(ref, target);
            },
          ),
        ),
      ),
    );

    await tester.tap(find.text('go'));
    await tester.pumpAndSettle();
    expect(runtime.applyCalls, 1);

    target = b;
    await tester.tap(find.text('go'));
    await tester.pumpAndSettle();
    expect(runtime.applyCalls, 1, reason: 'non-active edit must not apply');
  });

  testWidgets('deleting the active node applies a fallback once', (
    tester,
  ) async {
    final bridge = StoredBridge();
    final runtime = CountingRuntimeBridge();
    final container = makeActiveContainer(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(profilesControllerProvider.notifier);
    final all = container.read(profilesControllerProvider).all;
    final a = all[0].id;
    controller.setActive(a);
    controller.selectRow(a);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: _Probe(
            onPressed: (context, ref) {
              deleteSelectedProfiles(context, ref);
            },
          ),
        ),
      ),
    );

    await tester.tap(find.text('go'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('delete-confirm-ok')));
    await tester.pumpAndSettle();

    expect(bridge.getProfile(a), isNull);
    expect(runtime.applyCalls, 1);
    final active = container.read(profilesControllerProvider).activeId;
    expect(active, isNotNull);
    expect(active, isNot(a));
    expect(bridge.getProfile(active!)!.port, greaterThan(0));
  });

  testWidgets('deleting a non-active node never applies', (tester) async {
    final bridge = StoredBridge();
    final runtime = CountingRuntimeBridge();
    final container = makeActiveContainer(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(profilesControllerProvider.notifier);
    final all = container.read(profilesControllerProvider).all;
    final a = all[0].id;
    final b = all[1].id;
    controller.setActive(a);
    controller.selectRow(b);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: _Probe(
            onPressed: (context, ref) {
              deleteSelectedProfiles(context, ref);
            },
          ),
        ),
      ),
    );

    await tester.tap(find.text('go'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('delete-confirm-ok')));
    await tester.pumpAndSettle();

    expect(runtime.applyCalls, 0);
    expect(container.read(profilesControllerProvider).activeId, a);
  });
}
