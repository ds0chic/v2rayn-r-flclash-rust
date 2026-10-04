// RE-PROF-14b / P2: with `UiItem.EnableDragDropSort=false` the node rows must
// not register any drag-reorder handler (upstream only wires drag when the
// setting is true).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('drag-reorder is not registered when the setting is false', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 12, dragDropSort: false);
    expect(container.read(profilesEnableDragDropSortProvider), isFalse);
    expect(find.byType(Draggable<ProfileSummary>), findsNothing);
  });
}
