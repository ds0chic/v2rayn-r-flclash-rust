// R4-13.S15 settings-save-to-effect contract (MsgUIItem instance).
//
// MsgUIItem (MainMsgFilter / AutoRefresh) has no consumer outside the domain
// model and timing table: the message-window that would read it is not part of
// this delivery. The stored values persist, but the effect is registered as
// blocked rather than faked. No consumer is asserted here beyond persistence.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('MsgUIItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['MsgUIItem'] as Map<String, dynamic>;
      g['MainMsgFilter'] = 'error';
      g['AutoRefresh'] = true;
    });
    final g = doc['MsgUIItem'] as Map<String, dynamic>;
    expect(g['MainMsgFilter'], 'error');
    expect(g['AutoRefresh'], isTrue);
  });

  test('MsgUIItem defaults stay null (no upstream synthetic value)', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'MsgUIItem': <String, dynamic>{},
            })['MsgUIItem']
            as Map<String, dynamic>;
    expect(g['MainMsgFilter'], isNull);
    expect(g['AutoRefresh'], isNull);
  });
}
