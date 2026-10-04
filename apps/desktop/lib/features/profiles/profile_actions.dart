import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
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

/// Save-location picker for the full client-config file export. Returns the
/// chosen path, or null when the user cancels. Overridable in widget tests so
/// the OS dialog is never opened.
typedef ClientConfigSavePicker = Future<String?> Function(String suggestedName);

final clientConfigSavePickerProvider = Provider<ClientConfigSavePicker>((ref) {
  return (String suggestedName) async {
    final location = await getSaveLocation(
      suggestedName: suggestedName,
      acceptedTypeGroups: <XTypeGroup>[
        const XTypeGroup(label: '配置', extensions: <String>['json']),
      ],
    );
    return location?.path;
  };
});

/// Export the single selected (or active) node's complete client configuration.
///
/// Upstream `Export2ClientConfigAsync(blClipboard)` /
/// `Export2ClientConfigResult`: the node's generated kernel config is copied to
/// the clipboard, or written to a user-chosen file after the save dialog. A
/// cancel never writes; a write failure is reported and never faked as success.
Future<void> exportSelectedClientConfig(
  BuildContext context,
  WidgetRef ref, {
  required bool toClipboard,
}) async {
  final state = ref.read(profilesControllerProvider);
  final String? id;
  if (state.selected.length == 1) {
    id = state.selected.first;
  } else if (state.selected.isEmpty) {
    id = state.activeId;
  } else {
    id = null;
  }
  if (id == null) {
    _toast(ref, state.selected.length > 1 ? '请选择单个节点后导出' : '请先选择节点');
    return;
  }

  final bridge = ref.read(bridgePortProvider);
  final result = bridge.exportClientConfigText(id);
  if (!result.ok) {
    _toast(ref, '导出失败：${result.error?.messageKey ?? "无导出项"}');
    return;
  }

  if (toClipboard) {
    await Clipboard.setData(ClipboardData(text: result.text));
    _toast(ref, '已复制完整配置到剪贴板');
    return;
  }

  final picker = ref.read(clientConfigSavePickerProvider);
  final path = await picker('config.json');
  if (path == null || path.trim().isEmpty) {
    return;
  }
  final write = bridge.writeExportFile(path, result.text);
  if (!write.ok) {
    _toast(ref, '保存失败：${write.error?.messageKey ?? "写入被拒绝"}');
    return;
  }
  _toast(ref, '已保存完整配置：$path');
}

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
  // Upstream `EditServerAsync`: a successful save of the active node ends in
  // `Reload()`, which re-applies the running core.
  final wasActive = state.activeId == id;
  final bridge = ref.read(bridgePortProvider);
  final c.ProfileDto? saved;
  switch (resolveEditorKind(dto.configType)) {
    case SpecialEditorKind.custom:
      saved = await showCustomEditor(
        context,
        initial: ProfileDraft.fromDto(dto),
        onSave: controller.saveDraft,
        customImportFile: bridge.customImportFile,
        dataDir: bridge.dataDir,
      );
    case SpecialEditorKind.group:
      saved = await showGroupEditor(
        context,
        initial: ProfileDraft.fromDto(dto),
        allProfiles: state.profiles,
        subItems: controller.subItems(),
        onSave: controller.saveDraft,
      );
    case SpecialEditorKind.generic:
      saved = await showProfileEditor(
        context,
        initial: ProfileDraft.fromDto(dto),
        onSave: controller.saveDraft,
        allowConfigTypeChange: false,
      );
  }
  _toast(ref, saved == null ? '已取消编辑' : '已保存 ${saved.remarks}');
  if (saved != null && wasActive) {
    await applyAfterEditIfActive(ref, id);
  }
}

/// Delete the selection after an explicit confirmation.
///
/// Upstream `RemoveServerAsync`: after the confirmation the profiles are
/// removed and, when the active node was among them, `Reload()` runs.
/// `ConfigHandler.SetDefaultServer` then picks a replacement before applying.
Future<void> deleteSelectedProfiles(BuildContext context, WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final state = ref.read(profilesControllerProvider);
  if (state.selected.isEmpty) {
    _toast(ref, '请先选择要删除的节点');
    return;
  }
  final activeBefore = state.activeId;
  final removedActive =
      activeBefore != null && state.selected.contains(activeBefore);
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
  if (!result.ok) {
    _toast(ref, '删除失败');
    return;
  }
  if (removedActive) {
    await reconcileActiveAfterRemoval(ref);
  }
  _toast(ref, '已删除 ${result.removed} 个节点');
}

/// Upstream `ConfigHandler.SetDefaultServer` fallback after the active node was
/// removed: prefer a `Port > 0` node from the current visible list, then the
/// whole stored set; with no candidate the active id is cleared and nothing is
/// applied. Returns the chosen id, or null when the active id was cleared.
Future<String?> reconcileActiveAfterRemoval(WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final candidate = nextActiveAfterRemoval(
    ref.read(profilesControllerProvider),
  );
  if (candidate == null) {
    controller.setActive(null);
    controller.logAction(ProfileAction.activate, 'id=(none) after delete');
    return null;
  }
  final outcome = await activateProfileDetailed(ref, candidate);
  if (!outcome.persisted) {
    _toast(ref, '删除后未能设置新的活动节点');
    return null;
  }
  return candidate;
}

/// Pure fallback pick (upstream `SetDefaultServer`): first `Port > 0` row of
/// the current visible list that still exists in the store, else the first
/// stored `Port > 0` node. Null means "no candidate, clear the active id".
String? nextActiveAfterRemoval(ProfilesState state) {
  final stored = <String, c.ProfileDto>{
    for (final p in state.profiles) p.indexId: p,
  };
  for (final row in state.visible) {
    final profile = stored[row.id];
    if (profile != null && profile.port > 0) return row.id;
  }
  for (final profile in state.profiles) {
    if (profile.port > 0) return profile.indexId;
  }
  return null;
}

/// Applies the runtime after an edit that targeted the active node; a no-op for
/// any other node (the table reload already happened in `saveDraft`).
Future<void> applyAfterEditIfActive(WidgetRef ref, String editedId) async {
  if (ref.read(profilesControllerProvider).activeId != editedId) return;
  await _applyRuntime(ref);
}

/// Run the shared runtime apply and report whether it succeeded. A failed
/// apply keeps its structured error in the runtime state (never a fake ok).
Future<bool> _applyRuntime(WidgetRef ref) async {
  await ref.read(runtimeControllerProvider.notifier).applyActive();
  return ref.read(runtimeControllerProvider).error == null;
}

/// Clone the selection.
Future<void> copySelectedProfiles(WidgetRef ref) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  final result = controller.copySelected();
  _toast(ref, result.ok ? '已复制 ${result.copies.length} 个节点' : '复制失败');
}

/// Ctrl+C in the node table: copy the selected nodes' share URIs to the
/// clipboard, mirroring upstream `ProfilesViewModel.Export2ShareUrlAsync(false)`
/// (ACT-PROF-024). This is deliberately *not* [copySelectedProfiles], which is
/// the `复制` clone command (ACT-PROF-004) and must keep its row-creating
/// semantics.
///
/// Upstream batch-exports every selected node, and returns silently with no
/// selection (`GetProfileItems(true)` null), so an empty selection never
/// changes the clipboard nor shows a message.
Future<void> exportSelectedShareUrls(WidgetRef ref) async {
  final state = ref.read(profilesControllerProvider);
  if (state.selected.isEmpty) return;
  final bridge = ref.read(bridgePortProvider);
  final result = await bridge.exportProfiles(state.selected.toList(), 'share');
  if (!result.ok) {
    _toast(ref, '导出失败：${result.error?.messageKey ?? "无导出项"}');
    return;
  }
  await Clipboard.setData(ClipboardData(text: result.text));
  _toast(ref, '已导出 ${result.count} 个节点到剪贴板');
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

/// Outcome of activating a node: persistence and runtime apply are reported
/// separately so a stored-but-failed apply is never shown as full success.
class ActivationOutcome {
  const ActivationOutcome({
    required this.persisted,
    required this.applied,
    this.applyErrorCode,
  });

  final bool persisted;
  final bool applied;
  final String? applyErrorCode;

  bool get fullyOk => persisted && applied;
}

/// Activate one node by stable id and reload the managed core.
///
/// Shared by the table command and the tray node submenu (RT-11) so both run
/// the same use case. Re-selecting the active node is a no-op, mirroring
/// upstream `SetDefaultServer`. Returns the detailed outcome so callers can tell
/// a rejected persist from a failed runtime apply.
Future<ActivationOutcome> activateProfileDetailed(
  WidgetRef ref,
  String id,
) async {
  final controller = ref.read(profilesControllerProvider.notifier);
  if (ref.read(profilesControllerProvider).activeId == id) {
    return const ActivationOutcome(persisted: true, applied: true);
  }
  final result = controller.setActive(id);
  if (!result.ok) {
    return ActivationOutcome(
      persisted: false,
      applied: false,
      applyErrorCode: result.error?.code,
    );
  }
  controller.logAction(ProfileAction.activate, 'id=$id');
  final applied = await _applyRuntime(ref);
  return ActivationOutcome(
    persisted: true,
    applied: applied,
    applyErrorCode: applied
        ? null
        : ref.read(runtimeControllerProvider).error?.code,
  );
}

/// Bool wrapper kept for callers that only care about persistence (tray menu).
Future<bool> activateProfileById(WidgetRef ref, String id) async =>
    (await activateProfileDetailed(ref, id)).persisted;

/// Set the selected node as the active node and reload the managed core.
///
/// Upstream `ProfilesViewModel.SetDefaultServer`: selecting the node that is
/// already active returns without clearing it, and a successful switch ends in
/// `MainWindowViewModel.Reload()` which loads the core with the new default
/// server. There is no "deactivate" path through this command.
Future<void> setActiveSelected(WidgetRef ref) async {
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
  final outcome = await activateProfileDetailed(ref, id);
  if (!outcome.persisted) {
    _toast(
      ref,
      '设为活动节点失败${outcome.applyErrorCode == null ? '' : '（${outcome.applyErrorCode}）'}',
    );
  } else if (outcome.applied) {
    _toast(ref, '已设为活动节点');
  } else {
    _toast(
      ref,
      '已设为活动节点，但运行应用失败${outcome.applyErrorCode == null ? '' : '（${outcome.applyErrorCode}）'}',
    );
  }
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
  final bridge = ref.read(bridgePortProvider);
  final draft = controller.newDraft(configType)
    ..port = 0
    ..address = '';
  final saved = await showCustomEditor(
    context,
    initial: draft,
    onSave: controller.saveDraft,
    customImportFile: bridge.customImportFile,
    dataDir: bridge.dataDir,
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
  final bridge = ref.read(bridgePortProvider);
  final saved = await showCustomEditor(
    context,
    initial: ProfileDraft.fromDto(dto),
    onSave: controller.saveDraft,
    customImportFile: bridge.customImportFile,
    dataDir: bridge.dataDir,
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
