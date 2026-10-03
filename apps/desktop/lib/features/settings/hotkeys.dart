import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hotkey_manager/hotkey_manager.dart';
import 'package:v2rayn_desktop/features/settings/hotkey_keycodec.dart';
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

  /// Persisted Windows WPF `System.Windows.Input.Key` enum value (upstream
  /// `KeyEventItem.KeyCode`); `null`/0 means unbound. See [HotkeyKeyCodec] for
  /// the WPF Key <-> Win32 VK <-> Flutter conversion.
  final int? keyCode;

  static const HotkeyKeyCodec _codec = HotkeyKeyCodec();

  bool get isBound => keyCode != null && keyCode != 0;

  /// Human label, e.g. `Ctrl + Alt + K`.
  String get label => formatLabel(_codec);

  String formatLabel(HotkeyKeyCodec codec) {
    if (!isBound) return '（未设置）';
    final parts = <String>[];
    if (control) parts.add('Ctrl');
    if (alt) parts.add('Alt');
    if (shift) parts.add('Shift');
    parts.add(codec.labelForWpf(keyCode) ?? 'Key#${keyCode!}');
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

/// Called when the OS reports a registered hotkey was pressed.
typedef HotkeyTriggerHandler = void Function(GlobalHotkeyAction action);

/// Mutable holder for the live dispatcher. The shell installs the real handler
/// (window toggle / system-proxy action) at bootstrap; widget tests may leave it
/// null, in which case a trigger is dropped rather than faked.
class HotkeyDispatch {
  HotkeyTriggerHandler? handler;
}

final hotkeyDispatchProvider = Provider<HotkeyDispatch>(
  (ref) => HotkeyDispatch(),
);

/// The testable seam over the OS hotkey registration.
abstract class HotkeyRegistrar {
  /// Register the given bindings; returns the actions the OS accepted plus any
  /// conflict/failure notes. Must never fake success.
  ///
  /// [onTriggered] is invoked when the OS reports a bound hotkey, keyed by the
  /// action — this is the missing dispatch link from SET-15/RT-12.
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyBinding> bindings, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  });

  Future<void> unregisterAll();
}

/// Real registrar backed by `hotkey_manager`.
class PluginHotkeyRegistrar implements HotkeyRegistrar {
  const PluginHotkeyRegistrar();

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyBinding> bindings, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  }) async {
    final accepted = <GlobalHotkeyAction>{};
    final failures = <String>[];
    final manager = HotKeyManager.instance;
    // Reload semantics (upstream `HotkeyManager.ReLoad`).
    try {
      await manager.unregisterAll();
    } on Object catch (e) {
      failures.add('热键重载失败: $e');
    }
    for (final binding in bindings) {
      if (!binding.isBound) continue;
      // Persisted value is the WPF Key enum; resolve via WPF -> VK -> Flutter.
      final key = codec.physicalFromWpf(binding.keyCode);
      if (key == null) {
        failures.add('${binding.action.label}: 不支持的按键 #${binding.keyCode}');
        continue;
      }
      final modifiers = <HotKeyModifier>[];
      if (binding.control) modifiers.add(HotKeyModifier.control);
      if (binding.alt) modifiers.add(HotKeyModifier.alt);
      if (binding.shift) modifiers.add(HotKeyModifier.shift);
      final identifier = hotkeyIdentifier(binding.action);
      try {
        await manager.register(
          HotKey(
            identifier: identifier,
            key: key,
            modifiers: modifiers.isEmpty ? null : modifiers,
            scope: HotKeyScope.system,
          ),
          keyDownHandler: onTriggered == null
              ? null
              : (hotKey) {
                  final action = hotkeyActionFromIdentifier(hotKey.identifier);
                  if (action != null) onTriggered(action);
                },
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
}

/// Stable registration identifier, encoding the action value.
String hotkeyIdentifier(GlobalHotkeyAction action) => 'v2rayn-${action.value}';

/// Parse a registration identifier back to its action (null when unknown).
GlobalHotkeyAction? hotkeyActionFromIdentifier(String identifier) {
  final index = identifier.lastIndexOf('-');
  if (index < 0) return null;
  final value = int.tryParse(identifier.substring(index + 1));
  if (value == null) return null;
  for (final a in GlobalHotkeyAction.values) {
    if (a.value == value) return a;
  }
  return null;
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
  /// success. Uses the shell-installed dispatcher so a real key press reaches
  /// the same window/proxy entry points as the menus (RT-12).
  Future<HotkeyState> registerAll() async {
    final handler = ref.read(hotkeyDispatchProvider).handler;
    Set<GlobalHotkeyAction> accepted;
    List<String> failures;
    try {
      (accepted, failures) = await _registrar.register(
        state.bindings,
        onTriggered: handler,
      );
    } on Object catch (e) {
      // No native plugin (widget tests) or registrar crash: report honestly.
      accepted = const <GlobalHotkeyAction>{};
      failures = <String>['热键注册失败: $e'];
    }
    state = state.copyWith(
      registered: accepted,
      conflicts: failures,
      status: failures.isEmpty
          ? '热键已注册 ${accepted.length} 项'
          : '热键注册存在冲突: ${failures.length} 项',
    );
    return state;
  }

  Future<void> unregisterAll() async {
    try {
      await _registrar.unregisterAll();
    } on Object catch (_) {
      // Best-effort on teardown.
    }
  }

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
