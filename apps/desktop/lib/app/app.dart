import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/locale_config.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// When overridden to `true`, the bootstrap applies the persisted runtime plan
/// once after settings load. Defaults to `false`, so normal runs never
/// auto-start a core from the environment.
final applyPlanOnLaunchProvider = Provider<bool>((_) => false);

class V2rayNRApp extends ConsumerWidget {
  const V2rayNRApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shell = ref.watch(uiShellControllerProvider);
    return MaterialApp(
      title: AppWindowMetrics.title,
      debugShowCheckedModeBanner: false,
      theme: buildAppTheme(
        Brightness.light,
        accentName: shell.accentName,
        fontFamily: shell.fontFamily,
        fontSize: shell.fontSize,
        zebraEnabled: shell.zebraStriping,
      ),
      darkTheme: buildAppTheme(
        Brightness.dark,
        accentName: shell.accentName,
        fontFamily: shell.fontFamily,
        fontSize: shell.fontSize,
        zebraEnabled: shell.zebraStriping,
      ),
      themeMode: shell.themeMode,
      locale: localeForLanguage(shell.language),
      supportedLocales: kSupportedLanguages,
      home: const _RuntimeBootstrap(child: MainShell()),
    );
  }
}

/// Starts the runtime subscription/snapshot exactly once in the real app.
///
/// It is deliberately absent from widget tests, which build [MainShell]
/// directly and must never touch the native library.
class _RuntimeBootstrap extends ConsumerStatefulWidget {
  const _RuntimeBootstrap({required this.child});

  final Widget child;

  @override
  ConsumerState<_RuntimeBootstrap> createState() => _RuntimeBootstrapState();
}

class _RuntimeBootstrapState extends ConsumerState<_RuntimeBootstrap> {
  DesktopIntegration? _integration;

  @override
  void dispose() {
    windowShutdown();
    super.dispose();
  }

  void windowShutdown() {
    _integration?.removeListener();
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      final controller = ref.read(runtimeControllerProvider.notifier);
      await controller.start();
      // Load settings first so proxy/close/hotkey wiring reads real values.
      ref.read(settingsControllerProvider.notifier).load();
      // T17 evidence hooks: force layout / theme *after* the persisted document
      // is applied so the release screenshot matrix covers three layouts x
      // light/dark deterministically. No-ops in normal runs.
      _applyEvidenceLayoutAndTheme(ref);
      // T13 desktop integration (tray / hotkeys / close-to-tray / exit restore).
      // Widget tests never construct V2rayNRApp, so no plugin call runs there.
      _integration = DesktopIntegration(ref);
      await _integration!.start();
      ref.read(desktopIntegrationProvider).value = _integration;
      // Normal-startup restore (upstream `Init` -> `Reload`): if a node is
      // persisted as active, apply it once on launch. This is the production
      // path and does not depend on any test environment variable.
      await controller.restoreActiveOnLaunch();
      // Evidence-run hooks that force an apply even without a persisted active
      // node:
      //  * kDebugMode + V2RAYN_R_AUTOSTART (historical T03/T18b screenshots);
      //  * the applyPlanOnLaunch override, set by main() only in builds armed
      //    with --dart-define=V2RAYN_R_SMOKE_ARMED=true and the
      //    V2RAYN_R_AUTO_SMOKE environment variable (T20 packaged smoke).
      //    Default release builds ignore every state-changing automation env
      //    variable (ISSUE-08).
      var shouldApply = ref.read(applyPlanOnLaunchProvider);
      if (kDebugMode) {
        final autostart = Platform.environment['V2RAYN_R_AUTOSTART'];
        stderr.writeln('[t03] bootstrap autostart=$autostart');
        if (autostart == '1' || autostart == 'true') shouldApply = true;
      }
      if (shouldApply && !ref.read(runtimeControllerProvider).isRunning) {
        await controller.applyActive();
        final view = ref.read(runtimeControllerProvider);
        stderr.writeln(
          '[t18b] applyActive state=${view.state} pid=${view.pid} '
          'ports=${view.ports} error=${view.error}',
        );
      }
    });
  }

  /// Evidence-only: `V2RAYN_R_LAYOUT` / `V2RAYN_R_THEME` override the persisted
  /// UI state after it has loaded. `V2RAYN_R_THEME` is applied by writing the
  /// mode without toggling twice; layout is set through the controller so the
  /// split values remain consistent.
  static void _applyEvidenceLayoutAndTheme(WidgetRef ref) {
    final layoutEnv = Platform.environment['V2RAYN_R_LAYOUT'];
    if (layoutEnv != null) {
      ref
          .read(uiShellControllerProvider.notifier)
          .setLayout(AppLayoutMode.fromId(layoutEnv));
    }
    final themeEnv = Platform.environment['V2RAYN_R_THEME'];
    if (themeEnv != null) {
      ref
          .read(uiShellControllerProvider.notifier)
          .setThemeMode(
            themeEnv.toLowerCase() == 'dark' ? ThemeMode.dark : ThemeMode.light,
          );
    }
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
