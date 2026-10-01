import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

class V2rayNRApp extends ConsumerWidget {
  const V2rayNRApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final shell = ref.watch(uiShellControllerProvider);
    return MaterialApp(
      title: 'v2rayN-R (T05)',
      debugShowCheckedModeBanner: false,
      theme: buildAppTheme(
        Brightness.light,
        accentName: shell.accentName,
        fontFamily: shell.fontFamily,
        fontSize: shell.fontSize,
      ),
      darkTheme: buildAppTheme(
        Brightness.dark,
        accentName: shell.accentName,
        fontFamily: shell.fontFamily,
        fontSize: shell.fontSize,
      ),
      themeMode: shell.themeMode,
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
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      final controller = ref.read(runtimeControllerProvider.notifier);
      await controller.start();
      // Evidence-run hook: launch straight into the smoke session so the T03
      // screenshot can show a real Running state without a scripted click.
      // Debug-only: a release build must never auto-start a core from an
      // environment variable (ISSUE-08).
      if (!kDebugMode) return;
      final autostart = Platform.environment['V2RAYN_R_AUTOSTART'];
      stderr.writeln('[t03] bootstrap autostart=$autostart');
      if (autostart == '1' || autostart == 'true') {
        await controller.applySmoke();
        final view = ref.read(runtimeControllerProvider);
        stderr.writeln(
          '[t03] applySmoke state=${view.state} pid=${view.pid} '
          'ports=${view.ports} error=${view.error}',
        );
      }
    });
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
