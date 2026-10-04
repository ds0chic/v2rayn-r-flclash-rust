import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';

import 'support/profiles_harness.dart';

/// RE-PROF-03 empty-selection branch: upstream `Export2ShareUrlAsync` returns
/// silently, while `ShareServerAsync` warns `PleaseSelectServer`; neither opens
/// the share window.
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
    'empty selection keeps upstream Ctrl+C silence and Ctrl+F notice',
    (tester) async {
      final container = await pumpApp(tester, rows: 2);

      await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
      await pressPlain(tester, LogicalKeyboardKey.escape);
      expect(readState(container).selected, isEmpty);

      // Upstream `Export2ShareUrlAsync` returns silently with no selection.
      await pressWithCtrl(tester, LogicalKeyboardKey.keyC);
      await tester.pumpAndSettle();
      expect(clipboard, isEmpty);

      // Upstream `ShareServerAsync` warns `PleaseSelectServer`, no QR window.
      await pressWithCtrl(tester, LogicalKeyboardKey.keyF);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('profile-share-qr')), findsNothing);
      expect(container.read(uiShellControllerProvider).message, '请先选择节点');
    },
  );
}
