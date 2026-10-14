import 'package:flutter/material.dart';

/// Timed undo entry (UX1-03, reused by UX1-05): a snackbar with an undo
/// action visible for [duration]. The caller owns the undo semantics via
/// [onUndo]; the component only owns the countdown presentation.
ScaffoldFeatureController<SnackBar, SnackBarClosedReason> showUndoSnackbar(
  BuildContext context, {
  required String message,
  required VoidCallback onUndo,
  Duration duration = const Duration(seconds: 10),
  Key? actionKey,
}) {
  return ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(
      content: Text(message),
      duration: duration,
      action: SnackBarAction(
        key: actionKey ?? const ValueKey('undo-snackbar-action'),
        label: '撤销',
        onPressed: onUndo,
      ),
    ),
  );
}
