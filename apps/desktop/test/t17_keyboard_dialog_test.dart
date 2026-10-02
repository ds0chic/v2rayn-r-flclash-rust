import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/shared/widgets/app_dialog.dart';

/// One page build: Enter confirms the focused primary button, Esc dismisses the
/// dialog (WidgetsApp default DismissIntent) and reports cancel.
void main() {
  testWidgets('shared confirm dialog honours Enter=confirm and Esc=cancel', (
    tester,
  ) async {
    final results = <bool?>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: Center(
              child: ElevatedButton(
                onPressed: () async {
                  final value = await showAppConfirmDialog(
                    context,
                    title: '删除节点',
                    message: '确认删除?',
                    confirmLabel: '删除',
                    dialogKey: const ValueKey('confirm'),
                    confirmKey: const ValueKey('confirm-ok'),
                    cancelKey: const ValueKey('confirm-cancel'),
                  );
                  results.add(value);
                },
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    );

    // Enter confirms: the primary button autofocuses so ActivateIntent runs.
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('confirm-ok')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(results, <bool>[true]);
    expect(find.byKey(const ValueKey('confirm-ok')), findsNothing);

    // Esc cancels.
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(results, <bool>[true, false]);
  });
}
