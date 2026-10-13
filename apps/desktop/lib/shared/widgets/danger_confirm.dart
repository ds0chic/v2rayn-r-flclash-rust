import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/onboarding/onboarding_store.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Dangerous-action gate (UX1-03): names the target and the impact, keeps the
/// upstream cancel-left / confirm-right order with Enter-confirm / Esc-cancel,
/// and offers a per-dialog "don't ask again" suppression persisted in
/// ui_state (resettable via help menu UI-RESET-HINTS).
///
/// Returns true only on explicit confirm. Cancel, Esc and tap-outside all
/// return false with zero side effects.
Future<bool> showDangerConfirm(
  BuildContext context,
  WidgetRef ref, {
  required String title,
  required String target,
  required String impact,
  required String confirmLabel,
  required String dontAskKey,
  Key? dialogKey,
}) async {
  final store = ref.read(uiStateStoreProvider);
  if (OnboardingHints.isSuppressed(store, dontAskKey)) return true;
  var dontAsk = false;
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      key: dialogKey,
      title: Text(title),
      content: StatefulBuilder(
        builder: (context, setState) => Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(target, style: const TextStyle(fontWeight: FontWeight.w500)),
            const SizedBox(height: 8),
            Text(
              impact,
              style: TextStyle(
                fontSize: AppTokens.fontSizeSmall,
                color: Theme.of(context).colorScheme.error,
              ),
            ),
            const SizedBox(height: 12),
            Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Checkbox(
                  key: ValueKey('$dontAskKey-checkbox'),
                  value: dontAsk,
                  onChanged: (value) =>
                      setState(() => dontAsk = value ?? false),
                ),
                const Text('不再提示'),
              ],
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('danger-confirm-cancel'),
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('danger-confirm-ok'),
          autofocus: true,
          style: FilledButton.styleFrom(
            backgroundColor: Theme.of(context).colorScheme.error,
            foregroundColor: Theme.of(context).colorScheme.onError,
          ),
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(confirmLabel),
        ),
      ],
    ),
  );
  if (confirmed == true && dontAsk) {
    OnboardingHints.setSuppressed(store, dontAskKey, true);
  }
  return confirmed ?? false;
}
