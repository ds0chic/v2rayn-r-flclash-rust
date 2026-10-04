import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_format.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
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
                value: desiredTun,
                onChanged: (value) => _onTunToggle(ref, value),
              ),
              // FIX-13: the desired flag is persisted to `TunModeItem.EnableTun`
              // and then the real plan is re-applied. The actual label is read
              // from the runtime snapshot, never from the switch: no live
              // runtime (or switch off) reads 未启用; a helper refusal reads
              // 失败已回滚; a live runtime reads 已请求(未验证).
              Text(
                '实际: ${tunActualLabel(desiredTun, runtime)}',
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
                    platformController.applyModeFromConfig(mode),
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
                onSelected: routingController.setDefaultAndReload,
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
              // R3-09b: "今日" is the aggregate of the live per-node
              // `ServerStatItem` today counters (upstream `StatisticsManager`
              // semantics), not the session-cumulative proxy counter. With no
              // node rows (no applied session / statistics disabled) it shows
              // `--` instead of a fabricated zero.
              Text(
                '今日 ↑${monitor.hasTodayNodes ? formatTraffic(monitor.todayUp) : '--'} '
                '↓${monitor.hasTodayNodes ? formatTraffic(monitor.todayDown) : '--'}',
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
Future<void> _onTunToggle(WidgetRef ref, bool value) async {
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
          'TUN 保存失败: ${saved.error?.messageKey ?? 'error.settings_save_failed'}',
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
    shell.setMessage('TUN 授权被拒绝或 helper 不可用，运行状态未改变');
  } else {
    shell.setMessage(value ? 'TUN 已保存并应用' : 'TUN 已关闭并应用');
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
