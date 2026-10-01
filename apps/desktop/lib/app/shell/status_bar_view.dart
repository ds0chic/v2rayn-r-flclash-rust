import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
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
              const _Sep(),
              PopupMenuButton<int>(
                key: const ValueKey('system-proxy-selector'),
                tooltip: '系统代理 (T13 接入)',
                onSelected: shellController.setSystemProxyIndex,
                itemBuilder: (context) => <PopupMenuEntry<int>>[
                  for (var i = 0; i < SystemProxyMode.modes.length; i++)
                    PopupMenuItem<int>(
                      value: i,
                      child: Text(
                        SystemProxyMode.modes[i].label,
                        style: const TextStyle(fontSize: 12),
                      ),
                    ),
                ],
                child: Text(
                  '系统代理: ${SystemProxyMode.modes[shell.systemProxyIndex].label}',
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
                runtime.revisionLabel,
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
              Text(
                '代理 ↑${shell.proxySpeed.up} ↓${shell.proxySpeed.down}',
                key: const ValueKey('status-proxy-speed'),
                style: muted,
              ),
              const _Sep(),
              Text(
                '直连 ↑${shell.directSpeed.up} ↓${shell.directSpeed.down}',
                key: const ValueKey('status-direct-speed'),
                style: muted,
              ),
              if (shell.message != null) ...<Widget>[
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
