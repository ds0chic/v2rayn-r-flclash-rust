import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_format.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/proxy_settings_view.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Bottom status bar (compat/layouts.yaml LAY-STATUSBAR-001).
///
/// Every runtime value comes from the shared UI state store. There is no live
/// kernel/Clash data in T05, so the defaults render as `--` / `未运行`; the bar
/// never shows a fabricated "connected" state.
class StatusBarView extends ConsumerWidget {
  const StatusBarView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shell = ref.watch(uiShellControllerProvider);
    final shellController = ref.read(uiShellControllerProvider.notifier);
    final profiles = ref.watch(profilesControllerProvider);
    final runtime = ref.watch(runtimeControllerProvider);
    final routing = ref.watch(routingControllerProvider);
    final routingController = ref.read(routingControllerProvider.notifier);
    final platform = ref.watch(platformControllerProvider);
    final platformController = ref.read(platformControllerProvider.notifier);
    final monitor = ref.watch(monitorControllerProvider);
    final scheme = Theme.of(context).colorScheme;
    final muted = TextStyle(fontSize: 11.5, color: scheme.onSurfaceVariant);

    return Material(
      color: scheme.surfaceContainerHighest,
      child: SizedBox(
        height: AppTokens.statusBarHeight,
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(
            children: <Widget>[
              Text(
                '入站 ${shell.inbound ?? '--'}',
                key: const ValueKey('status-inbound'),
                style: muted,
              ),
              const _Sep(),
              Text(
                'LAN ${shell.inboundLan ?? '--'}',
                key: const ValueKey('status-inbound-lan'),
                style: muted,
              ),
              const _Sep(),
              const Text('TUN', style: TextStyle(fontSize: 11.5)),
              Switch(
                key: const ValueKey('tun-toggle'),
                value: shell.tunEnabled,
                onChanged: shellController.setTunEnabled,
              ),
              // T14: TUN actual state is read from the runtime snapshot, never
              // from the desired switch above. The bridge carries no per-lease
              // TUN facts, so the bar distinguishes only what the snapshot
              // proves: no live runtime (or switch off) reads 未启用; a helper
              // failure reads 失败已回滚; a live runtime with TUN desired reads
              // 已请求(未验证) — never a fabricated "TUN active".
              Text(
                '实际: ${_tunActualLabel(shell.tunEnabled, runtime)}',
                key: const ValueKey('tun-actual'),
                style: muted,
              ),
              const _Sep(),
              // F-SYSPROXY-001 / ACT-STAT-001: the four system-proxy modes,
              // applied through the real platform bridge. State is read back
              // from the backend, never fabricated from the click.
              PopupMenuButton<SysProxyMode>(
                key: const ValueKey('system-proxy-selector'),
                tooltip: '系统代理',
                onSelected: (mode) =>
                    _applyMode(context, ref, platformController, mode),
                itemBuilder: (context) => <PopupMenuEntry<SysProxyMode>>[
                  for (final mode in SysProxyMode.values)
                    PopupMenuItem<SysProxyMode>(
                      value: mode,
                      child: Text(
                        platform.desiredMode == mode
                            ? '✓ ${mode.label}'
                            : mode.label,
                        style: const TextStyle(fontSize: 12),
                      ),
                    ),
                ],
                child: Text(
                  '系统代理: ${platform.desiredMode.label} '
                  '(${platform.stateLabel})',
                  key: const ValueKey('status-sysproxy'),
                  style: muted,
                ),
              ),
              const _Sep(),
              // F-ROUTING-001: Rule / Global / Direct mode switch (real
              // backend switch through the routing controller).
              PopupMenuButton<String>(
                key: const ValueKey('routing-mode-selector'),
                tooltip: '路由模式',
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
                child: Text('路由模式: ${routing.ruleMode}', style: muted),
              ),
              const _Sep(),
              // Active routing scheme (upstream cmbRoutings2): switch default.
              PopupMenuButton<String>(
                key: const ValueKey('routing-selector'),
                tooltip: '路由',
                onSelected: routingController.setDefault,
                itemBuilder: (context) => <PopupMenuEntry<String>>[
                  if (routing.items.isEmpty)
                    const PopupMenuItem<String>(
                      value: '',
                      enabled: false,
                      child: Text('(无路由配置)', style: TextStyle(fontSize: 12)),
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
                child: Text('路由: ${_activeSchemeLabel(routing)}', style: muted),
              ),
              const _Sep(),
              Text(
                '节点: ${runtime.statusLabel}',
                key: const ValueKey('running-node'),
                style: muted,
              ),
              const _Sep(),
              Text(
                '运行时: ${runtime.state} '
                'host=${runtime.hostAlive ? 'alive' : 'down'} '
                'PID=${runtime.pid ?? '--'} '
                '端口=${runtime.ports.isEmpty ? '--' : runtime.ports.join(',')}',
                key: const ValueKey('runtime-info'),
                style: muted,
              ),
              const _Sep(),
              Text(
                runtime.hasUnappliedChanges
                    ? '${runtime.revisionLabel} (未应用)'
                    : runtime.revisionLabel,
                key: const ValueKey('runtime-revision'),
                style: muted,
              ),
              const _Sep(),
              Text(
                'epoch=${runtime.epoch ?? '--'} seq=${runtime.lastSeq ?? '--'}',
                key: const ValueKey('runtime-stream-position'),
                style: muted,
              ),
              if (runtime.error != null) ...<Widget>[
                const _Sep(),
                Text(
                  '错误 ${runtime.error!.code}: ${runtime.error!.messageKey}',
                  key: const ValueKey('runtime-error'),
                  style: TextStyle(fontSize: 11.5, color: scheme.error),
                ),
              ],
              const _Sep(),
              Text(
                'total=${profiles.totalCount} visible=${profiles.visible.length} '
                'selected=${profiles.selectedCount}',
                key: const ValueKey('status-counts'),
                style: muted,
              ),
              const SizedBox(width: 8),
              // F-MONITOR-006: real proxy/direct rates from the statistics
              // pipeline; `--` until the first batch arrives.
              Text(
                '代理 ↑${monitor.hasTraffic ? formatRate(monitor.proxyUpBps) : '--'} '
                '↓${monitor.hasTraffic ? formatRate(monitor.proxyDownBps) : '--'}',
                key: const ValueKey('status-proxy-speed'),
                style: muted,
              ),
              const _Sep(),
              Text(
                '直连 ↑${monitor.hasTraffic ? formatRate(monitor.directUpBps) : '--'} '
                '↓${monitor.hasTraffic ? formatRate(monitor.directDownBps) : '--'}',
                key: const ValueKey('status-direct-speed'),
                style: muted,
              ),
              const _Sep(),
              Text(
                '今日 ↑${monitor.hasTraffic ? formatTraffic(monitor.proxyUp) : '--'} '
                '↓${monitor.hasTraffic ? formatTraffic(monitor.proxyDown) : '--'}',
                key: const ValueKey('status-today-traffic'),
                style: muted,
              ),
              if (platform.error != null) ...<Widget>[
                const _Sep(),
                Text(
                  '系统代理错误 ${platform.error!.code}',
                  key: const ValueKey('status-sysproxy-error'),
                  style: TextStyle(fontSize: 11.5, color: scheme.error),
                ),
              ],
              if (platform.conflicts.isNotEmpty) ...<Widget>[
                const _Sep(),
                Text(
                  '系统代理冲突 ${platform.conflicts.map((c) => c.field).join(',')}',
                  key: const ValueKey('status-sysproxy-conflict'),
                  style: TextStyle(fontSize: 11.5, color: scheme.error),
                ),
              ],
              if (platform.message != null) ...<Widget>[
                const _Sep(),
                Text(
                  platform.message!,
                  key: const ValueKey('status-message'),
                  style: TextStyle(fontSize: 11.5, color: scheme.primary),
                ),
              ] else if (shell.message != null) ...<Widget>[
                const _Sep(),
                Text(
                  shell.message!,
                  key: const ValueKey('status-message'),
                  style: TextStyle(fontSize: 11.5, color: scheme.primary),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

/// T14 TUN actual-state label (see the `tun-actual` widget above).
///
/// `tunEnabled` is the *desired* switch from settings; `runtime` is the live
/// snapshot. Only a live runtime proves anything: without one the answer is
/// always 未启用, even when the switch is on.
String _tunActualLabel(bool tunEnabled, RuntimeView runtime) {
  if (!tunEnabled) return '未启用';
  if (runtime.error?.code == 'E_TUN_HELPER_UNAVAILABLE') return '失败已回滚';
  if (!runtime.isRunning) return '未启用';
  return '已请求(未验证)';
}

String _activeSchemeLabel(RoutingState routing) {
  for (final item in routing.items) {
    if (item.isActive) return item.remarks;
  }
  return routing.items.isEmpty ? '--' : routing.items.first.remarks;
}

/// Apply a system-proxy mode through the real bridge, computing the named
/// proxy / PAC URL from the persisted settings and the running inbound port.
void _applyMode(
  BuildContext context,
  WidgetRef ref,
  PlatformController controller,
  SysProxyMode mode,
) {
  // The settings document holds exceptions / advanced protocol / PAC path.
  final settings = ref.read(settingsControllerProvider);
  final config = ProxySettingsView.fromDocument(settings.document);
  final basePort = _firstInboundPort(settings.document);
  final server = buildProxyServer(
    port: basePort,
    advancedProtocol: config.advancedProtocol,
  );
  final bypass = buildProxyBypass(
    exceptions: config.exceptions,
    notProxyLocalAddress: config.notProxyLocalAddress,
  );
  switch (mode) {
    case SysProxyMode.pac:
      // PAC URL is only known once the PAC server is running; a real apply
      // happens from the settings/exit wiring which owns the PAC lifecycle.
      controller.setMessage('PAC 模式需要先启动 PAC 服务（设置/系统代理）');
    case SysProxyMode.unchanged:
      controller.apply(mode: mode);
    case SysProxyMode.forcedClear:
      controller.apply(mode: mode);
    case SysProxyMode.forcedChange:
      controller.apply(mode: mode, server: server, bypass: bypass);
  }
}

/// The first inbound `LocalPort` from the settings document (the base port).
int _firstInboundPort(Map<String, dynamic> document) {
  final inbound = document['Inbound'];
  if (inbound is List && inbound.isNotEmpty) {
    final first = inbound.first;
    if (first is Map<String, dynamic>) {
      return (first['LocalPort'] as num?)?.toInt() ?? 10808;
    }
  }
  return 10808;
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
