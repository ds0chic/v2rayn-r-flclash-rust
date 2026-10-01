import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
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
              PopupMenuButton<int>(
                key: const ValueKey('routing-selector'),
                tooltip: '路由',
                itemBuilder: (context) => const <PopupMenuEntry<int>>[
                  PopupMenuItem<int>(
                    value: 0,
                    child: Text('(无路由配置)', style: TextStyle(fontSize: 12)),
                  ),
                ],
                child: Text('路由: ${shell.routingLabel ?? '--'}', style: muted),
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
                '运行信息: ${shell.runningNode == null ? '未运行' : '就绪'}',
                key: const ValueKey('running-info'),
                style: muted,
              ),
              const _Sep(),
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
