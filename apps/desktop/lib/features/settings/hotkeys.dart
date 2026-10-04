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

/// A unique OS hotkey combination: modifiers plus a persisted WPF `Key`.
///
/// This is the upstream dictionary key (`key = (vKey << 16) | modifiers` in
/// `HotkeyManager.Init`), used so a combination shared by several actions is
/// registered exactly once.
class HotkeyCombo {
  const HotkeyCombo({
    required this.keyCode,
    this.alt = false,
    this.control = false,
    this.shift = false,
  });

  final int keyCode;
  final bool alt;
  final bool control;
  final bool shift;

  /// Stable registration identifier; independent of any action.
  String get id =>
      'v2rayn-$keyCode-${(control ? 1 : 0)}${(alt ? 2 : 0)}${(shift ? 4 : 0)}';

  @override
  bool operator ==(Object other) =>
      other is HotkeyCombo &&
      other.keyCode == keyCode &&
      other.alt == alt &&
      other.control == control &&
      other.shift == shift;

  @override
  int get hashCode => Object.hash(keyCode, alt, control, shift);
}

/// One OS registration: a combo and the ordered actions it dispatches
/// (upstream `Dictionary<int, List<EGlobalHotkey>>` value).
class HotkeyRegistration {
  const HotkeyRegistration({required this.combo, required this.actions});

  final HotkeyCombo combo;
  final List<GlobalHotkeyAction> actions;
}

/// Group bound bindings by combo, preserving first-seen order and deduplicating
/// a repeated action within one combo (upstream `HotkeyManager.Init`).
List<HotkeyRegistration> groupHotkeyBindings(List<HotkeyBinding> bindings) {
  final order = <HotkeyCombo>[];
  final actionsByCombo = <HotkeyCombo, List<GlobalHotkeyAction>>{};
  for (final binding in bindings) {
    if (!binding.isBound) continue;
    final combo = HotkeyCombo(
      keyCode: binding.keyCode!,
      alt: binding.alt,
      control: binding.control,
      shift: binding.shift,
    );
    final actions = actionsByCombo.putIfAbsent(combo, () {
      order.add(combo);
      return <GlobalHotkeyAction>[];
    });
    if (!actions.contains(binding.action)) {
      actions.add(binding.action);
    }
  }
  return <HotkeyRegistration>[
    for (final combo in order)
      HotkeyRegistration(combo: combo, actions: actionsByCombo[combo]!),
  ];
}

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
  /// Register the given combinations; returns the actions the OS accepted plus
  /// any conflict/failure notes. Must never fake success.
  ///
  /// [onTriggered] is invoked for every action bound to a fired combination
  /// — this is the missing dispatch link from SET-15/RT-12.
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyRegistration> registrations, {
    HotkeyTriggerHandler? onTriggered,
    HotkeyKeyCodec codec = const HotkeyKeyCodec(),
  });

  Future<void> unregisterAll();
}

/// Real registrar backed by `hotkey_manager`.
///
/// Combinations are pre-grouped by [HotkeyCombo], so a combination shared by
/// several actions is registered exactly once and its handler dispatches every
/// bound action (upstream `HotkeyManager` dictionary of action lists).
class PluginHotkeyRegistrar implements HotkeyRegistrar {
  const PluginHotkeyRegistrar();

  @override
  Future<(Set<GlobalHotkeyAction>, List<String>)> register(
    List<HotkeyRegistration> registrations, {
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
    for (final registration in registrations) {
      final actions = registration.actions;
      if (actions.isEmpty) continue;
      // Persisted value is the WPF Key enum; resolve via WPF -> VK -> Flutter.
      final key = codec.physicalFromWpf(registration.combo.keyCode);
      if (key == null) {
        failures.add(
          '${_labelsFor(actions)}: 不支持的按键 #${registration.combo.keyCode}',
        );
        continue;
      }
      final modifiers = <HotKeyModifier>[];
      if (registration.combo.control) modifiers.add(HotKeyModifier.control);
      if (registration.combo.alt) modifiers.add(HotKeyModifier.alt);
      if (registration.combo.shift) modifiers.add(HotKeyModifier.shift);
      final dispatch = List<GlobalHotkeyAction>.unmodifiable(actions);
      try {
        await manager.register(
          HotKey(
            identifier: registration.combo.id,
            key: key,
            modifiers: modifiers.isEmpty ? null : modifiers,
            scope: HotKeyScope.system,
          ),
          keyDownHandler: onTriggered == null
              ? null
              : (hotKey) {
                  for (final action in dispatch) {
                    onTriggered(action);
                  }
                },
        );
        accepted.addAll(dispatch);
      } on Object catch (e) {
        failures.add('${_labelsFor(actions)}: 注册失败 ${e.toString()}');
      }
    }
    return (accepted, failures);
  }

  String _labelsFor(List<GlobalHotkeyAction> actions) =>
      actions.map((a) => a.label).join('/');

  @override
  Future<void> unregisterAll() => HotKeyManager.instance.unregisterAll();
}

final hotkeyRegistrarProvider = Provider<HotkeyRegistrar>(
  (ref) => const PluginHotkeyRegistrar(),
);

final hotkeyControllerProvider =
    NotifierProvider<HotkeyController, HotkeyState>(HotkeyController.new);

class HotkeyController extends Notifier<HotkeyState> {
  @override
  HotkeyState build() => const HotkeyState();

  /// True while the editor holds native dispatch paused (`HotkeyManager.IsPause`).
  bool _paused = false;

  bool get isPaused => _paused;

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

  /// Reload bindings from the (possibly restored) settings document and
  /// re-register them, so native dispatch matches the restored configuration
  /// after a backup restore (R3-SET-04).
  ///
  /// Registration only runs when the shell has installed its live dispatcher
  /// ([hotkeyDispatchProvider]); in a pure widget/test context there is no
  /// native target, so the bindings are reloaded without touching the plugin.
  Future<HotkeyState> reloadFromSettings() async {
    loadFromSettings();
    if (ref.read(hotkeyDispatchProvider).handler == null) {
      return state;
    }
    return registerAll();
  }

  /// Pause native dispatch while the hotkey editor is open (upstream
  /// `HotkeyManager.IsPause`). The live registration is dropped so a combo
  /// pressed during recording reaches the Flutter editor instead of firing its
  /// saved action; [cancelEdit] restores the dropped registration.
  Future<void> beginEdit() async {
    if (_paused) return;
    _paused = true;
    try {
      await _registrar.unregisterAll();
    } on Object catch (_) {
      // Best effort: a missing native plugin must not block the editor.
    }
  }

  /// Leave the editor without saving: re-register the bindings captured before
  /// [beginEdit] (upstream `Closing -> IsPause = false`).
  Future<void> cancelEdit() async {
    if (!_paused) return;
    _paused = false;
    await registerAll();
  }

  /// (Re)register all bound hotkeys. Same `modifiers + Key` combos are grouped
  /// into one OS registration whose handler dispatches every action (upstream
  /// `Dictionary<int, List<EGlobalHotkey>>`). Reports conflicts instead of
  /// pretending success. Uses the shell-installed dispatcher so a real key press
  /// reaches the same window/proxy entry points as the menus (RT-12).
  Future<HotkeyState> registerAll() async {
    final handler = ref.read(hotkeyDispatchProvider).handler;
    Set<GlobalHotkeyAction> accepted;
    List<String> failures;
    try {
      (accepted, failures) = await _registrar.register(
        groupHotkeyBindings(state.bindings),
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
  /// Returns `true` only when the settings save *and* every native registration
  /// succeeded, so the window stays open on a conflict instead of closing over a
  /// silently-dead binding.
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
    if (state.conflicts.isEmpty) {
      // Editing is over and every combination registered: resume live dispatch.
      _paused = false;
      return true;
    }
    // A partial/failed registration keeps the window open (the caller returns
    // false). The successful combinations must NOT fire while the editor is
    // still recording (R3-SET-05): drop the native handlers and stay paused
    // until the edit ends via [cancelEdit] or a later successful save.
    _paused = true;
    try {
      await _registrar.unregisterAll();
    } on Object catch (_) {
      // Best effort: reporting the conflict is what matters.
    }
    state = state.copyWith(registered: const <GlobalHotkeyAction>{});
    return false;
  }
}
