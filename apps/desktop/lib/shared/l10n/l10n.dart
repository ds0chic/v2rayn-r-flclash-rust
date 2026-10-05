import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'strings.g.dart';

/// The active product language (`UiItem.CurrentLanguage`). Defaults to the
/// upstream `zh-Hans`; `app.dart` / the shell override it from persisted state
/// so every [L10n.of] consumer follows the setting.
final productLanguageProvider = Provider<String?>((_) => null);

/// Runtime lookup over the frozen upstream `ResUI` resource table.
///
/// [L10n.of] resolves the active locale from the persisted language setting
/// (`UiItem.CurrentLanguage`) when a Riverpod container is in scope, and falls
/// back to the ambient `Localizations` locale otherwise. This keeps the shell,
/// sub-windows and direct-widget harnesses on the same language: changing the
/// setting switches every product string, not just Material widgets. When a
/// locale is missing a translation the English source value is used; when a key
/// is entirely unknown the key itself is returned (never a crash, never blank).
class L10n {
  const L10n(this.languageCode);

  /// The persisted/active language code (`zh-Hans`, `en`, ...).
  final String languageCode;

  /// Upstream default language (`ConfigHandler` writes `zh-Hans`).
  static const String defaultLanguage = 'zh-Hans';

  static L10n of(BuildContext context) {
    // Prefer the persisted setting so a widget rebuilt after the theme dialog
    // (or a direct harness with a ProviderScope) matches the product language.
    try {
      final container = ProviderScope.containerOf(context, listen: false);
      final language =
          container.read(productLanguageProvider) ?? defaultLanguage;
      if (language.isNotEmpty) return L10n(language);
    } catch (_) {
      // No ProviderScope in the tree (plain widget harness): fall through.
    }
    // Scope-less harnesses render the upstream default (`zh-Hans`); a harness
    // that wants another language nests a `ProviderScope` overriding
    // [productLanguageProvider].
    return const L10n(defaultLanguage);
  }

  /// Resolve a language code from a [Locale], preferring the script-qualified
  /// upstream file name (zh-Hans / zh-Hant) over the bare language.
  static String _codeFor(Locale locale) {
    final script = locale.scriptCode;
    if (script != null && script.isNotEmpty) {
      return '${locale.languageCode}-$script';
    }
    return locale.languageCode;
  }

  /// Look up [key] in the active language, falling back to English, then to the
  /// key itself so a raw `menuFoo` never reaches the user without a value.
  String t(String key) {
    final entry = kResUiStrings[key];
    if (entry == null) return key;
    final direct = entry[languageCode];
    if (direct != null && direct.isNotEmpty) return direct;
    final en = entry['en'];
    if (en != null && en.isNotEmpty) return en;
    return key;
  }

  /// Look up [key] and replace `{0}`, `{1}` ... with [args].
  String format(String key, List<Object?> args) {
    var value = t(key);
    for (var i = 0; i < args.length; i++) {
      value = value.replaceAll('{$i}', '${args[i]}');
    }
    return value;
  }

  /// True when the table provides a value for [key] in [languageCode].
  bool has(String key) {
    final entry = kResUiStrings[key];
    return entry != null && (entry[languageCode]?.isNotEmpty ?? false);
  }
}

/// A `LocalizationsDelegate` that only exposes the active language code. The
/// product resource table is a plain map, so no `load` work is needed; the
/// delegate lets [L10n.of] read the code synchronously from the tree.
class L10nDelegate extends LocalizationsDelegate<L10n> {
  const L10nDelegate();

  @override
  bool isSupported(Locale locale) =>
      kL10nLanguages.contains(L10n._codeFor(locale)) ||
      kL10nLanguages.contains(locale.languageCode);

  @override
  Future<L10n> load(Locale locale) async => L10n(L10n._codeFor(locale));

  @override
  bool shouldReload(covariant LocalizationsDelegate<L10n> old) => false;
}
