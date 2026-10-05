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

    // Left partitions, in the upstream DockPanel order.
    final left = <Widget>[
      _twoLine(
        _statusText(
          '${context.tr('statusLocal')}: ${shell.inbound ?? '--'}',
          key: const ValueKey('status-inbound'),
        ),
        _statusText(
          '${context.tr('statusLan')}: ${shell.inboundLan ?? '--'}',
          key: const ValueKey('status-inbound-lan'),
        ),
      ),
      const _Sep(),
      Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            context.tr('TbEnableTunAs'),
            style: const TextStyle(fontSize: 11.5),
          ),
          Switch(
            key: const ValueKey('tun-toggle'),
            value: desiredTun,
            onChanged: (value) => _onTunToggle(context, ref, value),
          ),
          // FIX-13: the desired flag is persisted to `TunModeItem.EnableTun`
          // and then the real plan is re-applied. The actual label is read from
          // the runtime snapshot, never from the switch: no live runtime (or
          // switch off) reads 未启用; a helper refusal reads 失败已回滚; a live
          // runtime reads 已请求(未验证).
          _statusText(
            '${context.tr('statusActual')}: ${tunActualLabel(desiredTun, runtime)}',
            key: const ValueKey('tun-actual'),
          ),
        ],
      ),
      const _Sep(),
      // F-SYSPROXY-001 / ACT-STAT-001: the four system-proxy modes, applied
      // through the real platform bridge. State is read back from the backend,
      // never fabricated from the click.
      PopupMenuButton<SysProxyMode>(
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
          child: _statusText(
            '${context.tr('menuSystemproxy')}: ${platform.desiredMode.label} (${platform.stateLabel})',
            key: const ValueKey('status-sysproxy'),
          ),
        ),
      ),
      const SizedBox(width: 6),
      // F-ROUTING-001: Rule / Global / Direct mode switch (real backend switch
      // through the routing controller).
      PopupMenuButton<String>(
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
          child: _statusText(
            '${context.tr('menuRulemode')}: ${routing.ruleMode}',
          ),
        ),
      ),
      const SizedBox(width: 6),
      // Active routing scheme (upstream cmbRoutings2): switch default.
      PopupMenuButton<String>(
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
          child: _statusText(
            '${context.tr('menuRouting')}: ${_activeSchemeLabel(routing)}',
          ),
        ),
      ),
    ];

    // Flexible centre: two-line service summary, matching
    // `txtRunningServerDisplay` / `txtRunningInfoDisplay`.
    final center = _twoLine(
      _statusText(
        '${context.tr('statusNode')}: ${runtime.statusLabel}',
        key: const ValueKey('running-node'),
      ),
      _statusText(
        _runningSummary(context, runtime),
        key: const ValueKey('running-info'),
      ),
    );

    // Right partitions: two-line rates and the today aggregate.
    final right = <Widget>[
      _twoLine(
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
      const _Sep(),
      // R3-09b: "今日" is the aggregate of the live per-node
      // `ServerStatItem` today counters (upstream `StatisticsManager`
      // semantics). With no node rows it shows `--` instead of a fake zero.
      _statusText(
        '${context.tr('statusToday')} ↑${monitor.hasTodayNodes ? formatTraffic(monitor.todayUp) : '--'} '
        '↓${monitor.hasTodayNodes ? formatTraffic(monitor.todayDown) : '--'}',
        key: const ValueKey('status-today-traffic'),
      ),
      const _Sep(),
      // Technical diagnostics moved out of the action row into a details popup.
      PopupMenuButton<void>(
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
      ),
    ];

    final messages = <Widget>[
      if (runtime.error != null)
        _statusText(
          // R4-30: bridge errors render as a readable cause + action; the stable
          // code stays available in the details popup, never as the headline.
          context.errorKeyText(runtime.error!.messageKey),
          key: const ValueKey('runtime-error'),
          color: scheme.error,
        ),
      if (platform.error != null)
        _statusText(
          ErrorLocalizer.withCode(
            context.errorKeyText(platform.error!.messageKey),
            platform.error!.code,
          ),
          key: const ValueKey('status-sysproxy-error'),
          color: scheme.error,
        ),
      if (platform.conflicts.isNotEmpty)
        _statusText(
          '${context.tr('menuSystemproxy')} ${platform.conflicts.map((c) => c.field).join(',')}',
          key: const ValueKey('status-sysproxy-conflict'),
          color: scheme.error,
        ),
      if (platform.message != null)
        _statusText(
          platform.message!,
          key: const ValueKey('status-message'),
          color: scheme.primary,
        )
      else if (shell.message != null)
        _statusText(
          shell.message!,
          key: const ValueKey('status-message'),
          color: scheme.primary,
        ),
    ];

    return Material(
      color: scheme.surfaceContainerHighest,
      child: SizedBox(
        height: AppTokens.statusBarHeight,
        // A single partitioned strip. It stays on one line at the upstream
        // height; when the window is too narrow for every partition it scrolls
        // horizontally instead of shrinking the font or wrapping/dropping
        // controls, so every control stays identifiable and reachable.
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(
            children: <Widget>[
              ...left,
              const SizedBox(width: 8),
              center,
              const SizedBox(width: 8),
              ...right,
              for (final message in messages) ...<Widget>[
                const _Sep(),
                message,
              ],
            ],
          ),
        ),
      ),
    );
  }
}

/// A single status line. It never wraps, so the two-line partitions keep the
/// fixed bar height at any width/DPI; the bar scrolls horizontally when the
/// whole strip is wider than the window.
Widget _statusText(String data, {Key? key, Color? color}) {
  return Text(
    data,
    key: key,
    maxLines: 1,
    softWrap: false,
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
  return '${runtime.state} · ${context.tr('statusPorts')} $ports';
}

/// Bordered selector face with a visible dropdown arrow, so the status-bar
/// controls read as clickable controls rather than plain status text.
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
        children: <Widget>[child, const Icon(Icons.arrow_drop_down, size: 16)],
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
  if (!result.ok) return;
  final runtime = ref.read(runtimeControllerProvider);
  if (value && runtime.error != null) {
    // UAC/helper refusal or a missing privileged helper: the setting is saved
    // but the runtime was not switched to TUN. Report honestly.
    shell.setMessage(l10n.t('tunDenied'));
  } else {
    shell.setMessage(
      value ? l10n.t('tunSavedEnabled') : l10n.t('tunSavedDisabled'),
    );
  }
}

String _activeSchemeLabel(RoutingState routing) {
  for (final item in routing.items) {
    if (item.isActive) return item.remarks;
  }
  return routing.items.isEmpty ? '--' : routing.items.first.remarks;
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
