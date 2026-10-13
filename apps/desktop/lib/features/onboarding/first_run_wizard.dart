import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/onboarding/onboarding_store.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart'
    as profile_actions;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Auto-popup gate for the first-run wizard (U-B1).
///
/// Defaults to false so widget tests pumping [MainShell] directly never see
/// the dialog (condition A); production `main()` overrides it to true.
final firstRunAutoPopupProvider = Provider<bool>((_) => false);

/// Environment keys that mark an evidence/screenshot run. The wizard never
/// auto-shows on top of them so the `V2RAYN_R_OPEN_*` capture paths keep
/// working untouched.
bool suppressAutoPopupForEnv(Map<String, String> env) {
  if (env.keys.any((key) => key.startsWith('V2RAYN_R_OPEN_'))) return true;
  return env.containsKey('V2RAYN_R_AUTO_SMOKE') ||
      env.containsKey('V2RAYN_R_MONITOR_XRAY_PORT') ||
      env.containsKey('V2RAYN_R_IMPORT_SYNTHETIC') ||
      env.containsKey('V2RAYN_R_SEMANTICS_DUMP');
}

/// One-shot auto-show: empty library, first run, production only. Marks the
/// wizard completed before showing so a restart can never loop it; the help
/// menu reopens it any time.
Future<void> maybeShowFirstRunWizard(
  BuildContext context,
  WidgetRef ref,
) async {
  if (!ref.read(firstRunAutoPopupProvider)) return;
  if (suppressAutoPopupForEnv(Platform.environment)) return;
  final store = ref.read(uiStateStoreProvider);
  if (OnboardingHints.isCompleted(store)) return;
  if (ref.read(profilesControllerProvider).totalCount != 0) return;
  if (!context.mounted) return;
  OnboardingHints.setCompleted(store, true);
  await showFirstRunWizard(context, ref);
}

/// Manual entry (help menu, empty-state button). Does not touch the
/// completion flag: reopening an already-completed wizard is the point.
Future<void> showFirstRunWizard(BuildContext context, WidgetRef ref) {
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => FirstRunWizard(
      onAction: (actionId) => _dispatchMenuAction(context, ref, actionId),
    ),
  );
}

/// Clear hint-class UI state only (help menu entry UI-RESET-HINTS):
/// confirm-dialog suppressions plus the wizard completion flag. Density,
/// layout, theme and every business setting are left untouched.
void resetInterfaceHints(WidgetRef ref) {
  OnboardingHints.resetHints(ref.read(uiStateStoreProvider));
}

/// The shell installs its real menu dispatcher after the first frame (same
/// pattern as the tray delegate in [MainShell]) so the wizard buttons run the
/// identical `ACT-MAIN-*` use cases as the menu. A null delegate means the
/// wizard was opened outside the shell; the buttons then report an honest
/// "not wired" message instead of pretending to have run.
class MenuActionDelegateHolder {
  ValueChanged<String>? value;
}

final menuActionDelegateProvider = Provider<MenuActionDelegateHolder>(
  (ref) => MenuActionDelegateHolder(),
);

void _dispatchMenuAction(BuildContext context, WidgetRef ref, String actionId) {
  final delegate = ref.read(menuActionDelegateProvider).value;
  if (delegate != null) {
    delegate(actionId);
    return;
  }
  ref.read(uiShellControllerProvider.notifier).setMessage('该入口暂不可用：$actionId');
}

/// Three-step first-run wizard. Every step reuses an existing menu action —
/// the wizard owns no import/start logic of its own.
class FirstRunWizard extends ConsumerStatefulWidget {
  const FirstRunWizard({super.key, required this.onAction});

  /// Runs a shared `ACT-MAIN-*` command id (the shell passes its own menu
  /// dispatcher so the wizard and the menu run the identical use case).
  final ValueChanged<String> onAction;

  @override
  ConsumerState<FirstRunWizard> createState() => _FirstRunWizardState();
}

class _FirstRunWizardState extends ConsumerState<FirstRunWizard> {
  int _step = 0;
  bool _verifying = false;

  @override
  Widget build(BuildContext context) {
    final totalNodes = ref.watch(profilesControllerProvider).totalCount;
    final message = ref.watch(uiShellControllerProvider).message;
    return AlertDialog(
      key: const ValueKey('wizard-dialog'),
      title: const Text('首次启动向导'),
      content: SizedBox(
        width: 440,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              '第 ${_step + 1} / 3 步',
              style: TextStyle(
                fontSize: AppTokens.fontSizeSmall,
                color: Theme.of(context).colorScheme.primary,
              ),
            ),
            const SizedBox(height: 8),
            _stepBody(context, totalNodes, message),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('wizard-skip'),
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('跳过'),
        ),
        if (_step > 0)
          TextButton(
            key: const ValueKey('wizard-prev'),
            onPressed: () => setState(() => _step--),
            child: const Text('上一步'),
          ),
        if (_step < 2)
          FilledButton(
            key: const ValueKey('wizard-next'),
            onPressed: () => setState(() => _step++),
            child: const Text('下一步'),
          ),
        if (_step == 2)
          FilledButton(
            key: const ValueKey('wizard-finish'),
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('完成'),
          ),
      ],
    );
  }

  Widget _stepBody(BuildContext context, int totalNodes, String? message) {
    switch (_step) {
      case 0:
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            const Text('先把你的节点或订阅拿进来，三种方式都可以：'),
            const SizedBox(height: 12),
            FilledButton.tonal(
              key: const ValueKey('wizard-go-clipboard'),
              onPressed: () => widget.onAction('ACT-MAIN-016'),
              child: const Text('从剪贴板导入分享链接'),
            ),
            const SizedBox(height: 8),
            OutlinedButton(
              key: const ValueKey('wizard-go-scan'),
              onPressed: () => widget.onAction('ACT-MAIN-017'),
              child: const Text('扫描屏幕上的二维码'),
            ),
            const SizedBox(height: 8),
            OutlinedButton(
              key: const ValueKey('wizard-go-sub'),
              onPressed: () => widget.onAction('ACT-MAIN-019'),
              child: const Text('添加订阅（推荐，有订阅地址时）'),
            ),
          ],
        );
      case 1:
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              totalNodes == 0
                  ? '节点表还是空的。先完成第 1 步，把节点拿进来，这里会自动继续。'
                  : '已经有 $totalNodes 个节点了。在节点表里点一行，回车（Enter）把它设为活动节点。',
            ),
            const SizedBox(height: 12),
            OutlinedButton(
              key: const ValueKey('wizard-goto-table'),
              // The dialog is modal: closing it lands the user on the node
              // table behind it. Deliberately no programmatic filter focus —
              // the filter controller is owned by ProfilesPage and reaching
              // in would couple the wizard to its internals.
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('知道了，去选节点'),
            ),
          ],
        );
      default:
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            const Text('一键启动当前（或活动）节点，并马上验证本地代理是否真的通了：'),
            const SizedBox(height: 12),
            FilledButton(
              key: const ValueKey('wizard-launch-verify'),
              onPressed: totalNodes == 0 || _verifying
                  ? null
                  : _launchAndVerify,
              child: const Text('启动并验证'),
            ),
            const SizedBox(height: 8),
            Text(
              message ?? '',
              key: const ValueKey('wizard-verify-result'),
              style: TextStyle(fontSize: AppTokens.fontSizeSmall),
            ),
          ],
        );
    }
  }

  Future<void> _launchAndVerify() async {
    if (_verifying) return;
    setState(() => _verifying = true);
    try {
      // Same frozen-target command as the toolbar start button (R4-02).
      final target = profile_actions.resolveSingleTarget(
        ref.read(profilesControllerProvider),
        null,
      );
      await profile_actions.startProfileExplicit(ref, targetId: target);
      // Probe the applied endpoint; with nothing applied this reports
      // "无已应用端点" instead of fabricating success.
      await runAvailabilityTest(ref);
    } finally {
      if (mounted) setState(() => _verifying = false);
    }
  }
}
