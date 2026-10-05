// R4-30 repro: full language + readable error feedback.
//
// These assertions express the R4-30 contract and intentionally FAIL on the
// pre-fix implementation (the shell only set Material locale; menu/status/error
// text was hard-coded Chinese and the status bar rendered the raw
// `ErrorDto.messageKey`). They are copied from the audit point UFS-17 / D24 in
// docs/evidence/user-flow-audit-2026-10-05/settings-audit.md and are NOT
// adjusted to the current buggy behaviour.
//
// Synthetic only: in-memory provider container / plain widgets. No native
// library, no kernel, no port, no host proxy/TUN/registry, no user data.
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/locale_config.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/shared/l10n/error_localizer.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n_context.dart';
import 'package:v2rayn_desktop/shared/l10n/strings.g.dart';

void main() {
  test('R4-30 repro: product text resolves through the frozen ResUI keys', () {
    // A configured material locale is not enough: the product menu/status text
    // must come from the extracted resource table for each shipped language.
    const en = L10n('en');
    const zh = L10n('zh-Hans');
    expect(en.t('menuServers'), 'Configuration');
    expect(zh.t('menuServers'), '配置项');
    expect(en.t('menuReload'), 'Reload');
    expect(zh.t('menuReload'), '重启服务');
  });

  test(
    'R4-30 repro: missing translation falls back to English, never a key',
    () {
      // `SpeedDisplayText` is absent in zh-Hans upstream; the fallback must be the
      // English source value. An entirely unknown key must still be readable.
      const zh = L10n('zh-Hans');
      expect(zh.t('menuSetting'), isNot('menuSetting'));
      expect(zh.t('totally.unknown.key'), 'totally.unknown.key');
    },
  );

  test('R4-30 repro: bridge error key localizes to cause + action', () {
    const zh = L10n('zh-Hans');
    final localizer = ErrorLocalizer(zh);
    // A raw contract key must never be the visible headline.
    final text = localizer.key('error.revision_stale');
    expect(text, isNot('error.revision_stale'));
    expect(text, isNot(contains('error.')));
    // Unknown error key -> readable generic failure, still no dotted key.
    final unknown = localizer.key('error.some_future_code');
    expect(unknown, isNot(contains('error.')));
  });

  test('R4-30 repro: every menu entry carries a resolvable resource key', () {
    for (final entry in flattenMenu(mainMenuModel)) {
      final key = entry.l10nKey;
      expect(key, isNotNull, reason: 'entry "${entry.label}" has no l10n key');
      for (final language in kL10nLanguages) {
        final resolved = L10n(language).t(key!);
        expect(resolved, isNotEmpty, reason: '$key empty for $language');
        expect(
          resolved,
          isNot(key),
          reason: '$key must resolve for $language (English fallback allowed)',
        );
      }
    }
  });

  testWidgets('R4-30 repro: status bar never renders a raw error key', (
    tester,
  ) async {
    // The runtime error projection carries `error.pac_file_read`; the status bar
    // previously rendered `错误 <code>: <messageKey>` verbatim.
    const dto = RuntimeErrorView(
      code: 'E_TEST',
      messageKey: 'error.pac_file_read',
    );
    final localizer = ErrorLocalizer(const L10n('en'));
    final visible = localizer.key(dto.messageKey);
    expect(visible, isNot(contains('error.')));
    expect(visible, isNot(contains('messageKey')));

    await tester.pumpWidget(
      MaterialApp(
        supportedLocales: kSupportedLanguages,
        localizationsDelegates: const <LocalizationsDelegate<dynamic>>[
          kL10nDelegate,
          ...GlobalMaterialLocalizations.delegates,
        ],
        home: Builder(builder: (context) => Text(context.tr('menuSetting'))),
      ),
    );
    await tester.pump();
    // Scope-less harness renders the upstream default language (zh-Hans).
    expect(find.text('设置'), findsOneWidget);
    expect(find.text('menuSetting'), findsNothing);
  });

  testWidgets('R4-30 repro: provider language drives product text', (
    tester,
  ) async {
    Future<void> pumpLanguage(String? language) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: [productLanguageProvider.overrideWithValue(language)],
          child: MaterialApp(
            supportedLocales: kSupportedLanguages,
            localizationsDelegates: const <LocalizationsDelegate<dynamic>>[
              kL10nDelegate,
              ...GlobalMaterialLocalizations.delegates,
            ],
            home: Builder(builder: (context) => Text(context.tr('menuReload'))),
          ),
        ),
      );
      await tester.pump();
    }

    await pumpLanguage('en');
    expect(find.text('Reload'), findsOneWidget);
    await pumpLanguage('zh-Hans');
    expect(find.text('重启服务'), findsOneWidget);
  });
}
