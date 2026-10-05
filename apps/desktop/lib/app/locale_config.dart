import 'package:flutter/widgets.dart';

import '../shared/l10n/l10n.dart';

/// Maps the persisted `UiItem.CurrentLanguage` code (upstream uses `zh-Hans`,
/// `zh-Hant`, `en`, `fa`, ...) to a Flutter [Locale].
///
/// R4-30: the mapped locale now drives the whole product resource set
/// (`lib/shared/l10n/strings.g.dart`, extracted from the frozen `ResUI*.resx`),
/// not only Material widgets. [kL10nDelegate] exposes the language code to
/// [L10n.of], and `app.dart` registers it alongside the Material delegates.
Locale? localeForLanguage(String? code) {
  final value = (code ?? '').trim();
  if (value.isEmpty) return null;
  final parts = value.split('-');
  final language = parts.first.toLowerCase();
  if (language.length < 2) return null;
  if (parts.length > 1 && parts[1].isNotEmpty) {
    final script = parts[1];
    final normalized = script.length == 4
        ? '${script[0].toUpperCase()}${script.substring(1).toLowerCase()}'
        : script;
    return Locale.fromSubtags(languageCode: language, scriptCode: normalized);
  }
  return Locale(language);
}

/// Localizations delegate exposing the active product resource language.
///
/// Register in `MaterialApp.localizationsDelegates` before any widget that uses
/// `context.tr(...)` / `context.errorText(...)` is built.
const L10nDelegate kL10nDelegate = L10nDelegate();

/// Locales the frozen v2rayN UI ships (ResUI resource set).
const List<Locale> kSupportedLanguages = <Locale>[
  Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans'),
  Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hant'),
  Locale('en'),
  Locale('fa'),
  Locale('fr'),
  Locale('ru'),
  Locale('hu'),
  Locale('id'),
  Locale('az'),
];
