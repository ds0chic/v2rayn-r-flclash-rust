import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Shared confirmation dialog with the upstream action order (取消 on the
/// left, 确定/删除 on the right) and keyboard contract:
///   - Enter confirms  (the primary button autofocuses, so ActivateIntent runs)
///   - Esc   cancels   (WidgetsApp default DismissIntent pops the route)
Future<bool> showAppConfirmDialog(
  BuildContext context, {
  required String title,
  required String message,
  String confirmLabel = '确定',
  String cancelLabel = '取消',
  bool destructive = false,
  Key? dialogKey,
  Key? confirmKey,
  Key? cancelKey,
}) async {
  final result = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      key: dialogKey,
      title: Text(title),
      content: Text(message),
      actions: <Widget>[
        TextButton(
          key: cancelKey,
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(cancelLabel),
        ),
        FilledButton(
          key: confirmKey,
          autofocus: true,
          style: destructive
              ? FilledButton.styleFrom(
                  backgroundColor: Theme.of(context).colorScheme.error,
                  foregroundColor: Theme.of(context).colorScheme.onError,
                )
              : null,
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(confirmLabel),
        ),
      ],
    ),
  );
  return result ?? false;
}

/// Right-aligned dialog action row preserving the upstream 取消/确定 order.
class AppDialogActions extends StatelessWidget {
  const AppDialogActions({super.key, required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: AppTokens.splitterThickness),
      child: Row(mainAxisAlignment: MainAxisAlignment.end, children: children),
    );
  }
}
