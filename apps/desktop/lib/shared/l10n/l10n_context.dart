import 'package:flutter/widgets.dart';

import '../../bridge/api/contract.dart' as c;
import 'error_localizer.dart';
import 'l10n.dart';

/// Widget-tree shortcuts for product strings and error localization.
///
/// `context.tr('menuSetting')` resolves the frozen ResUI key through the active
/// locale; `context.errorText(errorDto)` renders a readable cause + action.
extension L10nContext on BuildContext {
  L10n get l10n => L10n.of(this);

  String tr(String key) => L10n.of(this).t(key);

  String trf(String key, List<Object?> args) => L10n.of(this).format(key, args);

  ErrorLocalizer get errorL10n => ErrorLocalizer(L10n.of(this));

  /// Readable text for a bridge error. Never shows a raw key.
  String errorText(c.ErrorDto error) => errorL10n.error(BridgeErrorView(error));

  /// Readable text for a runtime/platform error projection.
  String errorKeyText(String messageKey, {bool retryable = false}) => errorL10n
      .error(SimpleErrorView(messageKey: messageKey, retryable: retryable));

  /// Readable text for a stable status/message key.
  String messageText(String key) => errorL10n.key(key);
}
