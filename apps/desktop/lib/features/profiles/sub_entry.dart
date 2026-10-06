// SP-16/SP-18 独立准备：订阅入口与冻结命令目标的纯合同。
//
// 原版（冻结 7d6a967，ProfilesViewModel.cs）：
// - EditSubAsync(true) 新建空 SubItem；false 取 GetSubItem(_config.SubIndexId)，
//   null 直接 return（All/缺失门控，无操作）。
// - SubSelectedChangedAsync 写 _config.SubIndexId；RefreshSubscriptions 按
//   SubIndexId 命中恢复，否则回 All 首项（从不持久化修复值）。
// - RefreshServersBiz：pending > IndexId > 首行；单击只改内存选择。
//
// 本文件只做纯解析，不碰 BridgePort、FRB。组切换的持久化写与重开恢复由
// profiles_controller 经已有 settings/subs 桥接缝执行（SP-16 接线轮）。
// 跨窗 revision 重试对账依赖 SP-12（A04 在途），此处仅冻结“该编哪个对象”。
import 'dart:convert';

import 'package:v2rayn_desktop/features/profiles/command_context.dart';

/// 编辑当前订阅入口的解析结果。
enum SubEntryKind {
  /// 直达当前组 G 的编辑对象。
  editCurrent,

  /// 空白新建对象（不复用 G 身份）。
  createNew,

  /// All/空白/缺失组：按原版无操作门控，不回落首组。
  gatedAll,
}

class SubEntry {
  const SubEntry({required this.kind, this.targetId});

  final SubEntryKind kind;
  final String? targetId;
}

/// 解析“编辑当前订阅”应直达的对象。
///
/// [groupSubId] 为节点页当前组（null/空 = All 视图）；[existingIds] 为已存
/// 订阅 id。仅当非空且命中已存时返回 editCurrent，否则 gatedAll。
SubEntry resolveSubEntry({
  required String? groupSubId,
  required List<String> existingIds,
}) {
  final wanted = groupSubId?.trim();
  if (wanted == null || wanted.isEmpty) {
    return const SubEntry(kind: SubEntryKind.gatedAll);
  }
  if (existingIds.any((id) => id == wanted)) {
    return SubEntry(kind: SubEntryKind.editCurrent, targetId: wanted);
  }
  return const SubEntry(kind: SubEntryKind.gatedAll);
}

/// 解析“新增订阅”：恒为无身份的新对象。
SubEntry resolveCreateEntry() => const SubEntry(kind: SubEntryKind.createNew);

/// 冻结命令目标：菜单打开时捕获的 ids/primary/group。
class FrozenCommandTargets {
  const FrozenCommandTargets({
    required this.targetIds,
    required this.primaryId,
    required this.groupSubId,
  });

  final List<String> targetIds;
  final String? primaryId;
  final String? groupSubId;
}

/// 恢复冻结目标：组变更或目标不可见即失效（null），调用方须提示重选，
/// 禁止回落首行（R3-PROF-02 / UF-PROF-10）。空白组拼写归一为 All 视图，
/// 与 Rust `restore_command_targets` 一致。
FrozenCommandTargets? restoreCommandTargets({
  required List<String> frozenIds,
  required String? frozenPrimary,
  required String? frozenGroup,
  required String? currentGroup,
  required List<String> visibleIds,
}) {
  String? groupOf(String? value) {
    final trimmed = value?.trim();
    return (trimmed == null || trimmed.isEmpty) ? null : trimmed;
  }

  if (groupOf(frozenGroup) != groupOf(currentGroup)) return null;
  if (frozenIds.isEmpty) return null;
  final visible = visibleIds.toSet();
  if (frozenIds.any((id) => !visible.contains(id))) return null;
  return FrozenCommandTargets(
    targetIds: List<String>.unmodifiable(frozenIds),
    primaryId: frozenPrimary,
    groupSubId: frozenGroup,
  );
}

/// 单对象命令（编辑/分享/设为活动/导出完整配置）要求冻结 primary 仍可见。
bool isPrimaryTargetLive({
  required String? primaryId,
  required List<String> visibleIds,
}) {
  if (primaryId == null || primaryId.isEmpty) return false;
  return visibleIds.contains(primaryId);
}

/// 当前实现的子菜单 Esc 合同：整链关闭。
///
/// 上游 WPF 子菜单逐级 Esc 尚未真机对照（ui/README UI-09 注记），此处如实
/// 锁定当前行为，不猜逐级语义。
const bool menuEscClosesWholeChain = true;

/// SP-16 接线轮：`Config.SubIndexId` 独立组名（已有 settings 桥的组键）。
const String subIndexIdGroup = 'SubIndexId';

/// 重开恢复：持久化组命中现存订阅则恢复该组，否则回 All（null）。
///
/// 对应 Rust `selection::resolve_current_group` 与上游
/// `RefreshSubscriptions`（命中恢复，否则 All 首项，从不持久化修复值）。
/// 空白拼写归一为 All，与 [resolveSubEntry]/[normalizeGroupSubId] 一致。
String? resolveReopenGroup({
  required String? persisted,
  required List<String> existingIds,
}) {
  final entry = resolveSubEntry(
    groupSubId: persisted,
    existingIds: existingIds,
  );
  return entry.kind == SubEntryKind.editCurrent ? entry.targetId : null;
}

/// 从规范 `Config` JSON（PascalCase）读持久化组；缺失/空白/非法一律
/// 归一为 All（null），调用方不得回落首组。
String? readPersistedSubIndexId(String settingsJson) {
  try {
    final decoded = jsonDecode(settingsJson);
    if (decoded is! Map<String, dynamic>) return null;
    final value = decoded['SubIndexId'];
    if (value == null) return null;
    if (value is! String) return null;
    return normalizeGroupSubId(value);
  } catch (_) {
    return null;
  }
}

/// `SubIndexId` 独立组写的 patch 体：组 id 的 JSON 标量，All 写 null。
String subIndexIdPatch(String? groupSubId) =>
    jsonEncode(normalizeGroupSubId(groupSubId));

/// 解析 `group_revisions_json`；非法一律空表（调用方回落 revision 0，
/// 由桥端做 stale 仲裁，不伪造版本号）。
Map<String, int> decodeGroupRevisions(String groupRevisionsJson) {
  try {
    final decoded = jsonDecode(groupRevisionsJson);
    if (decoded is! Map) return const <String, int>{};
    final out = <String, int>{};
    decoded.forEach((key, value) {
      if (value is num) out['$key'] = value.toInt();
    });
    return out;
  } catch (_) {
    return const <String, int>{};
  }
}
