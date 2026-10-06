import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_format.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/l10n/error_localizer.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n_context.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Bottom status bar (compat/layouts.yaml LAY-STATUSBAR-001).
///
/// Partitioned like the upstream `StatusBarView.xaml`: left two-line 本地/局域网
/// ports, the Tun switch, the system-proxy and routing selectors; a flexible
/// centre two-line service summary; right two-line proxy/direct rates and today
/// traffic. Raw runtime diagnostics (host/PID/revision/epoch/seq/counts) moved
/// into the `详情` popup instead of being mixed into the action row. Every
/// runtime value still comes from the shared UI state store; no fabricated
/// "connected" state is ever shown.
class StatusBarView extends ConsumerWidget {
  const StatusBarView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shell = ref.watch(uiShellControllerProvider);
    final profiles = ref.watch(profilesControllerProvider);
    final runtime = ref.watch(runtimeControllerProvider);
    final settings = ref.watch(settingsControllerProvider);
    final desiredTun = _desiredTun(settings.document);
    final routing = ref.watch(routingControllerProvider);
    final routingController = ref.read(routingControllerProvider.notifier);
    final platform = ref.watch(platformControllerProvider);
    final platformController = ref.read(platformControllerProvider.notifier);
    final monitor = ref.watch(monitorControllerProvider);
    final scheme = Theme.of(context).colorScheme;

    // Left partitions, in the upstream DockPanel order. Each partition is
    // width-capped (SP-19 AppTokens budgets) so the whole strip fits the
    // upstream 800px minimum without a horizontal scroller; long labels
    // ellipsize inside their cap and keep their tooltip.
    final inboundBlock = ConstrainedBox(
      constraints: const BoxConstraints(
        maxWidth: AppTokens.statusInboundMaxWidth,
      ),
      child: _twoLine(
        _statusText(
          '${context.tr('statusLocal')}: ${shell.inbound ?? '--'}',
          key: const ValueKey('status-inbound'),
        ),
        _statusText(
          '${context.tr('statusLan')}: ${shell.inboundLan ?? '--'}',
          key: const ValueKey('status-inbound-lan'),
        ),
      ),
    );
    final tunBlock = ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: AppTokens.statusTunMaxWidth),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            context.tr('TbEnableTunAs'),
            style: const TextStyle(fontSize: 11.5),
          ),
          Switch(
            key: const ValueKey('tun-toggle'),
            value: desiredTun,
            // Compact desktop density (SP-19): the bar rows stay at text
            // height; the switch keeps its full tap behavior without the
            // mobile-size tap padding.
            materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
            onChanged: (value) => _onTunToggle(context, ref, value),
          ),
          // FIX-13: the desired flag is persisted to `TunModeItem.EnableTun`
          // and then the real plan is re-applied. The actual label is read from
          // the runtime snapshot, never from the switch: no live runtime (or
          // switch off) reads 未启用; a helper refusal reads 失败已回滚; a live
          // runtime reads 已请求(未验证).
          Flexible(
            child: ConstrainedBox(
              constraints: const BoxConstraints(
                maxWidth: AppTokens.statusTunActualMaxWidth,
              ),
              child: _statusText(
                '${context.tr('statusActual')}: ${tunActualLabel(desiredTun, runtime)}',
                key: const ValueKey('tun-actual'),
              ),
            ),
          ),
        ],
      ),
    );
    Widget cappedSelector({required double maxWidth, required Widget child}) {
      return ConstrainedBox(
        constraints: BoxConstraints(maxWidth: maxWidth),
        child: child,
      );
    }

    // F-ROUTING-001: Rule / Global / Direct mode switch (real backend switch
    // through the routing controller). It stays in row A between the two
    // upstream combos, so the R4-06 selector order (sysproxy, mode, routing)
    // holds at every width.
    final ruleModeSelector = cappedSelector(
      maxWidth: AppTokens.statusRuleModeMaxWidth,
      child: PopupMenuButton<String>(
        key: const ValueKey('routing-mode-selector'),
        tooltip: context.tr('menuRulemode'),
        onSelected: routingController.setRuleMode,
        itemBuilder: (context) => <PopupMenuEntry<String>>[
          for (final mode in ['Rule', 'Global', 'Direct'])
            PopupMenuItem<String>(
              value: mode,
              child: Text(
                mode == routing.ruleMode ? '✓ $mode' : mode,
                style: const TextStyle(fontSize: 12),
              ),
            ),
        ],
        child: _SelectorFace(
          // SP-19: value only, like the upstream combo display member.
          child: _statusText(routing.ruleMode),
        ),
      ),
    );

    // Row A (primary), in the upstream DockPanel order: two-line ports, the
    // 160-wide system-proxy and routing combos, the flex service summary and
    // the right-docked two-line rates. Width-capped so the whole row shares
    // the initial 800px viewport with no horizontal scroller.
    final left = <Widget>[
      inboundBlock,
      const _Sep(),
      // F-SYSPROXY-001 / ACT-STAT-001: the four system-proxy modes, applied
      // through the real platform bridge. State is read back from the backend,
      // never fabricated from the click.
      cappedSelector(
        maxWidth: AppTokens.statusSysProxyMaxWidth,
        child: PopupMenuButton<SysProxyMode>(
          key: const ValueKey('system-proxy-selector'),
          tooltip: context.tr('menuSystemproxy'),
          onSelected: (mode) => platformController.applyModeFromConfig(mode),
          itemBuilder: (context) => <PopupMenuEntry<SysProxyMode>>[
            for (final mode in SysProxyMode.values)
              PopupMenuItem<SysProxyMode>(
                value: mode,
                child: Text(
                  platform.desiredMode == mode ? '✓ ${mode.label}' : mode.label,
                  style: const TextStyle(fontSize: 12),
                ),
              ),
          ],
          child: _SelectorFace(
            // SP-19: value only (upstream combo shows the selected item, the
            // hint names the control). The full "control: value (state)" string
            // stays in the tooltip and the menu lists every mode.
            child: _statusText(
              '${platform.desiredMode.label} (${platform.stateLabel})',
              key: const ValueKey('status-sysproxy'),
            ),
          ),
        ),
      ),
      const SizedBox(width: 6),
      ruleModeSelector,
      const SizedBox(width: 6),
      // Active routing scheme (upstream cmbRoutings2): switch default.
      cappedSelector(
        maxWidth: AppTokens.statusRoutingMaxWidth,
        child: PopupMenuButton<String>(
          key: const ValueKey('routing-selector'),
          tooltip: context.tr('menuRouting'),
          onSelected: routingController.setDefaultAndReload,
          itemBuilder: (context) => <PopupMenuEntry<String>>[
            if (routing.items.isEmpty)
              PopupMenuItem<String>(
                value: '',
                enabled: false,
                child: Text(
                  context.tr('statusRoutingEmpty'),
                  style: const TextStyle(fontSize: 12),
                ),
              ),
            for (final item in routing.items)
              PopupMenuItem<String>(
                value: item.id,
                child: Text(
                  item.isActive ? '✓ ${item.remarks}' : item.remarks,
                  style: const TextStyle(fontSize: 12),
                ),
              ),
          ],
          child: _SelectorFace(
            // SP-19: remarks only (upstream DisplayMemberPath="Remarks"); the
            // control name stays in the tooltip, over-long names ellipsize.
            child: _statusText(_activeSchemeLabel(routing)),
          ),
        ),
      ),
    ];
    final rowALeft = left;

    // Flexible centre: two-line service summary, matching
    // `txtRunningServerDisplay` / `txtRunningInfoDisplay`. Both lines read the
    // actual session (SP-17 actual summary); double-click runs the original
    // availability test (upstream PreviewMouseDown -> TestServerAvailability).
    // The summary expands into leftover strip width and ellipsizes instead of
    // pushing the docked rates off screen.
    final center = Row(
      children: <Widget>[
        Expanded(
          child: GestureDetector(
            key: const ValueKey('running-summary-gesture'),
            onDoubleTap: () => runAvailabilityTest(ref),
            child: _twoLine(
              _statusText(
                '${context.tr('statusNode')}: ${runtime.actualSummaryLabel}',
                key: const ValueKey('running-node'),
              ),
              _statusText(
                _runningSummary(context, runtime),
                key: const ValueKey('running-info'),
              ),
            ),
          ),
        ),
        IconButton(
          key: const ValueKey('running-availability-test'),
          // No new l10n key: SP-17 keeps to existing strings, this entry is
          // intentionally literal (same as other transient probe feedback).
          tooltip: '测试可用性（双击运行信息也可触发）',
          iconSize: AppTokens.iconSizeSmall,
          // Compact desktop density (SP-19): icon button fits the two-line
          // row height instead of forcing a mobile-size tap target row.
          style: IconButton.styleFrom(
            padding: EdgeInsets.zero,
            minimumSize: const Size(20, 20),
            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          ),
          onPressed: () => runAvailabilityTest(ref),
          icon: Icon(AppTokens.icon('test')),
        ),
      ],
    );

    // SP-17 message ordering: the resolved headline/secondary order guarantees
    // a current runtime failure stays the headline while an older platform
    // message is demoted to the details popup (never covering a newer result).
    // Keys 'runtime-error'/'status-message' are kept for existing coverage.
    // SP-19: only the headline (plus conflicts) renders inline in the strip;
    // secondary texts live in the details popup so diagnostics never widen
    // the main row off the 800px viewport.
    final platformErrorText = platform.error == null
        ? null
        : ErrorLocalizer.withCode(
            context.errorKeyText(platform.error!.messageKey),
            platform.error!.code,
          );
    final resolved = resolveStatusMessages(
      runtimeError: runtime.error,
      runtimeNotice: runtime.notice,
      platformErrorText: platformErrorText,
      platformMessage: platform.message,
      shellMessage: shell.message,
    );

    // Row A right: the upstream right-docked two-line rates. Width-capped so
    // the rates stay in the initial 800px viewport next to the selectors.
    final ratesBlock = ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: AppTokens.statusRateMaxWidth),
      child: _twoLine(
        _statusText(
          '${context.tr('statusProxySpeed')} ↑${monitor.hasTraffic ? formatRate(monitor.proxyUpBps) : '--'} '
          '↓${monitor.hasTraffic ? formatRate(monitor.proxyDownBps) : '--'}',
          key: const ValueKey('status-proxy-speed'),
        ),
        _statusText(
          '${context.tr('statusDirectSpeed')} ↑${monitor.hasTraffic ? formatRate(monitor.directUpBps) : '--'} '
          '↓${monitor.hasTraffic ? formatRate(monitor.directDownBps) : '--'}',
          key: const ValueKey('status-direct-speed'),
        ),
        align: CrossAxisAlignment.end,
      ),
    );
    // R3-09b: "今日" is the aggregate of the live per-node
    // `ServerStatItem` today counters (upstream `StatisticsManager`
    // semantics). With no node rows it shows `--` instead of a fake zero.
    // Row B keeps it next to the TUN block so the main row never grows.
    final todayBlock = ConstrainedBox(
      constraints: const BoxConstraints(
        maxWidth: AppTokens.statusTodayMaxWidth,
      ),
      child: _statusText(
        '${context.tr('statusToday')} ↑${monitor.hasTodayNodes ? formatTraffic(monitor.todayUp) : '--'} '
        '↓${monitor.hasTodayNodes ? formatTraffic(monitor.todayDown) : '--'}',
        key: const ValueKey('status-today-traffic'),
      ),
    );
    // Technical diagnostics moved out of the action row into a details popup.
    final detailsButton = PopupMenuButton<void>(
      key: const ValueKey('status-details'),
      tooltip: context.tr('statusRunningDetails'),
      itemBuilder: (context) => <PopupMenuEntry<void>>[
        PopupMenuItem<void>(
          enabled: false,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                '${context.tr('statusRuntime')}: ${runtime.state} '
                'host=${runtime.hostAlive ? 'alive' : 'down'} '
                'PID=${runtime.pid ?? '--'} '
                '${context.tr('statusPorts')}=${runtime.ports.isEmpty ? '--' : runtime.ports.join(',')}',
                key: const ValueKey('runtime-info'),
                style: const TextStyle(fontSize: 12),
              ),
              const SizedBox(height: 4),
              Text(
                runtime.hasUnappliedChanges
                    ? '${runtime.revisionLabel} (${context.tr('statusUnapplied')})'
                    : runtime.revisionLabel,
                key: const ValueKey('runtime-revision'),
                style: const TextStyle(fontSize: 12),
              ),
              const SizedBox(height: 4),
              Text(
                'epoch=${runtime.epoch ?? '--'} seq=${runtime.lastSeq ?? '--'}',
                key: const ValueKey('runtime-stream-position'),
                style: const TextStyle(fontSize: 12),
              ),
              const SizedBox(height: 4),
              // SP-17 actual summary: session/operation/applied revision of
              // the retained actual, never the desired target.
              Text(
                'actual=${runtime.actualSummaryLabel} '
                'session=${runtime.sessionId ?? '--'} '
                'op=${runtime.operationId ?? '--'} '
                'applied=${runtime.appliedRevision ?? '-'}',
                key: const ValueKey('runtime-actual'),
                style: const TextStyle(fontSize: 12),
              ),
              if (runtime.hasCurrentFailure) ...<Widget>[
                const SizedBox(height: 4),
                Text(
                  'failed target=${runtime.failedTargetId ?? '(默认)'} '
                  'op=${runtime.failureOperationId ?? runtime.error?.operationId ?? '--'} '
                  'at=${runtime.failedAtMs ?? '--'} '
                  '${runtime.error!.code} (${runtime.error!.messageKey})',
                  key: const ValueKey('runtime-failure'),
                  style: const TextStyle(fontSize: 12),
                ),
              ],
              if (runtime.notice != null) ...<Widget>[
                const SizedBox(height: 4),
                Text(
                  'notice@${runtime.notice!.atMs}: ${runtime.notice!.text}',
                  key: const ValueKey('runtime-notice'),
                  style: const TextStyle(fontSize: 12),
                ),
              ],
              // Demoted secondary messages: kept viewable here while the
              // headline (failure or newest info) owns the strip, so no
              // older result can cover a newer one and diagnostics never
              // widen the main row.
              for (final secondary in resolved.secondaryTexts) ...<Widget>[
                const SizedBox(height: 4),
                Text(secondary, style: const TextStyle(fontSize: 12)),
              ],
              const SizedBox(height: 4),
              Text(
                'total=${profiles.totalCount} visible=${profiles.visible.length} '
                'selected=${profiles.selectedCount}',
                key: const ValueKey('status-counts'),
                style: const TextStyle(fontSize: 12),
              ),
            ],
          ),
        ),
      ],
      child: _SelectorFace(
        child: Text(
          context.tr('statusDetails'),
          style: const TextStyle(fontSize: 11.5),
        ),
      ),
    );

    // Row B notice entries: the headline plus proxy conflicts. The headline
    // is width-capped so diagnostics can never push row B off the 800px
    // viewport; secondary texts stay in the details popup above.
    final notices = <Widget>[
      if (resolved.headlineKind == StatusHeadlineKind.runtimeError &&
          runtime.error != null)
        ConstrainedBox(
          constraints: const BoxConstraints(
            maxWidth: AppTokens.statusNoticeMaxWidth,
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Flexible(
                child: _statusText(
                  // R4-30: bridge errors render as a readable cause + action;
                  // the stable code stays available in the details popup.
                  context.errorKeyText(runtime.error!.messageKey),
                  key: const ValueKey('runtime-error'),
                  color: scheme.error,
                ),
              ),
              if (runtime.canRetryFailed) ...<Widget>[
                const SizedBox(width: 4),
                TextButton(
                  key: const ValueKey('runtime-error-retry'),
                  // Compact desktop density (SP-19): the notice row stays at
                  // text height instead of forcing mobile-size tap targets.
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 2,
                    ),
                    minimumSize: const Size(0, 24),
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    textStyle: const TextStyle(fontSize: 12),
                  ),
                  onPressed: () => ref
                      .read(runtimeControllerProvider.notifier)
                      .retryFailed(),
                  child: const Text('重试'),
                ),
                TextButton(
                  key: const ValueKey('runtime-error-view'),
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 2,
                    ),
                    minimumSize: const Size(0, 24),
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    textStyle: const TextStyle(fontSize: 12),
                  ),
                  onPressed: () => showFailureDetails(context, runtime),
                  child: const Text('查看失败'),
                ),
              ],
            ],
          ),
        )
      else if (resolved.headlineKind == StatusHeadlineKind.runtimeError)
        ConstrainedBox(
          constraints: const BoxConstraints(
            maxWidth: AppTokens.statusNoticeMaxWidth,
          ),
          child: _statusText(
            resolved.headlineText,
            key: const ValueKey('status-sysproxy-error'),
            color: scheme.error,
          ),
        ),
      if (platform.error != null && runtime.error != null)
        ConstrainedBox(
          constraints: const BoxConstraints(
            maxWidth: AppTokens.statusNoticeMaxWidth,
          ),
          child: _statusText(
            platformErrorText!,
            key: const ValueKey('status-sysproxy-error'),
            color: scheme.error,
          ),
        ),
      if (platform.conflicts.isNotEmpty)
        ConstrainedBox(
          constraints: const BoxConstraints(
            maxWidth: AppTokens.statusNoticeMaxWidth,
          ),
          child: _statusText(
            '${context.tr('menuSystemproxy')} ${platform.conflicts.map((c) => c.field).join(',')}',
            key: const ValueKey('status-sysproxy-conflict'),
            color: scheme.error,
          ),
        ),
      if (resolved.headlineKind == StatusHeadlineKind.info)
        ConstrainedBox(
          constraints: const BoxConstraints(
            maxWidth: AppTokens.statusNoticeMaxWidth,
          ),
          child: _statusText(
            resolved.headlineText,
            key: const ValueKey('status-message'),
            color: scheme.primary,
          ),
        ),
    ];

    // SP-19 two-row bar. Row A keeps the upstream DockPanel order (two-line
    // ports, the 160-wide combos with the rule-mode switch between them, the
    // flex service summary, right-docked two-line rates) and always fits the
    // 800px minimum with no horizontal scroller. Row B carries the TUN block,
    // the today aggregate, the details entry and the notice headline, so
    // diagnostics and secondary feedback never widen the main row (UI-05). Fonts and spacing
    // follow the original tokens; long labels ellipsize with tooltips instead
    // of shrinking to unreadable sizes.
    return Material(
      color: scheme.surfaceContainerHighest,
      child: SizedBox(
        height: AppTokens.statusBarHeight,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              Row(
                children: <Widget>[
                  ...rowALeft,
                  const SizedBox(width: 8),
                  Expanded(child: center),
                  const SizedBox(width: 8),
                  ratesBlock,
                ],
              ),
              Row(
                children: <Widget>[
                  tunBlock,
                  const _Sep(),
                  todayBlock,
                  const _Sep(),
                  detailsButton,
                  for (final notice in notices) ...<Widget>[
                    const _Sep(),
                    Expanded(child: notice),
                  ],
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A single status line. It never wraps, so the two-line partitions keep the
/// fixed bar height at any width/DPI; long labels ellipsize inside their
/// width-capped partition instead of pushing neighbours off the 800px
/// viewport. Fonts stay at the original compact sizes (never shrunk to fit).
Widget _statusText(String data, {Key? key, Color? color}) {
  return Text(
    data,
    key: key,
    maxLines: 1,
    softWrap: false,
    overflow: TextOverflow.ellipsis,
    style: TextStyle(fontSize: AppTokens.fontSizeSmall, color: color),
  );
}

/// Two stacked lines, the upstream `StatusbarItem` block used by the ports,
/// service and rate partitions.
Widget _twoLine(
  Widget top,
  Widget bottom, {
  CrossAxisAlignment align = CrossAxisAlignment.start,
}) {
  return Column(
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: align,
    children: <Widget>[top, const SizedBox(height: 1), bottom],
  );
}

String _runningSummary(BuildContext context, RuntimeView runtime) {
  final ports = runtime.ports.isEmpty ? '--' : runtime.ports.join(',');
  final session = runtime.sessionId == null
      ? ''
      : ' · 会话=${_shortId(runtime.sessionId!)}';
  final applied = runtime.appliedRevision == null
      ? ''
      : ' · 已应用=${runtime.appliedRevision}';
  return '${runtime.state} · ${context.tr('statusPorts')} $ports$session$applied';
}

String _shortId(String id) => id.length > 12 ? '${id.substring(0, 12)}…' : id;

/// SP-17 "查看当前失败": the retained actual stays on screen while the dialog
/// names the failed target, operation, time and structured cause with a retry
/// entry. Never presents the failed desired target as applied.
Future<void> showFailureDetails(BuildContext context, RuntimeView runtime) {
  final error = runtime.error;
  if (error == null) return Future.value();
  return showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('当前失败'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            '失败目标=${runtime.failedTargetId ?? '(默认)'}',
            key: const ValueKey('failure-details-target'),
          ),
          Text('操作=${runtime.failureOperationId ?? error.operationId ?? '--'}'),
          Text('时间=${runtime.failedAtMs ?? '--'}'),
          Text('${error.code} (${error.messageKey})'),
          if (error.detail != null) Text(error.detail!),
          const SizedBox(height: 8),
          Text('仍在运行=${runtime.actualSummaryLabel}'),
        ],
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('failure-details-close'),
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('关闭'),
        ),
      ],
    ),
  );
}

/// Bordered selector face with a visible dropdown arrow, so the status-bar
/// controls read as clickable controls rather than plain status text.
///
/// The label flexes inside the width-capped partition and ellipsizes instead
/// of pushing the neighbour partitions off the 800px viewport. Every face is
/// used under a bounded width (capped combo or the tight bar row), so the
/// Flexible always has a finite bound.
class _SelectorFace extends StatelessWidget {
  const _SelectorFace({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.only(left: 6, right: 2, top: 2, bottom: 2),
      decoration: BoxDecoration(
        border: Border.all(color: scheme.outline),
        borderRadius: BorderRadius.circular(3),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Flexible(child: child),
          const Icon(Icons.arrow_drop_down, size: 16),
        ],
      ),
    );
  }
}

/// Desired TUN flag from the persisted settings document (`TunModeItem`).
bool _desiredTun(Map<String, dynamic> document) {
  final tun = document['TunModeItem'];
  if (tun is Map<String, dynamic>) return tun['EnableTun'] == true;
  return false;
}

/// Persist the desired TUN flag first; only a successful save re-applies the
/// real plan. A failed save leaves the running session untouched and reports
/// the failure; a denied/absent elevation helper surfaces through the runtime
/// error and is never rendered as success.
Future<void> _onTunToggle(
  BuildContext context,
  WidgetRef ref,
  bool value,
) async {
  // Resolve the language before any await; the toggle does not touch the
  // BuildContext across async gaps (avoids the use_build_context lint and a
  // stale element).
  final l10n = context.l10n;
  final localizer = ErrorLocalizer(l10n);
  final settings = ref.read(settingsControllerProvider.notifier);
  final shell = ref.read(uiShellControllerProvider.notifier);
  final result = await toggleTunDesired(
    enabled: value,
    persist: (enabled) {
      final document = settings.draft();
      final tun = Map<String, dynamic>.of(
        (document['TunModeItem'] as Map<String, dynamic>?) ?? const {},
      );
      tun['EnableTun'] = enabled;
      document['TunModeItem'] = tun;
      final saved = settings.saveDocument(document);
      if (!saved.ok) {
        shell.setMessage(
          '${l10n.t('settingsSaveFailed')}: '
          '${localizer.error(BridgeErrorView(saved.error!))}',
        );
      }
      return saved.ok;
    },
    apply: () => ref.read(runtimeControllerProvider.notifier).applyActive(),
  );
  if (!result.ok) {
    if (result.error == 'error.tun_apply_failed') {
      // The setting is saved but the runtime refused the plan (UAC/helper
      // refusal or a failed apply): report honestly, never as success.
      shell.setMessage(l10n.t('tunDenied'));
    }
    return;
  }
  shell.setMessage(
    value ? l10n.t('tunSavedEnabled') : l10n.t('tunSavedDisabled'),
  );
}

String _activeSchemeLabel(RoutingState routing) {
  for (final item in routing.items) {
    if (item.isActive) return item.remarks;
  }
  return routing.items.isEmpty ? '--' : routing.items.first.remarks;
}

/// SP-17 headline kind for the status-bar message strip.
enum StatusHeadlineKind { runtimeError, info, none }

/// Ordered status messages: one headline plus demoted secondary texts.
///
/// A current runtime failure is always the headline (with retry); older
/// platform/shell messages move to [secondaryTexts] so they can never cover a
/// newer result. With no failure, the newest info (availability notice, then
/// platform message, then shell message) is the headline.
class ResolvedStatusMessages {
  const ResolvedStatusMessages({
    required this.headlineKind,
    required this.headlineText,
    required this.headlineRetryable,
    required this.secondaryTexts,
  });

  /// Error headlines carry the error messageKey (localized at render);
  /// info headlines carry display text.
  final StatusHeadlineKind headlineKind;
  final String headlineText;
  final bool headlineRetryable;
  final List<String> secondaryTexts;
}

ResolvedStatusMessages resolveStatusMessages({
  required RuntimeErrorView? runtimeError,
  required RuntimeNotice? runtimeNotice,
  required String? platformErrorText,
  required String? platformMessage,
  required String? shellMessage,
}) {
  if (runtimeError != null) {
    final secondary = <String>[
      ?platformErrorText,
      ?platformMessage,
      ?runtimeNotice?.text,
      ?shellMessage,
    ];
    return ResolvedStatusMessages(
      headlineKind: StatusHeadlineKind.runtimeError,
      headlineText: runtimeError.messageKey,
      // A failure headline always offers view/retry; the structured
      // retryable flag only gates automatic retry, never the manual entry.
      headlineRetryable: true,
      secondaryTexts: secondary,
    );
  }
  if (platformErrorText != null) {
    return ResolvedStatusMessages(
      headlineKind: StatusHeadlineKind.runtimeError,
      headlineText: platformErrorText,
      headlineRetryable: false,
      secondaryTexts: <String>[
        ?runtimeNotice?.text,
        ?platformMessage,
        ?shellMessage,
      ],
    );
  }
  final info = runtimeNotice?.text ?? platformMessage ?? shellMessage;
  if (info == null) {
    return const ResolvedStatusMessages(
      headlineKind: StatusHeadlineKind.none,
      headlineText: '',
      headlineRetryable: false,
      secondaryTexts: <String>[],
    );
  }
  final rest = <String>[
    if (runtimeNotice == null &&
        platformMessage != null &&
        platformMessage != info)
      platformMessage,
    if (runtimeNotice != null && platformMessage != null) platformMessage,
    if (shellMessage != null && shellMessage != info) shellMessage,
  ];
  return ResolvedStatusMessages(
    headlineKind: StatusHeadlineKind.info,
    headlineText: info,
    headlineRetryable: false,
    secondaryTexts: rest,
  );
}

/// Result of the original availability test against the actual applied
/// endpoint (upstream `TestServerAvailability`).
class AvailabilityProbeResult {
  const AvailabilityProbeResult({
    required this.reachable,
    required this.endpoint,
    required this.elapsedMs,
    this.detail,
  });

  final bool reachable;
  final String endpoint;
  final int elapsedMs;
  final String? detail;

  String get label => reachable
      ? '可用性 $endpoint 可达 ${elapsedMs}ms'
      : '可用性 $endpoint 不可达${detail == null ? '' : '（$detail）'}';
}

/// Probe the actual applied local endpoint with a real loopback TCP connect.
///
/// Only ever dials 127.0.0.1 at the given applied port with a bounded timeout;
/// the reserved 10808 proxy port is never probed. No success is fabricated:
/// a refused/timeout port reports unreachable with its cause.
Future<AvailabilityProbeResult> probeLoopbackEndpoint(
  int port, {
  Duration timeout = const Duration(seconds: 3),
}) async {
  final endpoint = '127.0.0.1:$port';
  if (port == 10808) {
    return const AvailabilityProbeResult(
      reachable: false,
      endpoint: '127.0.0.1:10808',
      elapsedMs: 0,
      detail: '保留端口 10808 不探测',
    );
  }
  final stopwatch = Stopwatch()..start();
  try {
    final socket = await Socket.connect(
      InternetAddress.loopbackIPv4,
      port,
      timeout: timeout,
    );
    socket.destroy();
    stopwatch.stop();
    return AvailabilityProbeResult(
      reachable: true,
      endpoint: endpoint,
      elapsedMs: stopwatch.elapsedMilliseconds,
    );
  } on Object catch (e) {
    stopwatch.stop();
    return AvailabilityProbeResult(
      reachable: false,
      endpoint: endpoint,
      elapsedMs: stopwatch.elapsedMilliseconds,
      detail: e.toString().split('\n').first,
    );
  }
}

/// Run the original availability test for the actual session (ACT-STAT-004):
/// double-clicking the running texts (upstream PreviewMouseDown) or the test
/// button probes the applied endpoint and records real loopback feedback.
Future<void> runAvailabilityTest(WidgetRef ref) async {
  final runtime = ref.read(runtimeControllerProvider);
  final controller = ref.read(runtimeControllerProvider.notifier);
  final shell = ref.read(uiShellControllerProvider.notifier);
  final port = runtime.proxyPort;
  if (port == null) {
    controller.reportAvailabilityNotice(
      text: '无已应用端点，未执行可用性测试',
      severity: RuntimeNoticeSeverity.warning,
    );
    shell.setMessage('无已应用端点，未执行可用性测试');
    return;
  }
  shell.setMessage('正在测试 127.0.0.1:$port …');
  final result = await probeLoopbackEndpoint(port);
  controller.reportAvailabilityNotice(
    text: result.label,
    severity: result.reachable
        ? RuntimeNoticeSeverity.info
        : RuntimeNoticeSeverity.warning,
    operationId: 'avail-$port',
  );
  shell.setMessage(result.label);
}

class _Sep extends StatelessWidget {
  const _Sep();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.symmetric(horizontal: 6),
      child: Text('|', style: TextStyle(fontSize: 11, color: Colors.grey)),
    );
  }
}
