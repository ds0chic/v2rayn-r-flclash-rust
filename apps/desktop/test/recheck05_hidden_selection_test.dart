import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

import 'recheck01_group_inheritance_test.dart'
    show StoredBridge, makeStoredContainer;

void main() {
  test('switching group clears a selection that is no longer visible', () {
    final bridge = StoredBridge();
    final container = makeStoredContainer(bridge: bridge);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    final all = container.read(profilesControllerProvider).all;
    final a = all[0].id;
    final b = all[1].id;
    expect(controller.moveProfilesToGroup(<String>[a], 'A'), isTrue);
    expect(controller.moveProfilesToGroup(<String>[b], 'B'), isTrue);

    controller.setGroupSubId('A');
    controller.selectRow(a);
    expect(container.read(profilesControllerProvider).selected, contains(a));

    controller.setGroupSubId('B');
    expect(
      container.read(profilesControllerProvider).selected,
      isNot(contains(a)),
    );
    expect(container.read(profilesControllerProvider).selected, isEmpty);

    controller.selectRow(b);
    controller.setGroupSubId('A');
    expect(
      container.read(profilesControllerProvider).selected,
      isNot(contains(b)),
    );
    expect(container.read(profilesControllerProvider).selected, isEmpty);
  });

  test('text filter clears a selection it hides', () {
    final container = makeStoredContainer();
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    final all = container.read(profilesControllerProvider).all;
    final a = all[0].id;
    final b = all[1].id;
    controller.setGroupSubId(null);
    controller.selectRow(a);
    expect(container.read(profilesControllerProvider).selected, contains(a));

    // Remarks are Synthetic-00000 / Synthetic-00001: this hides node a.
    controller.setFilter('Synthetic-00001');
    final filtered = container.read(profilesControllerProvider);
    expect(filtered.visible.map((r) => r.id), contains(b));
    expect(filtered.visible.map((r) => r.id), isNot(contains(a)));
    expect(filtered.selected, isEmpty);

    controller.setFilter('');
    expect(container.read(profilesControllerProvider).selected, isEmpty);
  });
}
