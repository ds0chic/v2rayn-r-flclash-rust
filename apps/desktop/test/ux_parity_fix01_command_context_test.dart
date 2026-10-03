// FIX-01 / PR-19: the context-menu command target is an immutable snapshot
// captured when the menu opens. These pure tests pin the contract that the
// target cannot be mutated through the source list and that the controller
// only selects generated groups that really exist in the current view.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/command_context.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/shared/widgets/context_menu_session.dart';

import 'support/profiles_harness.dart';

void main() {
  test('CommandContext copies and freezes the captured target ids', () {
    final source = <String>['a', 'b'];
    final context = CommandContext(
      targetIds: source,
      primaryId: 'a',
      groupSubId: 'sub-1',
      menuOpenPosition: const Offset(4, 8),
      viewContext: ContextMenuRegion.data,
    );

    // Mutating the originating list must not retarget the command.
    source.add('c');
    expect(context.targetIds, <String>['a', 'b']);
    // And the stored list is itself unmodifiable.
    expect(() => context.targetIds.add('c'), throwsUnsupportedError);
    expect(context.hasTargets, isTrue);
    expect(context.hasGroup, isTrue);
  });

  test('an empty target/group context reports honestly', () {
    final context = CommandContext(
      targetIds: const <String>[],
      menuOpenPosition: Offset.zero,
      viewContext: ContextMenuRegion.empty,
    );
    expect(context.hasTargets, isFalse);
    expect(context.hasGroup, isFalse);
  });

  test('selectGenerated only selects ids that exist in the view', () {
    final container = makeContainer(rows: 20);
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.reload();

    controller.selectGenerated(<String>['missing', 'syn-000000']);
    expect(container.read(profilesControllerProvider).selected, {'syn-000000'});

    controller.selectGenerated(<String>['still-missing']);
    expect(container.read(profilesControllerProvider).selected, {'syn-000000'});
  });
}
