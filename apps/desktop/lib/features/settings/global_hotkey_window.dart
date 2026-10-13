import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n_context.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/settings/hotkeys.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';

/// Global hotkey window (LAY-HOTKEY-001).
///
/// The five `EGlobalHotkey` rows (显示窗口 / 清除系统代理 / 设置系统代理 /
/// 不改变系统代理 / PAC) are recorded as WPF `Key` values, persisted to
/// `GlobalHotkeys`, and re-registered through the shared `HotkeyController`
/// (SET-15): saving reloads the OS binding and reports real conflicts.
class GlobalHotkeyWindow extends ConsumerStatefulWidget {
  const GlobalHotkeyWindow({super.key, this.probe = const Win32HotkeyProbe()});

  /// UB2-04: display-only occupancy pre-check shown right after recording.
  /// Injected in tests so no real `RegisterHotKey` runs there; the verdict
  /// never blocks saving (the real registration happens at save time).
  final HotkeyProbe probe;

  /// R4-30 / UFS-17: the dialog builds its strings once, so a language change
  /// while it is open closes it; the user reopens it in the new language. The
  /// listener is optional; a scope-less harness simply keeps the dialog open.
  static Future<void> show(BuildContext context) {
    final navigator = Navigator.of(context, rootNavigator: true);
    final route = showDialog<void>(
      context: context,
      builder: (_) => const GlobalHotkeyWindow(),
    );
    try {
      final container = ProviderScope.containerOf(context, listen: false);
      var last = container.read(uiShellControllerProvider).language;
      final sub = container.listen<UiShellState>(uiShellControllerProvider, (
        previous,
        next,
      ) {
        if (next.language != last) {
          last = next.language;
          navigator.maybePop();
        }
      });
      route.whenComplete(sub.close);
    } catch (_) {}
    return route;
  }

  @override
  ConsumerState<GlobalHotkeyWindow> createState() => _GlobalHotkeyWindowState();
}

class _GlobalHotkeyWindowState extends ConsumerState<GlobalHotkeyWindow> {
  static const List<({int action, String label})> _actions =
      <({int action, String label})>[
        (action: 0, label: '显示/隐藏窗口'),
        (action: 1, label: '清除系统代理'),
        (action: 2, label: '设置系统代理'),
        (action: 3, label: '不改变系统代理'),
        (action: 4, label: 'PAC 模式'),
      ];

  static const HotkeyKeyCodec _codec = HotkeyKeyCodec();

  List<Map<String, dynamic>> _hotkeys = <Map<String, dynamic>>[];
  final Map<int, String> _labels = <int, String>{};

  /// UB2-04: per-row occupancy notes from the pre-check (display only).
  final Map<int, String> _conflicts = <int, String>{};

  /// UB2-04: novice captions, one per action row.
  static const Map<int, String> _helps = <int, String>{
    0: '全局显隐主窗口。建议带 Ctrl/Alt 组合防误触。',
    1: '一键关闭系统代理（软件不断开）。误触会切直连。',
    2: '一键开启系统代理。误触会让全系统应用走代理。',
    3: '保持系统代理现状不变。',
    4: '切到 PAC 自动模式。',
  };
  bool _draftInit = false;
  int? _recording;
  String? _status;
  bool _saved = false;
  late final HotkeyController _hotkeyController;

  @override
  void initState() {
    super.initState();
    // Pause the native registration while the editor is open (upstream
    // `HotkeyManager.IsPause = true`): a recorded combo must reach this editor
    // instead of firing its saved action. `dispose` restores it on cancel.
    _hotkeyController = ref.read(hotkeyControllerProvider.notifier);
    unawaited(_hotkeyController.beginEdit());
    Future<void>.microtask(() {
      if (!mounted) return;
      ref.read(settingsControllerProvider.notifier).load();
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    if (!_saved) {
      // Cancel / Esc / barrier dismissal: restore the pre-edit registration.
      unawaited(_hotkeyController.cancelEdit());
    }
    super.dispose();
  }

  void _ensureDraft() {
    _hotkeys = _readHotkeys();
    _labels.clear();
    for (final action in _actions) {
      _labels[action.action] = _labelFor(_entry(action.action));
    }
  }

  List<Map<String, dynamic>> _readHotkeys() {
    final value = ref
        .read(settingsControllerProvider.notifier)
        .draft()['GlobalHotkeys'];
    if (value is List) {
      return value.whereType<Map<String, dynamic>>().toList();
    }
    return <Map<String, dynamic>>[];
  }

  Map<String, dynamic> _entry(int action) {
    for (final item in _hotkeys) {
      if ((item['EGlobalHotkey'] as num?)?.toInt() == action) return item;
    }
    final created = <String, dynamic>{
      'EGlobalHotkey': action,
      'Alt': false,
      'Control': false,
      'Shift': false,
      'KeyCode': null,
    };
    _hotkeys.add(created);
    return created;
  }

  String _labelFor(Map<String, dynamic> entry) {
    return HotkeyBinding.fromSettings(entry).formatLabel(_codec);
  }

  KeyEventResult _onKey(int action, KeyEvent event) {
    if (_recording != action) return KeyEventResult.ignored;
    if (event is! KeyDownEvent) return KeyEventResult.handled;
    final keyboard = HardwareKeyboard.instance;
    final entry = _entry(action);
    // Modifier state is captured on every keydown but a bare modifier must not
    // terminate recording or be stored as the key (upstream
    // `TxtGlobalHotkey_PreviewKeyDown` sets `Key.None` for modifiers and keeps
    // the box focused). SET-15 regression: previously the first modifier ended
    // recording with the modifier's own key id.
    entry['Control'] = keyboard.isControlPressed;
    entry['Alt'] = keyboard.isAltPressed;
    entry['Shift'] = keyboard.isShiftPressed;
    if (HotkeyKeyCodec.isModifierKey(event.logicalKey)) {
      if (mounted) setState(() => _labels[action] = _labelFor(entry));
      return KeyEventResult.handled;
    }
    final wpfKey = _codec.wpfKeyForEvent(event);
    if (wpfKey == HotkeyKeyCodec.wpfNone) {
      if (mounted) {
        setState(() {
          _labels[action] = '不支持的按键';
          _recording = null;
        });
      }
      return KeyEventResult.handled;
    }
    entry['KeyCode'] = wpfKey;
    // UB2-04: display-only occupancy pre-check (never blocks saving; the
    // real verdict comes from registerAll at save time). The probe holds no
    // registration: Win32HotkeyProbe unregisters on its success path and
    // holds nothing when registration fails.
    final occupancy = widget.probe.probe(
      HotkeyCombo(
        keyCode: wpfKey,
        alt: entry['Alt'] == true,
        control: entry['Control'] == true,
        shift: entry['Shift'] == true,
      ),
      _codec,
    );
    if (mounted) {
      setState(() {
        _labels[action] = _labelFor(entry);
        _recording = null;
        if (occupancy.free) {
          _conflicts.remove(action);
        } else {
          _conflicts[action] = '该组合已被其它程序占用，保存后可能无法生效，建议换一个。';
        }
      });
    }
    return KeyEventResult.handled;
  }

  void _reset() {
    setState(() {
      _hotkeys.clear();
      _labels.clear();
      _conflicts.clear();
      _recording = null;
      _status = '已重置（保存后生效）';
    });
  }

  Future<void> _save() async {
    // Ensure all five rows exist (upstream GetKeyEventItem semantics).
    for (final action in _actions) {
      _entry(action.action);
    }
    final bindings = <HotkeyBinding>[
      for (final entry in _hotkeys) HotkeyBinding.fromSettings(entry),
    ];
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveGroup('GlobalHotkeys', _hotkeys);
    // Save then re-register through the shared controller so the OS binding is
    // reloaded and dispatched (SET-15 / RT-12). A failed persist or a native
    // registration conflict keeps the window open with the draft intact.
    final ok = await _hotkeyController.save(bindings, () => result.ok);
    if (!mounted) return;
    final state = ref.read(hotkeyControllerProvider);
    setState(() {
      _status = !result.ok
          ? (result.error?.messageKey ?? '保存失败')
          : (state.conflicts.isEmpty
                ? '已保存并重新注册 ${state.registered.length} 项'
                : '已保存；以下组合注册冲突：${state.conflicts.join('；')}');
    });
    if (ok) {
      _saved = true;
      Navigator.of(context).pop();
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(settingsControllerProvider);
    if (!_draftInit && state.loaded) {
      _ensureDraft();
      _draftInit = true;
    }
    // D2-06: a failed load is an explicit error + retry entry, never five
    // misleading "unbound" rows (mirrors the option window recovery entry).
    if (state.loadFailed) {
      return AlertDialog(
        title: const Text(
          '全局热键设置',
          style: TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
        ),
        content: Text(
          context.messageText(state.status ?? 'error.settings_load_failed'),
          key: const ValueKey('hotkey-load-error'),
          style: TextStyle(color: Theme.of(context).colorScheme.error),
        ),
        actions: <Widget>[
          TextButton(
            key: const ValueKey('hotkey-load-retry'),
            onPressed: () {
              ref.read(settingsControllerProvider.notifier).load();
              if (mounted) setState(() {});
            },
            child: Text(context.tr('retry')),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('取消'),
          ),
        ],
      );
    }
    return AlertDialog(
      title: const Text(
        '全局热键设置',
        style: TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
      ),
      content: SizedBox(
        width: 460,
        child: Focus(
          autofocus: true,
          onKeyEvent: (_, event) {
            final recording = _recording;
            if (recording == null) return KeyEventResult.ignored;
            return _onKey(recording, event);
          },
          // UB2-04: the per-row captions grow the dialog; scroll instead of
          // overflowing on small viewports.
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                const SettingsNote('录制 WPF Key 编码；保存后立即重注册并经共享入口分发。'),
                for (final action in _actions)
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: 3),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Row(
                          children: <Widget>[
                            SizedBox(
                              width: 150,
                              child: Text(
                                action.label,
                                style: const TextStyle(
                                  fontSize: AppTokens.fontSize,
                                ),
                              ),
                            ),
                            Expanded(
                              child: Text(
                                _labels[action.action] ?? '（未设置）',
                                key: ValueKey<String>(
                                  'hotkey-label-${action.action}',
                                ),
                                style: const TextStyle(
                                  fontSize: AppTokens.fontSize,
                                ),
                              ),
                            ),
                            TextButton(
                              key: ValueKey<String>(
                                'hotkey-record-${action.action}',
                              ),
                              onPressed: () => setState(
                                () => _recording = _recording == action.action
                                    ? null
                                    : action.action,
                              ),
                              child: Text(
                                _recording == action.action ? '按下组合…' : '录制',
                                style: const TextStyle(
                                  fontSize: AppTokens.fontSizeCaption,
                                ),
                              ),
                            ),
                          ],
                        ),
                        Padding(
                          padding: const EdgeInsets.only(left: 150, right: 4),
                          child: Text(
                            _helps[action.action] ?? '',
                            style: AppForm.helperStyle(Theme.of(context))
                                .copyWith(color: context.semantics.muted),
                          ),
                        ),
                        if (_conflicts[action.action] != null)
                          Padding(
                            padding: const EdgeInsets.only(
                              left: 150,
                              right: 4,
                              top: 1,
                            ),
                            child: Text(
                              _conflicts[action.action]!,
                              style: AppForm.helperStyle(Theme.of(context))
                                  .copyWith(color: context.semantics.warning),
                            ),
                          ),
                      ],
                    ),
                  ),
                if (_status != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Text(
                      _status!,
                      style: const TextStyle(
                        fontSize: AppTokens.fontSizeCaption,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(onPressed: _reset, child: const Text('重置')),
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(onPressed: _save, child: const Text('保存')),
      ],
    );
  }
}
