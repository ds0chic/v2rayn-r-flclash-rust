import 'package:flutter/widgets.dart';

/// Maps the persisted `UiItem.CurrentLanguage` code (upstream uses `zh-Hans`,
/// `zh-Hant`, `en`, `fa`, ...) to a Flutter [Locale].
///
/// The full localization resource bundle (`flutter_localizations` + ARB) is a
/// separate card (FIX-16B); this keeps the persisted value flowing into
/// `MaterialApp.locale` so the setting is no longer ignored.
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
