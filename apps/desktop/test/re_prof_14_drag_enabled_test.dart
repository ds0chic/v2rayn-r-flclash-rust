// RE-PROF-14b / P2: with `UiItem.EnableDragDropSort=true` the rows register the
// drag-reorder handlers (upstream `ProfilesView.xaml:27-34`).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('drag-reorder is registered when the setting is true', (
    tester,
  ) async {
    final container = await pumpApp(tester, rows: 12, dragDropSort: true);
    expect(container.read(profilesEnableDragDropSortProvider), isTrue);
    expect(find.byType(Draggable<ProfileSummary>), findsWidgets);
  });
}
