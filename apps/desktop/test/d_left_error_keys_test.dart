// D-LEFT: previously unmapped error keys that reach the UI get human text.
// Keys are a frozen contract: only mappings are added, no renames.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/shared/l10n/error_localizer.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n.dart';
import 'package:v2rayn_desktop/shared/l10n/strings.g.dart';

import 'support/subs_harness.dart';

void main() {
  test('recovery/sub-update/storage keys localize, never raw', () {
    const zh = ErrorLocalizer(L10n('zh-Hans'));
    expect(zh.key('error.recovery_required'), contains('恢复中'));
    expect(zh.key('error.sub_update_failed'), isNot(contains('error.')));
    expect(zh.key('error.storage'), isNot(contains('error.')));
    const en = ErrorLocalizer(L10n('en'));
    expect(
      en.key('error.recovery_required').toLowerCase(),
      contains('recover'),
    );
    expect(en.key('error.sub_update_failed'), isNot(contains('error.')));
    expect(en.key('error.storage'), isNot(contains('error.')));
    for (final lang in kL10nLanguages) {
      final localizer = ErrorLocalizer(L10n(lang));
      for (final key in <String>[
        'error.recovery_required',
        'error.sub_update_failed',
        'error.storage',
      ]) {
        expect(localizer.key(key), isNotEmpty, reason: '$key/$lang empty');
        expect(localizer.key(key), isNot(key), reason: '$key/$lang raw');
        expect(
          localizer.key(key),
          isNot(contains('error.')),
          reason: '$key/$lang',
        );
      }
    }
  });

  testWidgets('sub window renders a localized detail, not the raw key', (
    tester,
  ) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    container
        .read(subsControllerProvider.notifier)
        .setStatus(
          const SubStatus(
            kind: 'error',
            message: '订阅保存失败',
            detail: 'error.sub_update_failed',
          ),
        );
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.textContaining('error.'), findsNothing);
    expect(find.textContaining('订阅保存失败'), findsOneWidget);
  });
}
