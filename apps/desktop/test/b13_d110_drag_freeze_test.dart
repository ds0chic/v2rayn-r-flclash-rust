// B13 D1-10: a mid-drag reorder (e.g. the 150ms speedtest poll re-sorting an
// active Delay column) must not retarget the drag endpoint: the gesture
// resolves row positions against the row list frozen at drag start.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('drag endpoint survives a mid-drag reorder', (tester) async {
    final container = await pumpApp(tester, rows: 20);
    final controller = container.read(profilesControllerProvider.notifier);

    // Record screen positions in the initial (ascending) order.
    final from = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000002-Remarks')),
    );
    final to = tester.getCenter(
      find.byKey(const ValueKey('cell-syn-000004-Remarks')),
    );

    final gesture = await tester.startGesture(from);
    await tester.pump();
    expect(readState(container).selected, <String>{
      'syn-000002',
    }, reason: 'pointer-down selects the anchor');

    // Flip the visible order mid-gesture (what a live resort does).
    controller.sortBy('Remarks');
    controller.sortBy('Remarks');
    await tester.pump();
    expect(
      readState(container).visible.first.id,
      'syn-000019',
      reason: 'precondition: order really flipped',
    );

    await gesture.moveTo(to);
    await tester.pump();
    await gesture.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 700));

    expect(readState(container).selected, <String>{
      'syn-000002',
      'syn-000003',
      'syn-000004',
    });
  });
}
