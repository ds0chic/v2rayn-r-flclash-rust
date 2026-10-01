import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Theme setting window (LAY-THEME-001).
///
/// Theme, accent and font size apply immediately (`immediate` timing);
/// font family, language and main layout direction are applied live
/// in-session ahead of restart but their authoritative `apply_timing` is
/// `RestartApp` (see `settings_timing.rs`), so a restart converges them.
/// All persist to `UIItem`; reopening the app restores them.
class ThemeSettingDialog extends ConsumerStatefulWidget {
  const ThemeSettingDialog({super.key});

  static Future<void> show(BuildContext context) => showDialog<void>(
    context: context,
    builder: (_) => const ThemeSettingDialog(),
  );

  @override
  ConsumerState<ThemeSettingDialog> createState() => _ThemeSettingDialogState();
}

class _ThemeSettingDialogState extends ConsumerState<ThemeSettingDialog> {
  Map<String, dynamic> _ui = <String, dynamic>{};

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(() {
      if (!mounted) return;
      ref.read(settingsControllerProvider.notifier).load();
      if (mounted) setState(() {});
    });
  }

  /// Last successfully persisted `UiItem` draft. A failed save rolls the
  /// draft entry back to this snapshot and keeps the old theme applied.
  Map<String, dynamic> _lastSaved = <String, dynamic>{};

  void _ensureDraft() {
    final state = ref.read(settingsControllerProvider);
    if (!state.loaded) return;
    _ui =
        ref.read(settingsControllerProvider.notifier).draft()['UiItem']
            as Map<String, dynamic>? ??
        <String, dynamic>{};
    _lastSaved = Map<String, dynamic>.of(_ui);
  }

  void _apply(String key) {
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveGroup('UiItem', _ui);
    if (result.ok) {
      _lastSaved = Map<String, dynamic>.of(_ui);
      _applyTheme(_ui);
      setState(() {
        _status = '已应用';
      });
    } else {
      // Save first, apply second: on failure the old theme stays applied
      // and the draft entry is reverted so the UI cannot diverge from it.
      _ui[key] = _lastSaved[key];
      _applyTheme(_lastSaved);
      setState(() {
        _status = result.error?.messageKey ?? '保存失败';
      });
    }
  }

  void _applyTheme(Map<String, dynamic> ui) {
    ref
        .read(uiShellControllerProvider.notifier)
        .applyThemeSelection(
          theme: ui['CurrentTheme'] as String?,
          accent: (ui['ColorPrimaryName'] as String?) ?? '',
          fontFamily: (ui['CurrentFontFamily'] as String?) ?? '',
          fontSize: (ui['CurrentFontSize'] as num?)?.toInt(),
          language: ui['CurrentLanguage'] as String?,
        );
  }

  String? _status;

  void _set(String key, Object? value) {
    setState(() => _ui[key] = value);
    _apply(key);
  }

  bool _draftInit = false;

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(settingsControllerProvider);
    if (!_draftInit && state.loaded) {
      _ensureDraft();
      _draftInit = true;
    }
    final theme = (_ui['CurrentTheme'] as String?) ?? 'FollowSystem';
    final accent = (_ui['ColorPrimaryName'] as String?) ?? '';
    final language = (_ui['CurrentLanguage'] as String?) ?? 'zh-Hans';
    final size = (_ui['CurrentFontSize'] as num?)?.toInt() ?? 0;
    return AlertDialog(
      title: const Text('主题设置', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            SettingsDropdown<String>(
              label: '主题',
              value: theme,
              width: 200,
              items: _dropdown(const <String>[
                'FollowSystem',
                'Light',
                'Dark',
                'Aquatic',
                'Desert',
                'Dusk',
                'NightSky',
              ]),
              onChanged: (v) => _set('CurrentTheme', v),
            ),
            SettingsDropdown<String>(
              label: '强调色',
              value: accent,
              width: 200,
              items: _dropdown(<String>['', ...accentColors.keys]),
              onChanged: (v) => _set('ColorPrimaryName', v == '' ? null : v),
            ),
            SettingsTextField(
              label: '字体族',
              value: _ui['CurrentFontFamily'] as String?,
              onChanged: (v) => _set('CurrentFontFamily', v),
            ),
            SettingsNumberField(
              label: '字号',
              value: size,
              onChanged: (v) => _set('CurrentFontSize', v),
            ),
            SettingsDropdown<String>(
              label: '语言 (需重启应用)',
              value: language,
              width: 200,
              items: _dropdown(const <String>[
                'zh-Hans',
                'zh-Hant',
                'en',
                'fa',
                'fr',
                'ru',
                'hu',
                'id',
                'az',
              ]),
              onChanged: (v) => _set('CurrentLanguage', v),
            ),
            if (_status != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(_status!, style: const TextStyle(fontSize: 11)),
              ),
          ],
        ),
      ),
      actions: <Widget>[
        FilledButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('关闭'),
        ),
      ],
    );
  }

  List<DropdownMenuItem<String>> _dropdown(List<String> values) => values
      .map(
        (v) => DropdownMenuItem<String>(
          value: v,
          child: Text(
            v.isEmpty ? '（默认）' : v,
            style: const TextStyle(fontSize: 12),
          ),
        ),
      )
      .toList();
}
