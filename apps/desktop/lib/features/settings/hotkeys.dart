import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hotkey_manager/hotkey_manager.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// The five `EGlobalHotkey` actions (upstream `EGlobalHotkey`).
enum GlobalHotkeyAction {
  showForm(0, '显示/隐藏窗口'),
  systemProxyClear(1, '清除系统代理'),
  systemProxySet(2, '设置系统代理'),
  systemProxyUnchanged(3, '不改变系统代理'),
  systemProxyPac(4, 'PAC 模式');

  const GlobalHotkeyAction(this.value, this.label);

  final int value;
  final String label;

  static GlobalHotkeyAction fromValue(int value) =>
      GlobalHotkeyAction.values.firstWhere(
        (a) => a.value == value,
        orElse: () => GlobalHotkeyAction.showForm,
      );
}

/// One persisted `KeyEventItem` binding.
class HotkeyBinding {
  const HotkeyBinding({
    required this.action,
    this.alt = false,
    this.control = false,
    this.shift = false,
    this.keyCode,
  });

  final GlobalHotkeyAction action;
  final bool alt;
  final bool control;
  final bool shift;

  /// `KeyCode` (Windows virtual-key value upstream); `null`/0 means unbound.
  final int? keyCode;

  bool get isBound => keyCode != null && keyCode != 0;

  /// Human label, e.g. `Ctrl + Alt + K`.
  String get label {
    if (!isBound) return '（未设置）';
    final parts = <String>[];
    if (control) parts.add('Ctrl');
    if (alt) parts.add('Alt');
    if (shift) parts.add('Shift');
    parts.add('Key#${keyCode!}');
    return parts.join(' + ');
  }

  static HotkeyBinding fromSettings(Map<String, dynamic> entry) {
    return HotkeyBinding(
      action: GlobalHotkeyAction.fromValue(
        (entry['EGlobalHotkey'] as num?)?.toInt() ?? 0,
      ),
      alt: entry['Alt'] == true,
      control: entry['Control'] == true,
      shift: entry['Shift'] == true,
      keyCode: (entry['KeyCode'] as num?)?.toInt(),
    );
  }

  Map<String, dynamic> toSettings() => <String, dynamic>{
    'EGlobalHotkey': action.value,
    'Alt': alt,
    'Control': control,
    'Shift': shift,
    'KeyCode': keyCode,
  };
}

/// Parsed state of the `GlobalHotkeys` group.
class HotkeyState {
  const HotkeyState({
    this.bindings = const <HotkeyBinding>[],
    this.registered = const <GlobalHotkeyAction>{},
    this.conflicts = const <String>[],
    this.status,
  });

  final List<HotkeyBinding> bindings;

  /// Actions the OS accepted (empty until registration runs).
  final Set<GlobalHotkeyAction> registered;

  /// Human-readable conflict/failure notes.
  final List<String> conflicts;
  final String? status;

  HotkeyBinding? forAction(GlobalHotkeyAction action) {
    for (final b in bindings) {
      if (b.action == action) return b;
    }
    return null;
  }

  HotkeyState copyWith({
    List<HotkeyBinding>? bindings,
    Set<GlobalHotkeyAction>? registered,
    List<String>? conflicts,
    String? status,
  }) {
    return HotkeyState(
      bindings: bindings ?? this.bindings,
      registered: registered ?? this.registered,
      conflicts: conflicts ?? this.conflicts,
      status: status ?? this.status,
    );
  }
}

/// The testable seam over the OS hotkey registration.
abstract class HotkeyRegistrar {
  /// Register the given bindings; returns the actions the OS accepted plus any
  /// conflict/failure notes. Must never fake success.
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyBinding> bindings,
  );

  Future<void> unregisterAll();
}

/// Real registrar backed by `hotkey_manager`.
class PluginHotkeyRegistrar implements HotkeyRegistrar {
  const PluginHotkeyRegistrar();

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyBinding> bindings,
  ) async {
    final accepted = <GlobalHotkeyAction>{};
    final failures = <String>[];
    final manager = HotKeyManager.instance;
    // Reload semantics (upstream `HotkeyManager.ReLoad`).
    await manager.unregisterAll();
    for (final binding in bindings) {
      if (!binding.isBound) continue;
      final key = _physicalKeyFor(binding.keyCode!);
      if (key == null) {
        failures.add('${binding.action.label}: 不支持的按键 #${binding.keyCode}');
        continue;
      }
      final modifiers = <HotKeyModifier>[];
      if (binding.control) modifiers.add(HotKeyModifier.control);
      if (binding.alt) modifiers.add(HotKeyModifier.alt);
      if (binding.shift) modifiers.add(HotKeyModifier.shift);
      try {
        await manager.register(
          HotKey(
            identifier: 'v2rayn-${binding.action.value}',
            key: key,
            modifiers: modifiers.isEmpty ? null : modifiers,
            scope: HotKeyScope.system,
          ),
        );
        accepted.add(binding.action);
      } on Object catch (e) {
        failures.add('${binding.action.label}: 注册失败 ${e.toString()}');
      }
    }
    return (accepted, failures);
  }

  @override
  Future<void> unregisterAll() => HotKeyManager.instance.unregisterAll();

  /// Map a Windows virtual-key code to a Flutter key. Covers the common
  /// bindings; an unmapped code is reported as a conflict rather than guessed.
  /// Kept deliberately conservative so we never register the wrong key.
  static KeyboardKey? _physicalKeyFor(int virtualKey) {
    // Virtual-key codes that map cleanly to physical keys.
    const map = <int, PhysicalKeyboardKey>{
      0x41: PhysicalKeyboardKey.keyA,
      0x42: PhysicalKeyboardKey.keyB,
      0x43: PhysicalKeyboardKey.keyC,
      0x44: PhysicalKeyboardKey.keyD,
      0x45: PhysicalKeyboardKey.keyE,
      0x46: PhysicalKeyboardKey.keyF,
      0x47: PhysicalKeyboardKey.keyG,
      0x48: PhysicalKeyboardKey.keyH,
      0x49: PhysicalKeyboardKey.keyI,
      0x4A: PhysicalKeyboardKey.keyJ,
      0x4B: PhysicalKeyboardKey.keyK,
      0x4C: PhysicalKeyboardKey.keyL,
      0x4D: PhysicalKeyboardKey.keyM,
      0x4E: PhysicalKeyboardKey.keyN,
      0x4F: PhysicalKeyboardKey.keyO,
      0x50: PhysicalKeyboardKey.keyP,
      0x51: PhysicalKeyboardKey.keyQ,
      0x52: PhysicalKeyboardKey.keyR,
      0x53: PhysicalKeyboardKey.keyS,
      0x54: PhysicalKeyboardKey.keyT,
      0x55: PhysicalKeyboardKey.keyU,
      0x56: PhysicalKeyboardKey.keyV,
      0x57: PhysicalKeyboardKey.keyW,
      0x58: PhysicalKeyboardKey.keyX,
      0x59: PhysicalKeyboardKey.keyY,
      0x5A: PhysicalKeyboardKey.keyZ,
      0x30: PhysicalKeyboardKey.digit0,
      0x31: PhysicalKeyboardKey.digit1,
      0x32: PhysicalKeyboardKey.digit2,
      0x33: PhysicalKeyboardKey.digit3,
      0x34: PhysicalKeyboardKey.digit4,
      0x35: PhysicalKeyboardKey.digit5,
      0x36: PhysicalKeyboardKey.digit6,
      0x37: PhysicalKeyboardKey.digit7,
      0x38: PhysicalKeyboardKey.digit8,
      0x39: PhysicalKeyboardKey.digit9,
      0x70: PhysicalKeyboardKey.f1,
      0x71: PhysicalKeyboardKey.f2,
      0x72: PhysicalKeyboardKey.f3,
      0x73: PhysicalKeyboardKey.f4,
      0x74: PhysicalKeyboardKey.f5,
      0x75: PhysicalKeyboardKey.f6,
      0x76: PhysicalKeyboardKey.f7,
      0x77: PhysicalKeyboardKey.f8,
      0x78: PhysicalKeyboardKey.f9,
      0x79: PhysicalKeyboardKey.f10,
      0x7A: PhysicalKeyboardKey.f11,
      0x7B: PhysicalKeyboardKey.f12,
    };
    return map[virtualKey];
  }
}

final hotkeyRegistrarProvider = Provider<HotkeyRegistrar>(
  (ref) => const PluginHotkeyRegistrar(),
);

final hotkeyControllerProvider =
    NotifierProvider<HotkeyController, HotkeyState>(HotkeyController.new);

class HotkeyController extends Notifier<HotkeyState> {
  @override
  HotkeyState build() => const HotkeyState();

  HotkeyRegistrar get _registrar => ref.read(hotkeyRegistrarProvider);

  /// Read bindings from the settings controller's loaded document.
  HotkeyState loadFromSettings() {
    final document = ref.read(settingsControllerProvider).document;
    final raw = document['GlobalHotkeys'];
    final bindings = <HotkeyBinding>[];
    if (raw is List) {
      for (final entry in raw.whereType<Map<String, dynamic>>()) {
        bindings.add(HotkeyBinding.fromSettings(entry));
      }
    }
    state = HotkeyState(bindings: bindings);
    return state;
  }

  /// (Re)register all bound hotkeys. Reports conflicts instead of pretending
  /// success.
  Future<HotkeyState> registerAll() async {
    final (accepted, failures) = await _registrar.register(state.bindings);
    state = state.copyWith(
      registered: accepted,
      conflicts: failures,
      status: failures.isEmpty
          ? '热键已注册 ${accepted.length} 项'
          : '热键注册存在冲突: ${failures.length} 项',
    );
    return state;
  }

  Future<void> unregisterAll() => _registrar.unregisterAll();

  /// Save edited bindings back to the settings document and re-register.
  ///
  /// `resultOk` tells the caller whether the settings save succeeded; the hotkey
  /// window keeps the draft open on failure.
  Future<bool> save(
    List<HotkeyBinding> bindings,
    bool Function() persist,
  ) async {
    final ok = persist();
    if (!ok) {
      state = state.copyWith(status: '热键保存失败');
      return false;
    }
    state = state.copyWith(bindings: bindings);
    await registerAll();
    return true;
  }
}
