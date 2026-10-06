import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/features/subs/sub_edit_window.dart';
import 'package:v2rayn_desktop/features/subs/sub_share_dialog.dart';
import 'package:v2rayn_desktop/shared/widgets/app_dialog.dart';

/// 订阅设置窗删除：先确认后删除（上游 `SubSettingViewModel.DeleteSubAsync`
/// counterpart：`ShowYesNoInteraction(ResUI.RemoveServer)` 取消直接 return）。
///
/// [confirmDelete] 覆盖确认框（默认弹确认框），便于测试注入取消/确认。
/// 取消不写库；删除失败在窗内状态行报出错误并保留该组（控制器 delete 仅
/// 成功才刷新/resync）。返回 true=已删除。
Future<bool> confirmAndDeleteSub(
  BuildContext context,
  WidgetRef ref,
  String id, {
  Future<bool> Function(String remarks)? confirmDelete,
}) async {
  final items = ref.read(subsControllerProvider).items;
  final current = items.where((s) => s.id == id).firstOrNull;
  if (current == null) return false;
  final confirmed = await (confirmDelete != null
      ? confirmDelete(current.remarks)
      : showAppConfirmDialog(
          context,
          title: '删除订阅',
          message: '确认删除订阅“${current.remarks}”？',
          confirmLabel: '删除',
          destructive: true,
          dialogKey: const ValueKey('sub-delete-confirm'),
          confirmKey: const ValueKey('sub-delete-confirm-ok'),
          cancelKey: const ValueKey('sub-delete-cancel'),
        ));
  if (!confirmed) return false;
  final result = ref.read(subsControllerProvider.notifier).delete(<String>[
    current.id,
  ]);
  if (!result.ok) {
    final code = result.error?.messageKey ?? result.error?.code ?? '未知错误';
    ref
        .read(subsControllerProvider.notifier)
        .setStatus(SubStatus(kind: 'error', message: '订阅删除失败', detail: code));
    return false;
  }
  return true;
}

/// The subscription settings window (upstream `SubSettingWindow`,
/// F-SUB-001/002/003). Opened from the 订阅分组 menu (ACT-MAIN-019).
Future<void> showSubSettingWindow(BuildContext context, WidgetRef ref) {
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => const SubSettingWindow(),
  );
}

class SubSettingWindow extends ConsumerStatefulWidget {
  const SubSettingWindow({super.key});

  @override
  ConsumerState<SubSettingWindow> createState() => _SubSettingWindowState();
}

class _SubSettingWindowState extends ConsumerState<SubSettingWindow> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(subsControllerProvider.notifier).reload();
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(subsControllerProvider);
    final controller = ref.read(subsControllerProvider.notifier);
    return AlertDialog(
      key: const ValueKey('sub-setting-window'),
      title: const Text('订阅分组设置', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(0, 12, 0, 0),
      content: SizedBox(
        width: 720,
        height: 420,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            const _SubTableHeader(),
            const Divider(height: 1),
            Expanded(
              child: state.items.isEmpty
                  ? const Center(
                      key: ValueKey('sub-empty'),
                      child: Text('暂无订阅，请点击“新增”添加'),
                    )
                  : ListView.builder(
                      key: const ValueKey('sub-list'),
                      itemCount: state.items.length,
                      itemBuilder: (context, index) {
                        final item = state.items[index];
                        return _SubRow(
                          item: item,
                          selected: item.id == state.selectedId,
                          onTap: () => controller.select(item.id),
                          onToggle: (v) => controller.setEnabled(item.id, v),
                          onContext: () => _showContextMenu(context, item),
                        );
                      },
                    ),
            ),
            if (state.status != null) _StatusLine(status: state.status!),
          ],
        ),
      ),
      actions: <Widget>[
        _action('新增', const ValueKey('sub-add'), () => _add(context)),
        _action(
          '删除',
          const ValueKey('sub-delete'),
          state.selectedId == null ? null : () => _delete(context, state),
        ),
        _action(
          '编辑',
          const ValueKey('sub-edit'),
          state.selectedId == null ? null : () => _edit(context, state),
        ),
        _action(
          '分享',
          const ValueKey('sub-share'),
          () => _share(context, state),
        ),
        _action(
          '关闭',
          const ValueKey('sub-close'),
          () => Navigator.pop(context),
        ),
      ],
    );
  }

  Future<void> _add(BuildContext context) async {
    final controller = ref.read(subsControllerProvider.notifier);
    final saved = await showSubEditWindow(context, controller.newDraft());
    if (saved != null) controller.save(saved);
  }

  Future<void> _edit(BuildContext context, SubsState state) async {
    final current = state.selected;
    if (current == null) return;
    final controller = ref.read(subsControllerProvider.notifier);
    final saved = await showSubEditWindow(context, current);
    if (saved != null) controller.save(saved);
  }

  Future<void> _delete(BuildContext context, SubsState state) async {
    final id = state.selectedId;
    if (id == null) return;
    await confirmAndDeleteSub(context, ref, id);
  }

  void _share(BuildContext context, SubsState state) {
    final item = state.selected;
    if (item == null) return;
    showSubShareDialog(context, item);
  }

  void _showContextMenu(BuildContext context, c.SubItemDto item) {
    final controller = ref.read(subsControllerProvider.notifier);
    final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
    showMenu<String>(
      context: context,
      position: RelativeRect.fromLTRB(
        overlay.size.width / 3,
        overlay.size.height / 3,
        overlay.size.width / 3,
        overlay.size.height / 3,
      ),
      items: <PopupMenuEntry<String>>[
        const PopupMenuItem(value: 'edit', child: Text('编辑')),
        const PopupMenuItem(value: 'share', child: Text('分享')),
        PopupMenuItem(
          value: item.enabled ? 'disable' : 'enable',
          child: Text(item.enabled ? '停用' : '启用'),
        ),
        const PopupMenuItem(value: 'update', child: Text('更新')),
        const PopupMenuItem(value: 'delete', child: Text('删除')),
      ],
    ).then((value) async {
      if (value == null || !context.mounted) return;
      switch (value) {
        case 'edit':
          controller.select(item.id);
          _edit(context, ref.read(subsControllerProvider));
        case 'share':
          showSubShareDialog(context, item);
        case 'enable':
        case 'disable':
          controller.setEnabled(item.id, value == 'enable');
        case 'update':
          controller.update(subIds: <String>[item.id]);
        case 'delete':
          controller.select(item.id);
          if (context.mounted) {
            await confirmAndDeleteSub(context, ref, item.id);
          }
      }
    });
  }

  Widget _action(String label, Key key, VoidCallback? onPressed) => TextButton(
    key: key,
    onPressed: onPressed,
    child: Text(label, style: const TextStyle(fontSize: 13)),
  );
}

class _SubTableHeader extends StatelessWidget {
  const _SubTableHeader();

  @override
  Widget build(BuildContext context) {
    const style = TextStyle(fontSize: 12, fontWeight: FontWeight.w600);
    return const Padding(
      padding: EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      child: Row(
        children: <Widget>[
          SizedBox(width: 40, child: Text('启用', style: style)),
          Expanded(flex: 3, child: Text('备注', style: style)),
          Expanded(flex: 5, child: Text('Url', style: style)),
          SizedBox(width: 56, child: Text('间隔', style: style)),
          Expanded(flex: 2, child: Text('UserAgent', style: style)),
          SizedBox(width: 40, child: Text('排序', style: style)),
        ],
      ),
    );
  }
}

class _SubRow extends StatelessWidget {
  const _SubRow({
    required this.item,
    required this.selected,
    required this.onTap,
    required this.onToggle,
    required this.onContext,
  });

  final c.SubItemDto item;
  final bool selected;
  final VoidCallback onTap;
  final ValueChanged<bool> onToggle;
  final VoidCallback onContext;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return InkWell(
      key: ValueKey('sub-row-${item.id}'),
      onTap: onTap,
      onSecondaryTap: onContext,
      child: Container(
        color: selected ? scheme.primaryContainer : null,
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
        child: Row(
          children: <Widget>[
            SizedBox(
              width: 40,
              child: Checkbox(
                key: ValueKey('sub-enabled-${item.id}'),
                value: item.enabled,
                onChanged: (v) => onToggle(v ?? false),
              ),
            ),
            Expanded(
              flex: 3,
              child: Text(
                item.remarks.isEmpty ? '(未命名)' : item.remarks,
                style: const TextStyle(fontSize: 12),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            Expanded(
              flex: 5,
              child: Text(
                item.url,
                style: const TextStyle(fontSize: 12),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            SizedBox(
              width: 56,
              child: Text(
                item.autoUpdateInterval > 0
                    ? '${item.autoUpdateInterval} 分'
                    : '手动',
                style: const TextStyle(fontSize: 12),
              ),
            ),
            Expanded(
              flex: 2,
              child: Text(
                item.userAgent,
                style: const TextStyle(fontSize: 12),
                overflow: TextOverflow.ellipsis,
              ),
            ),
            SizedBox(
              width: 40,
              child: Text('${item.sort}', style: const TextStyle(fontSize: 12)),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusLine extends StatelessWidget {
  const _StatusLine({required this.status});

  final SubStatus status;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final color = status.isError
        ? scheme.error
        : status.isSuccess
        ? scheme.primary
        : scheme.onSurfaceVariant;
    return Container(
      key: const ValueKey('sub-status'),
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      color: scheme.surfaceContainerHighest,
      child: Text(
        status.detail == null
            ? status.message
            : '${status.message} — ${status.detail}',
        style: TextStyle(fontSize: 12, color: color),
      ),
    );
  }
}
