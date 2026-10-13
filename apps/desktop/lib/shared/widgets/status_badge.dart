import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Connection badge kind, derived read-only from the runtime snapshot.
/// The badge never owns kernel state: it only projects [RuntimeView] facts.
enum StatusBadgeKind { running, connecting, stopped, failed }

/// Map a runtime snapshot to a badge kind. A current failure always wins
/// (matching the status-bar headline ordering); a busy transition reads as
/// connecting; a clean non-running snapshot reads as stopped.
StatusBadgeKind badgeKindForRuntime(RuntimeView runtime) {
  if (runtime.error != null) return StatusBadgeKind.failed;
  if (runtime.isBusy) return StatusBadgeKind.connecting;
  if (runtime.isRunning) return StatusBadgeKind.running;
  return StatusBadgeKind.stopped;
}

/// Short scannable label for [kind]. The node/session detail stays in the
/// status-bar center line and the tooltip; the badge answers "are we on".
String badgeLabelFor(
  StatusBadgeKind kind, {
  String? mode,
  bool hasUnappliedChanges = false,
}) {
  final modeSuffix = mode == null || mode.isEmpty ? '' : ' · $mode';
  final unappliedSuffix = hasUnappliedChanges ? '（有未应用更改）' : '';
  switch (kind) {
    case StatusBadgeKind.running:
      return '运行中$modeSuffix$unappliedSuffix';
    case StatusBadgeKind.connecting:
      return '连接中…';
    case StatusBadgeKind.stopped:
      return '未启动';
    case StatusBadgeKind.failed:
      return '启动失败，查看';
  }
}

/// Connection badge (UX2-02 + UX1-02 aggregate capsule): 16px semantic icon
/// plus 500-weight text in a 6px rounded outline container. Icon and text
/// always change together (never color alone). Tapping is only wired in the
/// failed state (opens the failure details); all other states are display.
class StatusBadge extends StatelessWidget {
  const StatusBadge({
    super.key,
    required this.kind,
    required this.label,
    this.onTap,
  });

  final StatusBadgeKind kind;
  final String label;
  final VoidCallback? onTap;

  static String _iconName(StatusBadgeKind kind) {
    switch (kind) {
      case StatusBadgeKind.running:
        return 'running';
      case StatusBadgeKind.connecting:
        return 'info';
      case StatusBadgeKind.stopped:
        return 'stopped';
      case StatusBadgeKind.failed:
        return 'error';
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final semantics = context.semantics;
    final fg = switch (kind) {
      StatusBadgeKind.running => semantics.success,
      StatusBadgeKind.connecting => semantics.info,
      StatusBadgeKind.stopped => semantics.muted,
      StatusBadgeKind.failed => semantics.error,
    };
    final body = Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        border: Border.all(color: scheme.outline),
        borderRadius: BorderRadius.circular(AppTokens.radiusMedium),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (kind == StatusBadgeKind.connecting)
            const SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 2),
            )
          else
            Icon(AppTokens.icon(_iconName(kind)), size: 16, color: fg),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              label,
              maxLines: 1,
              softWrap: false,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: AppTokens.fontSizeSmall,
                fontWeight: FontWeight.w500,
                color: fg,
              ),
            ),
          ),
        ],
      ),
    );
    final switched = AnimatedSwitcher(
      duration: AppTokens.motionFast,
      child: KeyedSubtree(key: ValueKey<String>('$kind:$label'), child: body),
    );
    final withTip = Tooltip(message: label, child: switched);
    final tap = onTap;
    if (tap == null) return withTip;
    return InkWell(
      onTap: tap,
      borderRadius: BorderRadius.circular(AppTokens.radiusMedium),
      child: withTip,
    );
  }
}
