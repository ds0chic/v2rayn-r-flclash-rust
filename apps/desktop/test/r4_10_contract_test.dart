// R4-10 contract: async bridge + slow I/O.
//
// Synthetic only: no native library, no network, no port use, no user data and
// no real disk writes.
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

/// A bridge whose local backup call can be held open, so a test can prove the
/// UI keeps rendering while slow I/O is pending and that cancel discards the
/// result instead of committing it after the fact.
class _SlowBackupBridge extends SyntheticBridgePort {
  _SlowBackupBridge() : super(count: 3);

  Completer<void> localGate = Completer<void>();
  bool failLocal = false;
  int localCalls = 0;

  @override
  Future<c.BackupResultDto> t16BackupLocal(String destRoot) async {
    localCalls++;
    await localGate.future;
    if (failLocal) {
      return const c.BackupResultDto(
        ok: false,
        error: c.ErrorDto(
          code: 'E_INTERNAL',
          messageKey: 'error.backup_failed',
          retryable: true,
        ),
      );
    }
    return super.t16BackupLocal(destRoot);
  }
}

class _Harness extends ConsumerWidget {
  const _Harness();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final busy = ref.watch(backupControllerProvider).busy;
    return MaterialApp(
      home: Scaffold(
        body: Column(
          children: <Widget>[
            if (busy) const LinearProgressIndicator(key: Key('r4_10_busy')),
            Expanded(
              child: ListView.builder(
                itemCount: 40,
                itemBuilder: (_, i) =>
                    SizedBox(height: 40, child: Text('row $i')),
              ),
            ),
            ElevatedButton(
              onPressed: () =>
                  ref.read(backupControllerProvider.notifier).cancel(),
              child: const Text('cancel'),
            ),
          ],
        ),
      ),
    );
  }
}

ProviderContainer _container(BridgePort bridge) {
  final container = ProviderContainer(
    overrides: [bridgePortProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('backup bridge seam is asynchronous, not a sync UI-thread call', () {
    final bridge = SyntheticBridgePort(count: 3);
    expect(bridge.t16BackupLocal('x'), isA<Future<c.BackupResultDto>>());
    expect(bridge.t16BackupList('x'), isA<Future<c.BackupListDto>>());
    expect(bridge.t16BackupVerify('x'), isA<Future<c.VerificationDto>>());
    expect(bridge.t16BackupRestore('x'), isA<Future<c.RestoreResultDto>>());
    expect(bridge.t16BackupRecognize('x'), isA<Future<c.RecognitionDto>>());
    expect(
      bridge.t16BackupImportUpstream('x'),
      isA<Future<c.ImportSummaryDto>>(),
    );
  });

  test('reloadBundles awaits the async list and commits the result', () async {
    final bridge = SyntheticBridgePort(count: 3);
    final container = _container(bridge);
    final notifier = container.read(backupControllerProvider.notifier);
    expect(container.read(backupControllerProvider).bundles, isEmpty);
    await notifier.reloadBundles('parent');
    expect(container.read(backupControllerProvider).bundles, hasLength(1));
  });

  testWidgets(
    'slow local backup keeps frames rendering; cancel discards the result',
    (tester) async {
      final bridge = _SlowBackupBridge();
      final container = _container(bridge);
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const _Harness(),
        ),
      );

      final notifier = container.read(backupControllerProvider.notifier);
      final pending = notifier.localBackup(r'C:\synthetic\dest');
      await tester.pump();
      expect(container.read(backupControllerProvider).busy, isTrue);
      expect(find.byKey(const Key('r4_10_busy')), findsOneWidget);

      // Frames keep rendering and the list still scrolls while the gated I/O
      // is pending: the seam must not block the widget tree.
      await tester.pump(const Duration(milliseconds: 16));
      await tester.drag(find.byType(ListView), const Offset(0, -120));
      await tester.pump();
      expect(find.byType(ListView), findsOneWidget);

      // Cancel: the local generation makes the in-flight result stale.
      notifier.cancel();
      await tester.pump();
      expect(container.read(backupControllerProvider).busy, isFalse);
      expect(find.byKey(const Key('r4_10_busy')), findsNothing);

      // Letting the slow call finish after cancel must not commit anything.
      bridge.localGate.complete();
      await pending;
      await tester.pump();
      final state = container.read(backupControllerProvider);
      expect(state.bundles, isEmpty);
      expect(state.status?.message, '已取消');
    },
  );

  testWidgets('a slow backup failure is readable and can be retried', (
    tester,
  ) async {
    final bridge = _SlowBackupBridge()..failLocal = true;
    final container = _container(bridge);
    await tester.pumpWidget(
      UncontrolledProviderScope(container: container, child: const _Harness()),
    );
    final notifier = container.read(backupControllerProvider.notifier);

    final first = notifier.localBackup(r'C:\synthetic\dest');
    await tester.pump();
    bridge.localGate.complete();
    await first;
    await tester.pump();
    final failed = container.read(backupControllerProvider);
    expect(failed.busy, isFalse);
    expect(failed.status?.isError, isTrue);

    // A failing operation must not latch the window: a retry can still succeed.
    bridge
      ..failLocal = false
      ..localGate = (Completer<void>()..complete());
    await notifier.localBackup(r'C:\synthetic\dest');
    await tester.pump();
    expect(container.read(backupControllerProvider).status?.isSuccess, isTrue);
  });
}
