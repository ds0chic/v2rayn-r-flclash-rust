import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/profiles_harness.dart';

/// HKR-02 regression lock (UX1-08): with the filter box focused, Ctrl+C must
/// serve the text field (native copy), never the table's export-share-url
/// action. No production change in U-B1: the menu-open guard already swallows
/// every key while the context menu is up (profiles_table _onKey), modal
/// dialogs move focus away from the table, and the ledger (ACT-PROF-015 /
/// UX-SPACE-02) forbids adding the mixed-test row to the context menu, so the
/// only remaining gap was this untested guard.
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

  testWidgets('Ctrl+C in the filter box copies text, never exports', (
    tester,
  ) async {
    await pumpApp(tester, rows: 2);
    // Positive control: with the table focused, Ctrl+C exports the selection.
    await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
    await pressWithCtrl(tester, LogicalKeyboardKey.keyC);
    await tester.pumpAndSettle();
    expect(clipboard, hasLength(1));
    expect(clipboard.single, contains('vless://'));
    // Guard: moving focus into the filter box must silence the export, even
    // with a live selection behind it.
    clipboard.clear();
    await tester.enterText(find.byKey(const ValueKey('filter-field')), 'syn');
    await tester.pump();
    await pressWithCtrl(tester, LogicalKeyboardKey.keyC);
    await tester.pumpAndSettle();
    expect(clipboard, isEmpty);
  });
}
