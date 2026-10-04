import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

// SR-02 / SR-04 targeted UI tests: a failed upstream import must not claim the
// existing data was left untouched when the backend actually had to roll a
// committed database/config/resource set back.

class _FailingImportBridge extends SyntheticBridgePort {
  @override
  c.ImportSummaryDto t16BackupImportUpstream(String path) {
    t16Calls.add('backup_import:$path');
    return c.ImportSummaryDto(
      ok: false,
      status: 'failed',
      sourceVersion: 0,
      importedRows: BigInt.zero,
      warnings: 0,
      errors: 1,
      message: '',
      error: const c.ErrorDto(
        code: 'E_INTERNAL',
        messageKey: 'error.backup_failed',
        retryable: false,
        detail: 'import failed after commit; database/config/resources rolled back to pre-import state',
      ),
    );
  }
}

void main() {
  testWidgets(
    'import failure text reports the actual rollback, not a false claim',
    (tester) async {
      final bridge = _FailingImportBridge();
      final container = ProviderContainer(
        overrides: [bridgePortProvider.overrideWithValue(bridge)],
      );
      addTearDown(container.dispose);
      await tester.binding.setSurfaceSize(const Size(1100, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: BackupAndRestoreView()),
          ),
        ),
      );
      await tester.pump();

      await tester.enterText(
        find.byKey(const ValueKey('backup-archive-field')),
        '/tmp/bad.zip',
      );
      final dynamic button = tester.widget(
        find.byKey(const ValueKey('backup-import-btn')),
      );
      (button.onPressed as VoidCallback).call();
      await tester.pump();

      expect(
        find.textContaining('未修改现有数据'),
        findsNothing,
        reason: 'failure text must not claim existing data was untouched',
      );
      expect(find.textContaining('导入失败'), findsOneWidget);
      expect(find.textContaining('rolled back'), findsOneWidget);
    },
  );
}
