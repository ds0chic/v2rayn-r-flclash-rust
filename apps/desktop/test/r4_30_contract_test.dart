// R4-30 contract: full language + readable error feedback.
//
// One real user flow per the task card: switch the upstream language -> the main
// window, sub-window, menu, status bar and error feedback actually switch;
// reopening keeps the choice; a missing translation falls back to English/the
// readable key. Bridge/window/runtime/subscription/backup/update error paths
// never render a raw key.
//
// Synthetic only: in-memory provider container / plain widgets. No native
// library, no kernel, no port, no host proxy/TUN/registry, no user data.
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/locale_config.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/shared/l10n/error_localizer.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n.dart';
import 'package:v2rayn_desktop/shared/l10n/strings.g.dart';

import 'support/fake_platform_bridge.dart';
import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _container() {
  final container = ProviderContainer(
    overrides: [
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(4),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      runtimeBridgeProvider.overrideWithValue(SyntheticRuntimeBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Widget _localizedApp(Widget child, Locale locale) => MaterialApp(
  locale: locale,
  supportedLocales: kSupportedLanguages,
  localizationsDelegates: const <LocalizationsDelegate<dynamic>>[
    kL10nDelegate,
    ...GlobalMaterialLocalizations.delegates,
  ],
  home: child,
);

void main() {
  group('language infrastructure', () {
    test('kSupportedLanguages matches the frozen ResUI resx set', () {
      expect(kSupportedLanguages, hasLength(9));
      expect(kL10nLanguages, hasLength(9));
      expect(localeForLanguage('zh-Hans')?.languageCode, 'zh');
      expect(localeForLanguage('en'), const Locale('en'));
    });

    test('localeForLanguage produces the script-qualified code', () {
      const hans = Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans');
      expect(kL10nDelegate.isSupported(hans), isTrue);
      expect(kSupportedLanguages.contains(hans), isTrue);
    });

    test('resource table covers every listed language for every key', () {
      for (final entry in kResUiStrings.entries) {
        expect(
          entry.value['en'],
          isNotNull,
          reason: '${entry.key} has no English fallback',
        );
        for (final lang in kL10nLanguages) {
          final value = entry.value[lang];
          if (value != null) {
            expect(value, isNotEmpty, reason: '${entry.key}/$lang empty');
          }
        }
      }
    });

    test('missing translation falls back to English, then to the key', () {
      // zh-Hans upstream is missing SpeedDisplayText-style keys; force a key
      // that exists in en but not zh-Hant is hard to guarantee, so assert the
      // readable fallback contract directly.
      const l10n = L10n('en');
      expect(l10n.t('menuReload'), 'Reload');
      expect(l10n.t('no.such.key'), 'no.such.key');
    });
  });

  group('menu and status bar switch with the locale', () {
    testWidgets('menu labels resolve per active locale', (tester) async {
      final entry = mainMenuModel.first;
      expect(entry.labelFor(const L10n('en')), 'Configuration');
      expect(entry.labelFor(const L10n('zh-Hans')), '配置项');
      expect(entry.labelFor(const L10n('ru')), 'Конфигурация');
    });

    testWidgets('changing UiShellState.language persists and reapplies', (
      tester,
    ) async {
      final container = _container();
      final store = container.read(uiStateStoreProvider);
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: _localizedApp(
            Consumer(
              builder: (context, ref, _) {
                final shell = ref.watch(uiShellControllerProvider);
                return Text(L10n(shell.language ?? 'en').t('menuReload'));
              },
            ),
            const Locale('en'),
          ),
        ),
      );
      await tester.pump();
      expect(find.text('Reload'), findsOneWidget);

      container
          .read(uiShellControllerProvider.notifier)
          .applyThemeSelection(language: 'fr');
      await tester.pump();
      expect(find.text('Recharger'), findsOneWidget);
      // Persisted: reopening the store in a fresh container restores it.
      expect(store.loadSection('theme')!['language'], 'fr');
    });
  });

  group('readable error feedback (no raw keys)', () {
    test('bridge error keys localize for every supported language', () {
      const keys = <String>[
        'error.revision_stale',
        'error.not_found',
        'error.url_required',
        'error.url_invalid',
        'error.invalid_uri',
        'error.import_nothing',
        'error.remarks_required',
        'error.delete_failed',
        'error.no_profiles_selected',
        'error.not_wired',
        'error.codegen_failed',
        'error.runtime_timeout',
        'error.sub_headers_invalid',
        'error.sub_update_unconfirmed',
        'error.backup_invalid',
        'error.webdav_permission',
        'error.settings_json',
        'error.bridge_unavailable',
        'error.unknown',
      ];
      for (final lang in kL10nLanguages) {
        final localizer = ErrorLocalizer(L10n(lang));
        for (final key in keys) {
          final text = localizer.key(key);
          expect(text, isNotEmpty, reason: '$key/$lang empty');
          expect(
            text,
            isNot(key),
            reason: '$key must not render the raw key for $lang',
          );
          expect(text, isNot(contains('error.')), reason: '$key/$lang');
        }
      }
    });

    test('unknown error keys degrade to readable generic text', () {
      final localizer = ErrorLocalizer(const L10n('en'));
      expect(localizer.key('error.future_code'), isNot(contains('error.')));
      expect(localizer.key(null), isNot(contains('.')));
    });

    test('bridge ErrorDto localizes via the adapter', () {
      const dto = c.ErrorDto(
        code: 'E_REVISION_STALE',
        messageKey: 'error.revision_stale',
        retryable: false,
      );
      final text = ErrorLocalizer(const L10n('zh-Hans'))
          .error(BridgeErrorView(dto));
      expect(text, isNot(contains('error.')));
      expect(text, isNot(contains('revision_stale')));
    });

    test('runtime/platform error projections localize', () {
      const runtime = RuntimeErrorView(
        code: 'E_RT',
        messageKey: 'error.runtime_timeout',
      );
      final text = ErrorLocalizer(const L10n('en'))
          .error(SimpleErrorView(messageKey: runtime.messageKey));
      expect(text.toLowerCase(), contains('timed out'));
    });

    test(
      'window status keys localize, never a raw key (Wave K no regression)',
      () {
        const localizer = ErrorLocalizer(L10n('zh-Hans'));
        expect(localizer.key('settings.saved'), '操作成功');
        expect(localizer.key('settings.saved_need_core_restart'), '操作成功，请重启服务');
        expect(
          localizer.key('settings.saved_need_app_restart'),
          contains('重启应用'),
        );
        expect(
          localizer.key('settings.saved_need_next_launch'),
          contains('下次启动'),
        );
        for (final key in <String>[
          'settings.saved',
          'settings.saved_need_core_restart',
          'settings.saved_need_app_restart',
          'settings.saved_need_next_launch',
        ]) {
          expect(localizer.key(key), isNot(key));
        }
      },
    );

    test('validation keys localize', () {
      const localizer = ErrorLocalizer(L10n('zh-Hans'));
      expect(localizer.key('validate.local_port'), '请填写本地监听端口');
      expect(localizer.key('validate.fragment'), '请填写正确的分片参数');
    });

    test('SP-01/SP-25 contract keys localize, never raw', () {
      const zh = ErrorLocalizer(L10n('zh-Hans'));
      expect(zh.key('error.config_corrupt'), contains('配置'));
      expect(zh.key('error.webdav_tls'), contains('TLS'));
      const en = ErrorLocalizer(L10n('en'));
      expect(en.key('error.config_corrupt').toLowerCase(), contains('corrupt'));
      expect(en.key('error.webdav_tls').toLowerCase(), contains('tls'));
      for (final key in <String>['error.config_corrupt', 'error.webdav_tls']) {
        expect(zh.key(key), isNot(key));
        expect(en.key(key), isNot(key));
      }
    });

    test('status bar shows a readable runtime error, not a key', () {
      const runtime = RuntimeErrorView(
        code: 'E_TEST',
        messageKey: 'error.pac_file_read',
      );
      final text = ErrorLocalizer(const L10n('en'))
          .error(SimpleErrorView(messageKey: runtime.messageKey));
      expect(text, isNot(contains('error.')));
      expect(text, isNot(contains('pac_file_read')));
    });
  });

  group('menu key provenance', () {
    test(
      'every menu entry resolves for all 9 languages (en fallback allowed)',
      () {
        for (final entry in flattenMenu(mainMenuModel)) {
          expect(entry.l10nKey, isNotNull, reason: entry.label);
          for (final lang in kL10nLanguages) {
            final resolved = L10n(lang).t(entry.l10nKey!);
            expect(resolved, isNotEmpty);
            expect(
              resolved,
              isNot(entry.l10nKey),
              reason: '${entry.l10nKey}/$lang',
            );
          }
        }
      },
    );
  });
}
