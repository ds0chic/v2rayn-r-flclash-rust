import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/custom_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';
import 'package:v2rayn_desktop/features/profiles/profile_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/template_window.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/shared/widgets/app_dialog.dart';

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

/// Which dedicated editor an existing node must open in.
///
/// Mirrors upstream `ProfilesViewModel.EditServerAsync` (frozen `7d6a967`):
/// Custom/Outbound -> AddServer2, PolicyGroup/ProxyChain (`IsGroupType`) ->
/// AddGroup, everything else -> AddServer (the generic editor). Routing by
/// [ConfigType] keeps special nodes out of the generic editor's core/port/
/// network clamps (PR-02).
enum SpecialEditorKind { generic, custom, group }

/// Pure routing decision for [editSelectedProfile], unit-tested without widgets.
SpecialEditorKind resolveEditorKind(ConfigType configType) {
  switch (configType) {
    case ConfigType.custom:
    case ConfigType.outbound:
      return SpecialEditorKind.custom;
    case ConfigType.policyGroup:
    case ConfigType.proxyChain:
      return SpecialEditorKind.group;
    default:
      return SpecialEditorKind.generic;
  }
}

/// Edit the single selected node (Ctrl+D / context menu / double click).
///
/// The editor is chosen by [resolveEditorKind]: special nodes reopen in their
/// dedicated editor, ordinary nodes in the generic one. Cancel never persists
/// (each dialog edits a local [ProfileDraft] copy).
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
  switch (resolveEditorKind(dto.configType)) {
    case SpecialEditorKind.custom:
      final saved = await showCustomEditor(
        context,
        initial: ProfileDraft.fromDto(dto),
        onSave: controller.saveDraft,
      );
      _toast(ref, saved == null ? '已取消编辑' : '已保存 ${saved.remarks}');
    case SpecialEditorKind.group:
      final saved = await showGroupEditor(
        context,
        initial: ProfileDraft.fromDto(dto),
        allProfiles: state.profiles,
        subItems: controller.subItems(),
        previewChildren: controller.groupChildPreview,
        onSave: controller.saveDraft,
      );
      _toast(ref, saved == null ? '已取消编辑' : '已保存 ${saved.remarks}');
    case SpecialEditorKind.generic:
      final saved = await showProfileEditor(
        context,
        initial: ProfileDraft.fromDto(dto),
        onSave: controller.saveDraft,
        allowConfigTypeChange: false,
      );
      _toast(ref, saved == null ? '已取消编辑' : '已保存 ${saved.remarks}');
  }
}

/// Delete the selection after an explicit confirmation.
Future<void> deleteSelectedProfiles(BuildContext context, WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.isEmpty) {
    _toast(ref, '请先选择要删除的节点');
    return;
  }
  final confirmed = await showAppConfirmDialog(
    context,
    title: '删除节点',
    message: '确认删除选中的 ${state.selected.length} 个节点?',
    confirmLabel: '删除',
    destructive: true,
    dialogKey: const ValueKey('delete-confirm'),
    confirmKey: const ValueKey('delete-confirm-ok'),
    cancelKey: const ValueKey('delete-cancel'),
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

/// Set the selected node as the active node and reload the managed core.
///
/// Upstream `ProfilesViewModel.SetDefaultServer`: selecting the node that is
/// already active returns without clearing it, and a successful switch ends in
/// `MainWindowViewModel.Reload()` which loads the core with the new default
/// server. There is no "deactivate" path through this command.
Future<void> setActiveSelected(WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.length != 1) {
    _toast(ref, '请选择单个节点后设为活动');
    return;
  }
  final id = state.selected.first;
  if (state.activeId == id) {
    _toast(ref, '该节点已是活动节点');
    return;
  }
  final result = controller.setActive(id);
  if (!result.ok) {
    _toast(ref, '操作失败');
    return;
  }
  controller.logAction(ProfileAction.activate, 'id=$id');
  _toast(ref, '已设为活动节点');
  await ref.read(runtimeControllerProvider.notifier).applyActive();
}

/// Add a PolicyGroup / ProxyChain node through the group editor + bridge.
Future<void> startAddGroupProfile(
  BuildContext context,
  WidgetRef ref,
  ConfigType configType,
) async {
  assert(
    configType == ConfigType.policyGroup || configType == ConfigType.proxyChain,
  );
  final controller = ref.read(profilesControllerProvider.notifier);
  final draft = controller.newDraft(configType)
    ..coreType = CoreType.xray
    ..port = 0
    ..address = '';
  final saved = await showGroupEditor(
    context,
    initial: draft,
    allProfiles: ref.read(profilesControllerProvider).profiles,
    subItems: controller.subItems(),
    previewChildren: controller.groupChildPreview,
    onSave: controller.saveDraft,
  );
  _toast(ref, saved == null ? '已取消添加' : '已保存 ${saved.remarks}');
}

/// Edit the single selected group/chain node.
Future<void> editSelectedGroup(BuildContext context, WidgetRef ref) async {
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
  if (dto.configType != ConfigType.policyGroup &&
      dto.configType != ConfigType.proxyChain) {
    _toast(ref, '所选不是策略组/代理链节点');
    return;
  }
  final saved = await showGroupEditor(
    context,
    initial: ProfileDraft.fromDto(dto),
    allProfiles: state.profiles,
    subItems: controller.subItems(),
    previewChildren: controller.groupChildPreview,
    onSave: controller.saveDraft,
  );
  _toast(ref, saved == null ? '已取消编辑' : '已保存 ${saved.remarks}');
}

/// Add a Custom / Outbound node through the AddServer2 editor + bridge.
Future<void> startAddCustomProfile(
  BuildContext context,
  WidgetRef ref,
  ConfigType configType,
) async {
  assert(configType == ConfigType.custom || configType == ConfigType.outbound);
  final controller = ref.read(profilesControllerProvider.notifier);
  final draft = controller.newDraft(configType)
    ..port = 0
    ..address = '';
  final saved = await showCustomEditor(
    context,
    initial: draft,
    onSave: controller.saveDraft,
  );
  _toast(ref, saved == null ? '已取消添加' : '已保存 ${saved.remarks}');
}

/// Edit the single selected Custom / Outbound node.
Future<void> editSelectedCustom(BuildContext context, WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.length != 1) {
    _toast(ref, state.selected.isEmpty ? '请先选择节点' : '请选择单个节点后编辑');
    return;
  }
  final dto = controller.profileById(state.selected.first);
  if (dto == null) {
    _toast(ref, '未找到节点');
    return;
  }
  if (dto.configType != ConfigType.custom &&
      dto.configType != ConfigType.outbound) {
    _toast(ref, '所选不是自定义配置/出站节点');
    return;
  }
  final saved = await showCustomEditor(
    context,
    initial: ProfileDraft.fromDto(dto),
    onSave: controller.saveDraft,
  );
  _toast(ref, saved == null ? '已取消编辑' : '已保存 ${saved.remarks}');
}

/// Open the full-config template settings window.
Future<void> openFullConfigTemplateWindow(
  BuildContext context,
  WidgetRef ref,
) async {
  final bridge = ref.read(bridgePortProvider);
  final controller = ref.read(profilesControllerProvider.notifier);
  final saved = await showFullConfigTemplateWindow(
    context,
    initial: bridge.listTemplates().items,
    onSave: (item) => bridge.saveTemplate(item),
  );
  if (saved == true) {
    controller.reload();
    _toast(ref, '完整配置模板已保存');
  } else {
    _toast(ref, '已取消模板设置');
  }
}

void _toast(WidgetRef ref, String message) {
  ref.read(uiShellControllerProvider.notifier).setMessage(message);
}
