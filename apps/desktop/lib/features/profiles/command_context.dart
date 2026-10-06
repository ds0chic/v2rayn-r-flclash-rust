import 'package:flutter/widgets.dart';
import 'package:v2rayn_desktop/shared/widgets/context_menu_session.dart';

/// Immutable snapshot of what a node-table command operates on.
///
/// The context is captured when the context menu opens (before the menu closes)
/// and passed with the command, so closing the menu can never destroy the
/// business target (FIX-01 / PR-19). Commands only read this snapshot; they
/// must never fall back to the live selection/session.
///
/// Fields:
/// * [targetIds] - the exact node ids the menu was opened on, stored as an
///   unmodifiable list so no later mutation can retarget the command. True
///   node ids, never row indices.
/// * [primaryId] - the row under the pointer, when the trigger hit a data row.
/// * [groupSubId] - the selected subscription group's stable id at open time
///   (`null` = the "all groups" view). The auto-group generator uses this as
///   the object instead of a selected node's own `subid` (PR-21).
/// * [viewContext] - which surface received the trigger (data/rowHeader/
///   columnHeader/empty).
/// * [menuOpenPosition] - anchor-local position the menu opened at.
/// * [focusRestore] - table focus to restore when the menu closes.
@immutable
class CommandContext {
  CommandContext({
    required List<String> targetIds,
    required this.menuOpenPosition,
    this.primaryId,
    this.groupSubId,
    this.viewContext = ContextMenuRegion.data,
    this.focusRestore,
  }) : targetIds = List<String>.unmodifiable(targetIds);

  final List<String> targetIds;
  final String? primaryId;
  final String? groupSubId;
  final ContextMenuRegion viewContext;
  final Offset menuOpenPosition;
  final FocusNode? focusRestore;

  /// Whether the menu was opened with a non-empty node selection.
  bool get hasTargets => targetIds.isNotEmpty;

  /// Whether the context pins a concrete subscription group for generation.
  bool get hasGroup => groupSubId != null && groupSubId!.isNotEmpty;
}

/// Blank group spellings (`null`/`''`/whitespace) all name the "All" view.
String? normalizeGroupSubId(String? value) {
  final trimmed = value?.trim();
  return (trimmed == null || trimmed.isEmpty) ? null : trimmed;
}

/// Pure SP-18 gate for a captured menu command: same (normalized) group, a
/// non-empty snapshot, every target still visible. `false` means the caller
/// must refuse with a re-select prompt, never fall back to the first row.
/// Mirrors `selection.rs::restore_command_targets` (Rust) and the controller's
/// `restoreContextTargets` rebind check; the table still calls the latter for
/// the actual selection rebind.
bool isCommandContextLive({
  required CommandContext? command,
  required String? currentGroupSubId,
  required List<String> visibleIds,
}) {
  if (command == null || command.targetIds.isEmpty) return false;
  if (normalizeGroupSubId(command.groupSubId) !=
      normalizeGroupSubId(currentGroupSubId)) {
    return false;
  }
  final visible = visibleIds.toSet();
  return command.targetIds.every(visible.contains);
}
