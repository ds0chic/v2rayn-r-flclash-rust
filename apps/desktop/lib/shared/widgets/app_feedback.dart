import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Unified transient message presentation.
///
/// Success/info go through a single floating snack bar with the shared token
/// style; errors additionally carry the error color/icon. Callers must never
/// use this to announce a backend action that did not run.
void showAppMessage(
  BuildContext context,
  String message, {
  bool isError = false,
}) {
  final messenger = ScaffoldMessenger.maybeOf(context);
  if (messenger == null) return;
  final scheme = Theme.of(context).colorScheme;
  messenger
    ..hideCurrentSnackBar()
    ..showSnackBar(
      SnackBar(
        duration: Duration(milliseconds: isError ? 4000 : 2200),
        content: Row(
          children: <Widget>[
            Icon(
              isError ? AppTokens.icon('error') : AppTokens.icon('success'),
              size: AppTokens.iconSize,
              color: isError ? scheme.error : scheme.primary,
            ),
            const SizedBox(width: 8),
            Expanded(child: Text(message)),
          ],
        ),
      ),
    );
}

/// Inline error banner used inside forms/windows where a snack bar would be
/// detached from the field that failed. Mirrors the error code + readable key.
class InlineError extends StatelessWidget {
  const InlineError({super.key, required this.message, this.code});

  final String message;
  final String? code;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final text = code == null ? message : '$message ($code)';
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      decoration: BoxDecoration(
        color: scheme.errorContainer.withValues(alpha: 0.5),
        borderRadius: const BorderRadius.all(Radius.circular(3)),
      ),
      child: Row(
        children: <Widget>[
          Icon(
            AppTokens.icon('error'),
            size: AppTokens.iconSizeSmall,
            color: scheme.onErrorContainer,
          ),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              text,
              style: TextStyle(
                fontSize: AppTokens.fontSizeSmall,
                color: scheme.onErrorContainer,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
