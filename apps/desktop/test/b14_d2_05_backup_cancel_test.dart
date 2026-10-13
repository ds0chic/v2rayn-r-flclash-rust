import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

/// A local backup that stays in flight until the test releases it.
class _HangingBridge extends SyntheticBridgePort {
  final Completer<c.BackupResultDto> gate = Completer<c.BackupResultDto>();

  @override
  Future<c.BackupResultDto> t16BackupLocal(String destRoot) {
    t16Calls.add('backup_local');
    return gate.future;
  }
}

/// D2-05: a running backup/restore operation must offer a UI cancel entry
/// wired to `BackupController.cancel()`, and a late completion after the
/// cancel must not be reported as success.
void main() {
  testWidgets('busy backup window offers cancel and drops late success', (
    tester,
  ) async {
    final bridge = _HangingBridge();
    final container = ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);

    await tester.binding.setSurfaceSize(const Size(1100, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: BackupAndRestoreView())),
      ),
    );
    await tester.pump();

    await tester.enterText(
      find.byKey(const ValueKey('backup-dest-field')),
      '/tmp/synthetic-backup',
    );
    await tester.tap(find.byKey(const ValueKey('backup-local-btn')));
    await tester.pump();
    expect(container.read(backupControllerProvider).busy, isTrue);
    expect(
      find.byKey(const ValueKey('backup-cancel')),
      findsOneWidget,
      reason: 'busy window must offer a cancel entry',
    );

    await tester.tap(find.byKey(const ValueKey('backup-cancel')));
    await tester.pump();
    expect(container.read(backupControllerProvider).busy, isFalse);
    expect(find.textContaining('已取消'), findsOneWidget);

    // The late engine completion must be discarded, never shown as success.
    bridge.gate.complete(const c.BackupResultDto(ok: true, root: '/tmp/x'));
    await tester.pumpAndSettle();
    expect(find.textContaining('本地备份完成'), findsNothing);
    expect(container.read(backupControllerProvider).busy, isFalse);
  });
}
