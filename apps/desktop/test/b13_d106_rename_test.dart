// B13 D1-06: the rename dialog saves through the controller and closes;
// the locally created TextEditingController must be released on every exit
// path (framework dispose contract).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('rename saves remarks and closes the dialog', (tester) async {
    final container = await pumpApp(tester, rows: 5);
    await tapRow(tester, const ValueKey('cell-syn-000001-Remarks'));

    await tester.tap(find.byKey(const ValueKey('toolbar-备注')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('remarks-dialog')), findsOneWidget);

    await tester.enterText(
      find.byKey(const ValueKey('remarks-field')),
      'renamed-node',
    );
    await tester.tap(find.byKey(const ValueKey('remarks-save')));
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('remarks-dialog')), findsNothing);
    expect(
      container.read(uiShellControllerProvider).message,
      '备注已更新',
    );
    final controller = container.read(profilesControllerProvider.notifier);
    expect(controller.profileById('syn-000001')?.remarks, 'renamed-node');
  });

  testWidgets('rename cancel leaves remarks untouched', (tester) async {
    final container = await pumpApp(tester, rows: 5);
    await tapRow(tester, const ValueKey('cell-syn-000001-Remarks'));

    await tester.tap(find.byKey(const ValueKey('toolbar-备注')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('remarks-field')),
      'discarded',
    );
    await tester.tap(find.byKey(const ValueKey('remarks-cancel')));
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('remarks-dialog')), findsNothing);
    final controller = container.read(profilesControllerProvider.notifier);
    expect(
      controller.profileById('syn-000001')?.remarks,
      isNot('discarded'),
    );
  });
}
