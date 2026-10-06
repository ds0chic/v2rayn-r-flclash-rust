// SP-16 独立准备：编辑当前订阅直达 G / 新增直达空白对象的入口。
//
// 原版（冻结 7d6a967，ProfilesViewModel.EditSubAsync）：
// - blNew=true 直接 new SubItem()；false 取 GetSubItem(_config.SubIndexId)，
//   取不到直接 return（All/已删门控，不打开总列表）。
// - 对话框取消不写库；确定后 RefreshSubscriptions + SubSelectedChangedAsync。
//
// 本文件只用已有 subs_controller.save（单次保存）与 SubEditWindow；
// 保存成功后的跨窗 revision 重试/生效对账依赖 SP-12（A04 在途），此处登记
// 阻塞，不伪造完成。
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/sub_entry.dart';
import 'package:v2rayn_desktop/features/subs/sub_edit_window.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/shared/widgets/app_dialog.dart';

/// 编辑当前订阅：直达节点页当前组 G。All/缺失按原版门控，无操作并提示。
///
/// 返回 true=已保存，false=门控/取消/保存拒绝（均不打开总列表）。
Future<bool> openEditCurrentSub(BuildContext context, WidgetRef ref) async {
  final groupSubId = ref.read(profilesControllerProvider).groupSubId;
  final items = ref.read(subsControllerProvider).items;
  final entry = resolveSubEntry(
    groupSubId: groupSubId,
    existingIds: [for (final s in items) s.id],
  );
  if (entry.kind != SubEntryKind.editCurrent || entry.targetId == null) {
    ref
        .read(uiShellControllerProvider.notifier)
        .setMessage('当前为全部视图，无可直接编辑的订阅（已保留总列表入口）');
    return false;
  }
  final current = items.where((s) => s.id == entry.targetId).firstOrNull;
  if (current == null) return false;
  final saved = await showSubEditWindow(context, current);
  if (saved == null) {
    ref.read(uiShellControllerProvider.notifier).setMessage('已取消：未修改订阅');
    return false;
  }
  final result = ref.read(subsControllerProvider.notifier).save(saved);
  if (!result.ok) {
    ref.read(uiShellControllerProvider.notifier).setMessage('订阅保存被拒绝，已保留原数据');
    return false;
  }
  return true;
}

/// 删除当前订阅：主窗直接删 G 入口（上游 `ProfilesViewModel.DeleteSubAsync`
/// counterpart：取 GetSubItem(SubIndexId)，null 直接 return；确认框取消直接
/// return；删除后 RefreshSubscriptions + SubSelectedChangedAsync）。
///
/// All/缺失按原版门控，无操作并提示（不弹确认框）。[confirmDelete] 覆盖确认框
/// （默认弹上游 `ResUI.RemoveServer` 对等的确认框），便于测试注入取消/确认。
/// 删除失败报出错误并保持当前组（控制器 delete 仅成功才 resync）。
/// 返回 true=已删除。
Future<bool> deleteCurrentSub(
  BuildContext context,
  WidgetRef ref, {
  Future<bool> Function(String remarks)? confirmDelete,
}) async {
  final groupSubId = ref.read(profilesControllerProvider).groupSubId;
  final items = ref.read(subsControllerProvider).items;
  final entry = resolveSubEntry(
    groupSubId: groupSubId,
    existingIds: [for (final s in items) s.id],
  );
  if (entry.kind != SubEntryKind.editCurrent || entry.targetId == null) {
    ref
        .read(uiShellControllerProvider.notifier)
        .setMessage('当前为全部视图，无可直接删除的订阅（已保留总列表入口）');
    return false;
  }
  final current = items.where((s) => s.id == entry.targetId).firstOrNull;
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
  if (!confirmed) {
    ref.read(uiShellControllerProvider.notifier).setMessage('已取消：未删除订阅');
    return false;
  }
  final result = ref.read(subsControllerProvider.notifier).delete(<String>[
    current.id,
  ]);
  if (!result.ok) {
    final code = result.error?.messageKey ?? result.error?.code ?? '未知错误';
    ref
        .read(subsControllerProvider.notifier)
        .setStatus(SubStatus(kind: 'error', message: '订阅删除失败', detail: code));
    ref
        .read(uiShellControllerProvider.notifier)
        .setMessage('订阅删除失败（$code），已保留原数据');
    return false;
  }
  ref
      .read(uiShellControllerProvider.notifier)
      .setMessage('已删除订阅“${current.remarks}”');
  return true;
}

/// 新增订阅：空白对象，不复用 G 身份。取消不写库。
Future<bool> openAddSub(BuildContext context, WidgetRef ref) async {
  final draft = ref.read(subsControllerProvider.notifier).newDraft();
  final saved = await showSubEditWindow(context, draft);
  if (saved == null) {
    ref.read(uiShellControllerProvider.notifier).setMessage('已取消：未添加订阅');
    return false;
  }
  final result = ref.read(subsControllerProvider.notifier).save(saved);
  if (!result.ok) {
    ref.read(uiShellControllerProvider.notifier).setMessage('订阅保存被拒绝，已保留原数据');
    return false;
  }
  return true;
}
