// UX-SPACE-01 / LAY-PROFILES-002: the localized display label must not change
// the persisted column key. Column order/width/visibility are stored by ExName
// and must survive a reopen.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

void main() {
  test('column layout persists by ExName across reopen after title change', () {
    final store = MemoryUiStateStore();
    ProviderContainer make() => ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
        uiStateStoreProvider.overrideWithValue(store),
        profileRowCountProvider.overrideWithValue(10),
      ],
    );

    final first = make();
    addTearDown(first.dispose);
    final controller = first.read(profilesControllerProvider.notifier);
    controller.resizeColumn('Remarks', 40);
    controller.toggleColumnVisibility('StreamSecurity');
    controller.moveColumn('Port', 1);
    final before = first.read(profilesControllerProvider).columns;
    expect(before.firstWhere((c) => c.key == 'Remarks').title, '别名');

    // The stored section is keyed by the stable ExName, never the label.
    final widths =
        store.loadSection(ProfilesController.columnSection)!['widths'] as Map;
    expect(widths.containsKey('Remarks'), isTrue);
    expect(widths.containsKey('别名'), isFalse);

    // Reopen with the same store: layout must come back unchanged.
    final second = make();
    addTearDown(second.dispose);
    final after = second.read(profilesControllerProvider).columns;
    expect(after.firstWhere((c) => c.key == 'Remarks').width, greaterThan(150));
    expect(after.firstWhere((c) => c.key == 'StreamSecurity').visible, isFalse);
    final keys = after.map((c) => c.key).toList();
    expect(keys.indexOf('Port'), greaterThan(keys.indexOf('Network')));
    expect(after.firstWhere((c) => c.key == 'Remarks').title, '别名');
  });
}
