import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// Open the check-update window (ACT-WIN-005, F-APP-005/007, F-CORE-002/003).
Future<void> showCheckUpdateWindow(BuildContext context, WidgetRef ref) {
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => const CheckUpdateView(),
  );
}

class CheckUpdateView extends ConsumerWidget {
  const CheckUpdateView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(updateControllerProvider);
    final controller = ref.read(updateControllerProvider.notifier);
    final scheme = Theme.of(context).colorScheme;
    return AlertDialog(
      key: const ValueKey('update-window'),
      title: const Text('检查更新', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
      content: SizedBox(
        width: 760,
        height: 520,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: CheckboxListTile(
                    key: const ValueKey('update-prerelease'),
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    value: state.prerelease,
                    onChanged: state.busy
                        ? null
                        : (value) => controller.setPrerelease(value ?? false),
                    title: const Text(
                      '预览版（预发布）',
                      style: TextStyle(fontSize: 12),
                    ),
                  ),
                ),
                Expanded(
                  child: CheckboxListTile(
                    key: const ValueKey('update-via-proxy'),
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    value: state.viaProxy,
                    onChanged: state.busy
                        ? null
                        : (value) => controller.setViaProxy(value ?? false),
                    title: const Text('经代理', style: TextStyle(fontSize: 12)),
                  ),
                ),
              ],
            ),
            const Divider(height: 8),
            Expanded(
              child: ListView.builder(
                key: const ValueKey('update-target-list'),
                itemCount: state.targets.length,
                itemBuilder: (context, index) {
                  final target = state.targets[index];
                  return _TargetRow(
                    target: target,
                    check: state.checkFor(target.core),
                    selected: state.selected.contains(target.core),
                    onChanged: state.busy
                        ? null
                        : (value) => controller.toggleCore(
                            target.core,
                            value ?? false,
                          ),
                  );
                },
              ),
            ),
            const Divider(height: 8),
            _StatusArea(
              key: const ValueKey('update-status'),
              busy: state.busy,
              stage: state.stage,
              status: state.status,
            ),
            if (state.lastSpec?.ok == true)
              Padding(
                padding: const EdgeInsets.only(bottom: 4),
                child: Text(
                  '外部升级：${state.lastSpec!.helperExe} ← ${state.lastSpec!.source}',
                  style: TextStyle(
                    fontSize: 11,
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('update-check-only-btn'),
          onPressed: state.busy ? null : controller.checkOnly,
          child: const Text('仅检查', style: TextStyle(fontSize: 12)),
        ),
        FilledButton(
          key: const ValueKey('update-apply-btn'),
          onPressed: state.busy ? null : controller.checkAndApply,
          child: const Text('检查更新', style: TextStyle(fontSize: 12)),
        ),
        OutlinedButton(
          key: const ValueKey('update-app-spec-btn'),
          onPressed: state.busy ? null : controller.applyAppUpdate,
          child: const Text('应用自身更新', style: TextStyle(fontSize: 12)),
        ),
        TextButton(
          key: const ValueKey('update-close'),
          onPressed: () => Navigator.pop(context),
          child: const Text('关闭'),
        ),
      ],
    );
  }
}

class _TargetRow extends StatelessWidget {
  const _TargetRow({
    required this.target,
    required this.check,
    required this.selected,
    required this.onChanged,
  });

  final c.UpdateTargetDto target;
  final c.CoreUpdateDto? check;
  final bool selected;
  final ValueChanged<bool?>? onChanged;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final enabled = target.supported;
    final blocked = check != null && !check!.supported;
    final label = check == null
        ? '—'
        : blocked
        ? _noteLabel(check!.note)
        : check!.hasUpdate
        ? '可更新 ${check!.remoteVersion ?? ''}'
        : '已是最新 ${check!.remoteVersion ?? ''}';
    return Opacity(
      opacity: enabled ? 1 : 0.5,
      child: Row(
        children: <Widget>[
          SizedBox(
            width: 40,
            child: Checkbox(
              key: ValueKey('update-core-${target.core}'),
              value: enabled && selected,
              onChanged: enabled ? onChanged : null,
            ),
          ),
          SizedBox(
            width: 96,
            child: Text(
              target.core,
              style: const TextStyle(fontSize: 12),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          Expanded(
            child: Text(
              target.maxVersion == null
                  ? target.repo
                  : '${target.repo}（上限 ${target.maxVersion}）',
              style: const TextStyle(fontSize: 12),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          SizedBox(
            width: 150,
            child: Text(
              enabled ? label : '不支持更新',
              textAlign: TextAlign.right,
              style: TextStyle(
                fontSize: 12,
                color: blocked
                    ? scheme.error
                    : enabled && check?.hasUpdate == true
                    ? scheme.primary
                    : scheme.onSurfaceVariant,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}

/// Human-readable text for a per-target check note. A missing own release
/// source is shown as an explicit block, never as "up to date" (R3-08).
String _noteLabel(String? note) {
  switch (note) {
    case 'error.update_app_source_unconfigured':
      return '应用自身发行源未配置';
    case 'error.update_unsupported':
      return '不支持更新';
    default:
      return (note == null || note.isEmpty) ? '不可用' : note;
  }
}

class _StatusArea extends StatelessWidget {
  const _StatusArea({
    super.key,
    required this.busy,
    required this.stage,
    required this.status,
  });

  final bool busy;
  final String? stage;
  final UpdateStatus? status;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    // Stage text only; never a fabricated percentage.
    final message = busy
        ? (stage ?? '处理中…')
        : status == null
        ? '就绪'
        : status!.detail == null
        ? status!.message
        : '${status!.message} — ${status!.detail}';
    final color = status?.isError == true
        ? scheme.error
        : status?.isSuccess == true
        ? scheme.primary
        : scheme.onSurfaceVariant;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      color: scheme.surfaceContainerHighest,
      child: Text(message, style: TextStyle(fontSize: 12, color: color)),
    );
  }
}
