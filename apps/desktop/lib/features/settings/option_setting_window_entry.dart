import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Root widget of the independent settings window (second Flutter engine).
class OptionSettingsWindowApp extends StatelessWidget {
  const OptionSettingsWindowApp({super.key, required this.host});

  final SettingsEditorHost host;

  @override
  Widget build(BuildContext context) {
    // A ProviderScope is required by the ConsumerStatefulWidget, but this
    // engine never reads the Rust-backed settings controller: the editor is in
    // host mode and relays the draft to the main engine.
    return ProviderScope(
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: buildAppTheme(Brightness.light),
        home: OptionSettingWindow(host: host, standalone: true),
      ),
    );
  }
}

/// Boots the settings window engine. Called by `settingsWindowMain`
/// (`lib/main.dart`), which is the actual Flutter entrypoint symbol.
void runOptionSettingWindow() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(OptionSettingsWindowApp(host: NativeSettingsEditorHost()));
}
