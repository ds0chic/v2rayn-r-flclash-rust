import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';

/// Global hotkey window (LAY-HOTKEY-001).
///
/// The five `EGlobalHotkey` rows (显示窗口 / 清除系统代理 / 设置系统代理 /
/// 不改变系统代理 / PAC) are edited and persisted to `GlobalHotkeys`.
/// Registration with the OS (`HotkeyManager`) stays with T13: this window only
/// stores bindings and never reports a successful registration.
class GlobalHotkeyWindow extends ConsumerStatefulWidget {
  const GlobalHotkeyWindow({super.key});

  static Future<void> show(BuildContext context) => showDialog<void>(
    context: context,
    builder: (_) => const GlobalHotkeyWindow(),
  );

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

  List<Map<String, dynamic>> _hotkeys = <Map<String, dynamic>>[];
  final Map<int, String> _labels = <int, String>{};
  final Map<int, String> _keyLabels = <int, String>{};
  bool _draftInit = false;
  int? _recording;
  String? _status;

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
    final parts = <String>[];
    if (entry['Control'] == true) parts.add('Ctrl');
    if (entry['Alt'] == true) parts.add('Alt');
    if (entry['Shift'] == true) parts.add('Shift');
    final keyCode = (entry['KeyCode'] as num?)?.toInt();
    if (keyCode != null && keyCode != 0) {
      parts.add(_keyLabels[keyCode] ?? 'Key#$keyCode');
    }
    return parts.isEmpty ? '（未设置）' : parts.join(' + ');
  }

  KeyEventResult _onKey(int action, KeyEvent event) {
    if (_recording != action) return KeyEventResult.ignored;
    if (event is! KeyDownEvent) return KeyEventResult.handled;
    final keyboard = HardwareKeyboard.instance;
    final entry = _entry(action);
    final keyCode = event.logicalKey.keyId;
    entry['Control'] = keyboard.isControlPressed;
    entry['Alt'] = keyboard.isAltPressed;
    entry['Shift'] = keyboard.isShiftPressed;
    entry['KeyCode'] = keyCode;
    _keyLabels[keyCode] = event.logicalKey.keyLabel.isEmpty
        ? 'Key#$keyCode'
        : event.logicalKey.keyLabel;
    if (mounted) {
      setState(() {
        _labels[action] = _labelFor(entry);
        _recording = null;
      });
    }
    return KeyEventResult.handled;
  }

  void _reset() {
    setState(() {
      _hotkeys.clear();
      _labels.clear();
      _recording = null;
      _status = '已重置（保存后生效）';
    });
  }

  void _save() {
    // Ensure all five rows exist (upstream GetKeyEventItem semantics).
    for (final action in _actions) {
      _entry(action.action);
    }
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveGroup('GlobalHotkeys', _hotkeys);
    if (!mounted) return;
    setState(() {
      _status = result.ok
          ? '已保存（OS 全局注册留待 T13）'
          : (result.error?.messageKey ?? '保存失败');
    });
    if (result.ok) {
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
    return AlertDialog(
      title: const Text('全局热键设置', style: TextStyle(fontSize: 15)),
      content: SizedBox(
        width: 460,
        child: Focus(
          autofocus: true,
          onKeyEvent: (_, event) {
            final recording = _recording;
            if (recording == null) return KeyEventResult.ignored;
            return _onKey(recording, event);
          },
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              const SettingsNote('OS 全局热键注册属于 T13；此窗口仅录制与保存绑定。'),
              for (final action in _actions)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 3),
                  child: Row(
                    children: <Widget>[
                      SizedBox(
                        width: 150,
                        child: Text(
                          action.label,
                          style: const TextStyle(fontSize: 12),
                        ),
                      ),
                      Expanded(
                        child: Text(
                          _labels[action.action] ?? '（未设置）',
                          key: ValueKey<String>(
                            'hotkey-label-${action.action}',
                          ),
                          style: const TextStyle(fontSize: 12),
                        ),
                      ),
                      TextButton(
                        key: ValueKey<String>('hotkey-record-${action.action}'),
                        onPressed: () => setState(
                          () => _recording = _recording == action.action
                              ? null
                              : action.action,
                        ),
                        child: Text(
                          _recording == action.action ? '按下组合…' : '录制',
                          style: const TextStyle(fontSize: 11),
                        ),
                      ),
                    ],
                  ),
                ),
              if (_status != null)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(_status!, style: const TextStyle(fontSize: 11)),
                ),
            ],
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
