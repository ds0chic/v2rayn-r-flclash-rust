import 'package:flutter/services.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

/// Canonical action identifiers shared by the keyboard mapper, the table
/// widget and the tests. Names match the upstream `ACT-PROF-*` shortcuts.
class ProfileAction {
  static const selectAll = 'select-all';
  static const copy = 'copy';
  static const edit = 'edit';
  static const share = 'share';
  static const tcping = 'tcping';
  static const realping = 'realping';
  static const speedtest = 'speedtest';
  static const mixedTest = 'mixed-test';
  static const fastRealping = 'fast-realping';
  static const udpTest = 'udp-test';
  static const removeInvalid = 'remove-invalid';
  static const removeDuplicate = 'remove-duplicate';
  static const stopTest = 'stop-test';
  static const activate = 'activate';
  static const delete = 'delete';
  static const moveTop = 'move-top';
  static const moveUp = 'move-up';
  static const moveDown = 'move-down';
  static const moveBottom = 'move-bottom';
  static const navigateUp = 'navigate-up';
  static const navigateDown = 'navigate-down';
  static const escape = 'escape';
  static const dragStart = 'drag-start';
  static const drop = 'drop';
  static const contextMenu = 'context-menu';
}

/// Map a key press to a table action, or null. Modifier state comes from
/// [HardwareKeyboard], not from the event, so the mapping is deterministic.
String? actionForKey({
  required LogicalKeyboardKey key,
  required bool ctrl,
  required bool shift,
  required bool alt,
}) {
  if (alt) return null;
  if (ctrl) {
    final mapping = <LogicalKeyboardKey, String>{
      LogicalKeyboardKey.keyA: ProfileAction.selectAll,
      LogicalKeyboardKey.keyC: ProfileAction.copy,
      LogicalKeyboardKey.keyD: ProfileAction.edit,
      LogicalKeyboardKey.keyF: ProfileAction.share,
      LogicalKeyboardKey.keyO: ProfileAction.tcping,
      LogicalKeyboardKey.keyR: ProfileAction.realping,
      LogicalKeyboardKey.keyT: ProfileAction.speedtest,
      LogicalKeyboardKey.keyE: ProfileAction.mixedTest,
    };
    return mapping[key];
  }
  if (shift) return null;
  final mapping = <LogicalKeyboardKey, String>{
    LogicalKeyboardKey.enter: ProfileAction.activate,
    LogicalKeyboardKey.numpadEnter: ProfileAction.activate,
    LogicalKeyboardKey.delete: ProfileAction.delete,
    LogicalKeyboardKey.backspace: ProfileAction.delete,
    LogicalKeyboardKey.keyT: ProfileAction.moveTop,
    LogicalKeyboardKey.keyU: ProfileAction.moveUp,
    LogicalKeyboardKey.keyD: ProfileAction.moveDown,
    LogicalKeyboardKey.keyB: ProfileAction.moveBottom,
    LogicalKeyboardKey.arrowUp: ProfileAction.navigateUp,
    LogicalKeyboardKey.arrowDown: ProfileAction.navigateDown,
    LogicalKeyboardKey.escape: ProfileAction.escape,
  };
  return mapping[key];
}

/// Single-item selection.
Set<String> selectSingle(String id) => <String>{id};

/// Ctrl-click toggle. The clicked row becomes the new range anchor.
Set<String> toggleSelection(Set<String> current, String id) {
  final next = Set<String>.of(current);
  if (!next.add(id)) {
    next.remove(id);
  }
  return next;
}

/// Shift range selection from the anchor to the clicked id (inclusive).
Set<String> extendSelection(
  List<ProfileSummary> rows,
  Set<String> current,
  String id,
) {
  if (current.isEmpty) return selectSingle(id);
  final ids = rows.map((r) => r.id).toList();
  final anchorId = current.last;
  final anchorIndex = ids.indexOf(anchorId);
  final targetIndex = ids.indexOf(id);
  if (anchorIndex < 0 || targetIndex < 0) return selectSingle(id);
  final start = anchorIndex < targetIndex ? anchorIndex : targetIndex;
  final end = anchorIndex < targetIndex ? targetIndex : anchorIndex;
  return ids.sublist(start, end + 1).toSet();
}
