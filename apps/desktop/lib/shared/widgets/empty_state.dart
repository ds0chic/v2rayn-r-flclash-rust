import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Consistent empty-state presentation for every panel (nodes / subscriptions
/// / logs / connections / proxies / not-running). Icon plus text, never a
/// fabricated row or a fake "connected" state.
class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.message,
    this.semanticIcon = 'empty',
    this.detail,
    this.messageKey,
    this.actions,
  });

  final String message;

  /// Semantic name resolved through [AppTokens.icon]; keeps one icon family.
  final String semanticIcon;
  final String? detail;
  final Key? messageKey;

  /// Optional call-to-action buttons rendered under the text (U-B1: empty
  /// states become actionable instead of dead ends). Null keeps the legacy
  /// icon-plus-text rendering untouched.
  final List<Widget>? actions;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final muted = scheme.onSurfaceVariant;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(
              AppTokens.icon(semanticIcon),
              size: 32,
              color: muted.withValues(alpha: 0.7),
            ),
            const SizedBox(height: 8),
            Text(
              message,
              key: messageKey,
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: AppTokens.fontSize, color: muted),
            ),
            if (detail != null) ...<Widget>[
              const SizedBox(height: 4),
              Text(
                detail!,
                textAlign: TextAlign.center,
                style: TextStyle(
                  fontSize: AppTokens.fontSizeSmall,
                  color: muted.withValues(alpha: 0.8),
                ),
              ),
            ],
            if (actions != null && actions!.isNotEmpty) ...<Widget>[
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                alignment: WrapAlignment.center,
                children: actions!,
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// In-progress indicator that shows the real backend stage text and a spinner.
/// No synthetic 0-100% progress is ever produced (plan §08).
class StageIndicator extends StatelessWidget {
  const StageIndicator({
    super.key,
    required this.stage,
    this.semanticIcon = 'running',
  });

  final String stage;
  final String semanticIcon;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        const SizedBox(
          width: 12,
          height: 12,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
        const SizedBox(width: 6),
        Icon(
          AppTokens.icon(semanticIcon),
          size: AppTokens.iconSizeSmall,
          color: scheme.primary,
        ),
        const SizedBox(width: 4),
        Text(
          stage,
          style: TextStyle(
            fontSize: AppTokens.fontSizeSmall,
            color: scheme.primary,
          ),
        ),
      ],
    );
  }
}
