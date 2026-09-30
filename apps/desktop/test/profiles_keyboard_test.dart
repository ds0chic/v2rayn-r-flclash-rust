import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('ctrl and plain keyboard actions map to distinct events', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 20);
    final controller = container.read(profilesControllerProvider.notifier);

    await pressWithCtrl(tester, LogicalKeyboardKey.keyD);
    expect(readState(container).lastEvent?.action, 'edit');

    await pressWithCtrl(tester, LogicalKeyboardKey.keyF);
    expect(readState(container).lastEvent?.action, 'share');

    await pressWithCtrl(tester, LogicalKeyboardKey.keyO);
    expect(readState(container).lastEvent?.action, 'tcping');

    await pressWithCtrl(tester, LogicalKeyboardKey.keyR);
    expect(readState(container).lastEvent?.action, 'realping');

    await pressWithCtrl(tester, LogicalKeyboardKey.keyT);
    expect(readState(container).lastEvent?.action, 'speedtest');

    await pressWithCtrl(tester, LogicalKeyboardKey.keyE);
    expect(readState(container).lastEvent?.action, 'mixed-test');

    await pressWithCtrl(tester, LogicalKeyboardKey.keyC);
    expect(readState(container).lastEvent?.action, 'copy');

    await pressPlain(tester, LogicalKeyboardKey.enter);
    expect(readState(container).lastEvent?.action, 'activate');

    await pressPlain(tester, LogicalKeyboardKey.delete);
    expect(readState(container).lastEvent?.action, 'delete');

    await pressPlain(tester, LogicalKeyboardKey.backspace);
    expect(readState(container).lastEvent?.action, 'delete');

    await pressPlain(tester, LogicalKeyboardKey.keyT);
    expect(readState(container).lastEvent?.action, 'move-top');

    await pressPlain(tester, LogicalKeyboardKey.keyU);
    expect(readState(container).lastEvent?.action, 'move-up');

    await pressPlain(tester, LogicalKeyboardKey.keyD);
    expect(readState(container).lastEvent?.action, 'move-down');

    await pressPlain(tester, LogicalKeyboardKey.keyB);
    expect(readState(container).lastEvent?.action, 'move-bottom');

    await pressPlain(tester, LogicalKeyboardKey.escape);
    expect(readState(container).lastEvent?.action, 'escape');

    // The controller performed no business mutation.
    controller.selectAll();
    expect(readState(container).visible.length, 20);
  });
}
