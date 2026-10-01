import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';

/// Add a node of [configType] through the real editor + bridge.
Future<void> startAddProfile(
  BuildContext context,
  WidgetRef ref,
  ConfigType configType,
) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final draft = controller.newDraft(configType);
  final saved = await showProfileEditor(
    context,
    initial: draft,
    onSave: controller.saveDraft,
  );
  _toast(ref, saved == null ? '已取消添加' : '已保存 ${saved.configType.name}');
}

/// Edit the single selected node (Ctrl+D / context menu / double click).
Future<void> editSelectedProfile(BuildContext context, WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.length != 1) {
    _toast(ref, state.selected.isEmpty ? '请先选择节点' : '请选择单个节点后编辑');
    return;
  }
  final id = state.selected.first;
  final dto = controller.profileById(id);
  if (dto == null) {
    _toast(ref, '未找到节点 $id');
    return;
  }
  final saved = await showProfileEditor(
    context,
    initial: ProfileDraft.fromDto(dto),
    onSave: controller.saveDraft,
    allowConfigTypeChange: false,
  );
  _toast(ref, saved == null ? '已取消编辑' : '已保存 ${saved.remarks}');
}

/// Delete the selection after an explicit confirmation.
Future<void> deleteSelectedProfiles(BuildContext context, WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.isEmpty) {
    _toast(ref, '请先选择要删除的节点');
    return;
  }
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('delete-confirm'),
      title: const Text('删除节点', style: TextStyle(fontSize: 15)),
      content: Text('确认删除选中的 ${state.selected.length} 个节点?'),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('delete-cancel'),
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('delete-confirm-ok'),
          onPressed: () => Navigator.of(context).pop(true),
          child: const Text('删除'),
        ),
      ],
    ),
  );
  if (confirmed != true) {
    _toast(ref, '已取消删除');
    return;
  }
  final result = controller.deleteSelected();
  _toast(ref, result.ok ? '已删除 ${result.removed} 个节点' : '删除失败');
}

/// Clone the selection.
Future<void> copySelectedProfiles(WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final result = controller.copySelected();
  _toast(ref, result.ok ? '已复制 ${result.copies.length} 个节点' : '复制失败');
}

/// Rename the single selected node.
Future<void> renameSelectedProfile(BuildContext context, WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.length != 1) {
    _toast(ref, '请选择单个节点后修改备注');
    return;
  }
  final id = state.selected.first;
  final current = controller.profileById(id);
  final textController = TextEditingController(text: current?.remarks ?? '');
  final remarks = await showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      key: const ValueKey('remarks-dialog'),
      title: const Text('修改备注', style: TextStyle(fontSize: 15)),
      content: TextField(
        key: const ValueKey('remarks-field'),
        controller: textController,
        autofocus: true,
        decoration: const InputDecoration(
          isDense: true,
          border: OutlineInputBorder(),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('remarks-cancel'),
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('remarks-save'),
          onPressed: () => Navigator.of(context).pop(textController.text),
          child: const Text('保存'),
        ),
      ],
    ),
  );
  if (remarks == null) {
    _toast(ref, '已取消修改备注');
    return;
  }
  final result = controller.renameProfile(id, remarks);
  _toast(ref, result.ok ? '备注已更新' : '备注更新失败');
}

/// Enable/disable the active node (persisted across restarts).
Future<void> toggleActiveSelected(WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.length != 1) {
    _toast(ref, '请选择单个节点后启用/停用');
    return;
  }
  final id = state.selected.first;
  final next = state.activeId == id ? null : id;
  final result = controller.setActive(next);
  controller.logAction(ProfileAction.activate, 'id=${next ?? "(none)"}');
  _toast(ref, result.ok ? (next == null ? '已停用活动节点' : '已启用为活动节点') : '操作失败');
}

void _toast(WidgetRef ref, String message) {
  ref.read(uiShellControllerProvider.notifier).setMessage(message);
}
