import 'package:flutter/widgets.dart';

/// Which surface of the node table received the context-menu trigger.
enum ContextMenuRegion { data, rowHeader, columnHeader, empty }

/// UI-side snapshot of the node table context-menu command target.
///
/// It captures what the menu commands operate on at the moment the menu opens:
/// the target profile ids (multi-selection preserved), the primary row, the
/// group/view context, the anchor-local open position and the focus to restore
/// on close. It carries no backend or runtime state; it only pins the command
/// target so a later selection change cannot silently retarget an open menu
/// (UX-CTX-01/02).
@immutable
class ContextMenuSession {
  const ContextMenuSession({
    required this.targetIds,
    required this.position,
    this.primaryId,
    this.groupSubId,
    this.region = ContextMenuRegion.data,
    this.focusRestore,
  });

  /// Profile ids captured as the command target. True node ids, never row
  /// indices, so reordering/refresh cannot retarget a command.
  final List<String> targetIds;

  /// Anchor-local position the menu was opened at.
  final Offset position;

  /// The row the menu was invoked on, when the trigger hit a data row.
  final String? primaryId;

  /// Selected subscription group when the menu opened; a group change closes
  /// the stale command context instead of acting on another view.
  final String? groupSubId;

  final ContextMenuRegion region;

  /// Focus node to restore when the menu closes.
  final FocusNode? focusRestore;

  bool get hasTargets => targetIds.isNotEmpty;
}
