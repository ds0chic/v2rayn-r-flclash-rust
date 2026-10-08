import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';
import 'package:v2rayn_desktop/perf/semantics_dump_hook.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Root widget of the independent settings window (second Flutter engine).
class OptionSettingsWindowApp extends StatelessWidget {
  const OptionSettingsWindowApp({
    super.key,
    required this.host,
    this.brightness = Brightness.light,
    this.accentName,
    this.fontFamily,
    this.fontSize,
    this.locale,
  });

  final SettingsEditorHost host;

  /// Presentation envelope forwarded by the main window (SP-19 / UI-08): the
  /// second engine never builds a second settings backend, it only renders
  /// the shared editor body under the main-window theme/font/locale.
  /// Defaults keep the previous light rendering until the native runner
  /// forwards the envelope (tracked as the SP-19 runner follow-up).
  final Brightness brightness;
  final String? accentName;
  final String? fontFamily;
  final double? fontSize;
  final Locale? locale;

  @override
  Widget build(BuildContext context) {
    // A ProviderScope is required by the ConsumerStatefulWidget, but this
    // engine never reads the Rust-backed settings controller: the editor is in
    // host mode and relays the draft to the main engine.
    return ProviderScope(
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: buildAppTheme(
          brightness,
          accentName: accentName,
          fontFamily: fontFamily,
          fontSize: fontSize,
        ),
        locale: locale,
        home: OptionSettingWindow(host: host, standalone: true),
      ),
    );
  }
}

/// Boots the settings window engine. Called by `settingsWindowMain`
/// (`lib/main.dart`), which is the actual Flutter entrypoint symbol.
void runOptionSettingWindow() {
  WidgetsFlutterBinding.ensureInitialized();
  // Armed-only semantics dump for this secondary engine (UIA gap workaround):
  // writes next to the main window snapshot with a `.settings` suffix.
  if (SemanticsDumpHook.enabled) {
    SemanticsDumpHook.suffix = '.settings';
    SemanticsDumpHook.start();
  }
  runApp(OptionSettingsWindowApp(host: NativeSettingsEditorHost()));
}
