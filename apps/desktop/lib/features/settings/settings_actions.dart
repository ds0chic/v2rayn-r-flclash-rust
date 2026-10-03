import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/global_hotkey_window.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/theme_setting_dialog.dart';

/// Open the option settings window, refreshing the document first.
Future<void> openOptionSettingWindow(
  BuildContext context,
  WidgetRef ref,
) async {
  try {
    ref.read(settingsControllerProvider.notifier).load();
  } catch (_) {}
  await OptionSettingWindow.show(context);
}

/// Open the theme setting window (immediate apply + persist).
Future<void> openThemeSettingDialog(BuildContext context, WidgetRef ref) async {
  try {
    ref.read(settingsControllerProvider.notifier).load();
  } catch (_) {}
  await ThemeSettingDialog.show(context);
}

/// Open the global hotkey window (record as WPF Key, persist, re-register).
Future<void> openGlobalHotkeyWindow(BuildContext context, WidgetRef ref) async {
  try {
    ref.read(settingsControllerProvider.notifier).load();
  } catch (_) {}
  await GlobalHotkeyWindow.show(context);
}
