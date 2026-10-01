import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

/// Theme setting window (LAY-THEME-001).
///
/// Theme, accent, font family, font size and language apply immediately to the
/// live UI (`immediate` timing) and persist to `UIItem`; reopening the app
/// restores them.
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

  void _ensureDraft() {
    final state = ref.read(settingsControllerProvider);
    if (!state.loaded) return;
    _ui =
        ref.read(settingsControllerProvider.notifier).draft()['UiItem']
            as Map<String, dynamic>? ??
        <String, dynamic>{};
  }

  void _apply() {
    final theme = _ui['CurrentTheme'] as String?;
    final accent = _ui['ColorPrimaryName'] as String?;
    final family = _ui['CurrentFontFamily'] as String?;
    final size = (_ui['CurrentFontSize'] as num?)?.toInt();
    ref
        .read(uiShellControllerProvider.notifier)
        .applyThemeSelection(
          theme: theme,
          accent: accent ?? '',
          fontFamily: family ?? '',
          fontSize: size,
          language: _ui['CurrentLanguage'] as String?,
        );
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveGroup('UiItem', _ui);
    setState(() {
      _status = result.ok ? '已应用' : (result.error?.messageKey ?? '保存失败');
    });
  }

  String? _status;

  void _set(String key, Object? value) {
    setState(() => _ui[key] = value);
    _apply();
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
