import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

/// RE-PROF-03: the node-table keyboard entries must match upstream
/// `ProfilesView.xaml.cs:221-246` — Ctrl+C exports the selected nodes' share
/// URIs (never clones) and Ctrl+F opens the share QR window.
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late List<String> clipboard;

  setUp(() {
    clipboard = <String>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          if (call.method == 'Clipboard.setData') {
            clipboard.add((call.arguments as Map)['text'] as String);
          }
          return null;
        });
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null);
  });

  testWidgets(
    'Ctrl+C batch-exports share URIs without cloning; Ctrl+F opens QR',
    (tester) async {
      final container = await pumpApp(tester, rows: 2);

      // Single selection: Ctrl+C copies the share URI and keeps the row count.
      await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
      final before = readState(container).visible.length;
      await pressWithCtrl(tester, LogicalKeyboardKey.keyC);
      await tester.pumpAndSettle();
      expect(clipboard, hasLength(1));
      expect(clipboard.single, contains('vless://syn-syn-000000'));
      expect(readState(container).visible.length, before);
      expect(readState(container).lastEvent?.action, 'export-share-url');

      // Multiple selection: Ctrl+C exports every selected node in one batch.
      await pressWithCtrlTap(tester, const ValueKey('cell-syn-000001-Remarks'));
      clipboard.clear();
      await pressWithCtrl(tester, LogicalKeyboardKey.keyC);
      await tester.pumpAndSettle();
      expect(clipboard, hasLength(1));
      expect(clipboard.single, contains('vless://syn-syn-000000'));
      expect(clipboard.single, contains('vless://syn-syn-000001'));
      expect(readState(container).visible.length, before);

      // Ctrl+F opens the share QR window for the single selected node.
      await tester.tap(find.byKey(const ValueKey('cell-syn-000000-Remarks')));
      await tester.pump(const Duration(milliseconds: 400));
      await pressWithCtrl(tester, LogicalKeyboardKey.keyF);
      await tester.pumpAndSettle();
      final shareDialog = find.byKey(const ValueKey('profile-share-qr'));
      expect(shareDialog, findsOneWidget);

      await tester.tap(
        find.descendant(of: shareDialog, matching: find.text('关闭')),
      );
      await tester.pumpAndSettle();
      expect(shareDialog, findsNothing);
    },
  );
}
