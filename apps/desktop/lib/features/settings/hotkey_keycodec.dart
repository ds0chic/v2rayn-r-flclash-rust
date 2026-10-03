import 'package:flutter/services.dart';

/// Explicit conversion contract between the three hotkey key encodings:
///
/// * **persisted** — the Windows Presentation Foundation `System.Windows.Input.Key`
///   enum integer, exactly what upstream `KeyEventItem.KeyCode` stores
///   (`GlobalHotkeySettingWindow.xaml.cs` writes `(int)e.Key`);
/// * **virtual key (VK)** — the Win32 virtual-key code, which upstream derives
///   with `KeyInterop.VirtualKeyFromKey((Key)item.KeyCode)` in
///   `HotkeyManager.Register`, and which the Windows `RegisterHotKey` API wants;
/// * **Flutter** — the `LogicalKeyboardKey`/`PhysicalKeyboardKey` identity used
///   while recording and by `hotkey_manager`.
///
/// Persisting the WPF enum (not a raw Flutter key id) keeps `guiNConfig.json`
/// round-trippable with the frozen v2rayN config, and makes the register path
/// an explicit `WPF Key -> VK -> Flutter key` conversion instead of the previous
/// "record a Flutter key id, register it as a VK" mismatch (SET-15).
class HotkeyKeyCodec {
  const HotkeyKeyCodec();

  /// `Key.None`; upstream treats this (and `null`/0) as "not registered".
  static const int wpfNone = 0;

  /// Record a Flutter key event into the persisted WPF `Key` enum value.
  ///
  /// Logical identity is preferred (upstream WPF `e.Key` is layout-aware), with
  /// the physical key as a fallback for events whose logical key differs under
  /// a non-US layout. Returns [wpfNone] when the key is not in the supported,
  /// verified set — callers must treat that as "unrecordable", never guess.
  int wpfKeyForEvent(KeyEvent event) {
    final logical = wpfKeyFromLogical(event.logicalKey);
    if (logical != wpfNone) return logical;
    return wpfKeyFromPhysical(event.physicalKey);
  }

  /// Persisted WPF `Key` for a Flutter logical key, or [wpfNone].
  int wpfKeyFromLogical(LogicalKeyboardKey key) {
    for (final e in _entries) {
      if (e.logical == key) return e.wpf;
    }
    return wpfNone;
  }

  /// Persisted WPF `Key` for a Flutter physical key, or [wpfNone].
  int wpfKeyFromPhysical(PhysicalKeyboardKey key) {
    for (final e in _entries) {
      if (e.physical == key) return e.wpf;
    }
    return wpfNone;
  }

  /// Win32 virtual-key code for a persisted WPF `Key` value (upstream
  /// `KeyInterop.VirtualKeyFromKey`). Returns `null` for unsupported keys.
  int? virtualKeyFromWpf(int? wpfKey) {
    if (wpfKey == null || wpfKey == wpfNone) return null;
    for (final e in _entries) {
      if (e.wpf == wpfKey) return e.vk;
    }
    return null;
  }

  /// Persisted WPF `Key` value for a Win32 virtual-key code, or [wpfNone].
  int wpfFromVirtualKey(int? virtualKey) {
    if (virtualKey == null || virtualKey == 0) return wpfNone;
    for (final e in _entries) {
      if (e.vk == virtualKey) return e.wpf;
    }
    return wpfNone;
  }

  /// Flutter physical key the `hotkey_manager` registrar should register for a
  /// persisted WPF `Key` value. Returns `null` for unsupported keys so the
  /// registrar reports a conflict rather than registering the wrong key.
  PhysicalKeyboardKey? physicalFromWpf(int? wpfKey) {
    if (wpfKey == null || wpfKey == wpfNone) return null;
    for (final e in _entries) {
      if (e.wpf == wpfKey) return e.physical;
    }
    return null;
  }

  /// Human label for a persisted WPF `Key` value (`Key.None` -> null).
  String? labelForWpf(int? wpfKey) {
    if (wpfKey == null || wpfKey == wpfNone) return null;
    for (final e in _entries) {
      if (e.wpf == wpfKey) return e.label;
    }
    return null;
  }

  /// Whether the pressed key is a bare modifier. Upstream keeps recording and
  /// only sets the modifier flags for these (never terminates recording on a
  /// modifier keydown; see `TxtGlobalHotkey_PreviewKeyDown`).
  static bool isModifierKey(LogicalKeyboardKey key) =>
      key == LogicalKeyboardKey.controlLeft ||
      key == LogicalKeyboardKey.controlRight ||
      key == LogicalKeyboardKey.shiftLeft ||
      key == LogicalKeyboardKey.shiftRight ||
      key == LogicalKeyboardKey.altLeft ||
      key == LogicalKeyboardKey.altRight ||
      key == LogicalKeyboardKey.metaLeft ||
      key == LogicalKeyboardKey.metaRight;
}

class _KeyEntry {
  const _KeyEntry(this.wpf, this.vk, this.logical, this.physical, this.label);

  final int wpf;
  final int vk;
  final LogicalKeyboardKey logical;
  final PhysicalKeyboardKey physical;
  final String label;
}

final List<_KeyEntry> _entries = _buildEntries();

List<_KeyEntry> _buildEntries() {
  final out = <_KeyEntry>[];

  const lettersL = <LogicalKeyboardKey>[
    LogicalKeyboardKey.keyA,
    LogicalKeyboardKey.keyB,
    LogicalKeyboardKey.keyC,
    LogicalKeyboardKey.keyD,
    LogicalKeyboardKey.keyE,
    LogicalKeyboardKey.keyF,
    LogicalKeyboardKey.keyG,
    LogicalKeyboardKey.keyH,
    LogicalKeyboardKey.keyI,
    LogicalKeyboardKey.keyJ,
    LogicalKeyboardKey.keyK,
    LogicalKeyboardKey.keyL,
    LogicalKeyboardKey.keyM,
    LogicalKeyboardKey.keyN,
    LogicalKeyboardKey.keyO,
    LogicalKeyboardKey.keyP,
    LogicalKeyboardKey.keyQ,
    LogicalKeyboardKey.keyR,
    LogicalKeyboardKey.keyS,
    LogicalKeyboardKey.keyT,
    LogicalKeyboardKey.keyU,
    LogicalKeyboardKey.keyV,
    LogicalKeyboardKey.keyW,
    LogicalKeyboardKey.keyX,
    LogicalKeyboardKey.keyY,
    LogicalKeyboardKey.keyZ,
  ];
  const lettersP = <PhysicalKeyboardKey>[
    PhysicalKeyboardKey.keyA,
    PhysicalKeyboardKey.keyB,
    PhysicalKeyboardKey.keyC,
    PhysicalKeyboardKey.keyD,
    PhysicalKeyboardKey.keyE,
    PhysicalKeyboardKey.keyF,
    PhysicalKeyboardKey.keyG,
    PhysicalKeyboardKey.keyH,
    PhysicalKeyboardKey.keyI,
    PhysicalKeyboardKey.keyJ,
    PhysicalKeyboardKey.keyK,
    PhysicalKeyboardKey.keyL,
    PhysicalKeyboardKey.keyM,
    PhysicalKeyboardKey.keyN,
    PhysicalKeyboardKey.keyO,
    PhysicalKeyboardKey.keyP,
    PhysicalKeyboardKey.keyQ,
    PhysicalKeyboardKey.keyR,
    PhysicalKeyboardKey.keyS,
    PhysicalKeyboardKey.keyT,
    PhysicalKeyboardKey.keyU,
    PhysicalKeyboardKey.keyV,
    PhysicalKeyboardKey.keyW,
    PhysicalKeyboardKey.keyX,
    PhysicalKeyboardKey.keyY,
    PhysicalKeyboardKey.keyZ,
  ];
  for (var i = 0; i < 26; i++) {
    final ch = String.fromCharCode(0x41 + i);
    out.add(_KeyEntry(44 + i, 0x41 + i, lettersL[i], lettersP[i], ch));
  }

  const digitsL = <LogicalKeyboardKey>[
    LogicalKeyboardKey.digit0,
    LogicalKeyboardKey.digit1,
    LogicalKeyboardKey.digit2,
    LogicalKeyboardKey.digit3,
    LogicalKeyboardKey.digit4,
    LogicalKeyboardKey.digit5,
    LogicalKeyboardKey.digit6,
    LogicalKeyboardKey.digit7,
    LogicalKeyboardKey.digit8,
    LogicalKeyboardKey.digit9,
  ];
  const digitsP = <PhysicalKeyboardKey>[
    PhysicalKeyboardKey.digit0,
    PhysicalKeyboardKey.digit1,
    PhysicalKeyboardKey.digit2,
    PhysicalKeyboardKey.digit3,
    PhysicalKeyboardKey.digit4,
    PhysicalKeyboardKey.digit5,
    PhysicalKeyboardKey.digit6,
    PhysicalKeyboardKey.digit7,
    PhysicalKeyboardKey.digit8,
    PhysicalKeyboardKey.digit9,
  ];
  for (var i = 0; i < 10; i++) {
    out.add(_KeyEntry(34 + i, 0x30 + i, digitsL[i], digitsP[i], '$i'));
  }

  const fL = <LogicalKeyboardKey>[
    LogicalKeyboardKey.f1,
    LogicalKeyboardKey.f2,
    LogicalKeyboardKey.f3,
    LogicalKeyboardKey.f4,
    LogicalKeyboardKey.f5,
    LogicalKeyboardKey.f6,
    LogicalKeyboardKey.f7,
    LogicalKeyboardKey.f8,
    LogicalKeyboardKey.f9,
    LogicalKeyboardKey.f10,
    LogicalKeyboardKey.f11,
    LogicalKeyboardKey.f12,
    LogicalKeyboardKey.f13,
    LogicalKeyboardKey.f14,
    LogicalKeyboardKey.f15,
    LogicalKeyboardKey.f16,
    LogicalKeyboardKey.f17,
    LogicalKeyboardKey.f18,
    LogicalKeyboardKey.f19,
    LogicalKeyboardKey.f20,
    LogicalKeyboardKey.f21,
    LogicalKeyboardKey.f22,
    LogicalKeyboardKey.f23,
    LogicalKeyboardKey.f24,
  ];
  const fP = <PhysicalKeyboardKey>[
    PhysicalKeyboardKey.f1,
    PhysicalKeyboardKey.f2,
    PhysicalKeyboardKey.f3,
    PhysicalKeyboardKey.f4,
    PhysicalKeyboardKey.f5,
    PhysicalKeyboardKey.f6,
    PhysicalKeyboardKey.f7,
    PhysicalKeyboardKey.f8,
    PhysicalKeyboardKey.f9,
    PhysicalKeyboardKey.f10,
    PhysicalKeyboardKey.f11,
    PhysicalKeyboardKey.f12,
    PhysicalKeyboardKey.f13,
    PhysicalKeyboardKey.f14,
    PhysicalKeyboardKey.f15,
    PhysicalKeyboardKey.f16,
    PhysicalKeyboardKey.f17,
    PhysicalKeyboardKey.f18,
    PhysicalKeyboardKey.f19,
    PhysicalKeyboardKey.f20,
    PhysicalKeyboardKey.f21,
    PhysicalKeyboardKey.f22,
    PhysicalKeyboardKey.f23,
    PhysicalKeyboardKey.f24,
  ];
  for (var i = 0; i < 24; i++) {
    out.add(_KeyEntry(90 + i, 0x70 + i, fL[i], fP[i], 'F${i + 1}'));
  }

  const npL = <LogicalKeyboardKey>[
    LogicalKeyboardKey.numpad0,
    LogicalKeyboardKey.numpad1,
    LogicalKeyboardKey.numpad2,
    LogicalKeyboardKey.numpad3,
    LogicalKeyboardKey.numpad4,
    LogicalKeyboardKey.numpad5,
    LogicalKeyboardKey.numpad6,
    LogicalKeyboardKey.numpad7,
    LogicalKeyboardKey.numpad8,
    LogicalKeyboardKey.numpad9,
  ];
  const npP = <PhysicalKeyboardKey>[
    PhysicalKeyboardKey.numpad0,
    PhysicalKeyboardKey.numpad1,
    PhysicalKeyboardKey.numpad2,
    PhysicalKeyboardKey.numpad3,
    PhysicalKeyboardKey.numpad4,
    PhysicalKeyboardKey.numpad5,
    PhysicalKeyboardKey.numpad6,
    PhysicalKeyboardKey.numpad7,
    PhysicalKeyboardKey.numpad8,
    PhysicalKeyboardKey.numpad9,
  ];
  for (var i = 0; i < 10; i++) {
    out.add(_KeyEntry(74 + i, 0x60 + i, npL[i], npP[i], 'NumPad$i'));
  }

  void named(
    int wpf,
    int vk,
    LogicalKeyboardKey l,
    PhysicalKeyboardKey p,
    String label,
  ) => out.add(_KeyEntry(wpf, vk, l, p, label));

  named(18, 0x20, LogicalKeyboardKey.space, PhysicalKeyboardKey.space, 'Space');
  named(
    13,
    0x1B,
    LogicalKeyboardKey.escape,
    PhysicalKeyboardKey.escape,
    'Escape',
  );
  named(3, 0x09, LogicalKeyboardKey.tab, PhysicalKeyboardKey.tab, 'Tab');
  named(
    2,
    0x08,
    LogicalKeyboardKey.backspace,
    PhysicalKeyboardKey.backspace,
    'Back',
  );
  named(6, 0x0D, LogicalKeyboardKey.enter, PhysicalKeyboardKey.enter, 'Return');
  named(
    31,
    0x2D,
    LogicalKeyboardKey.insert,
    PhysicalKeyboardKey.insert,
    'Insert',
  );
  named(
    32,
    0x2E,
    LogicalKeyboardKey.delete,
    PhysicalKeyboardKey.delete,
    'Delete',
  );
  named(22, 0x24, LogicalKeyboardKey.home, PhysicalKeyboardKey.home, 'Home');
  named(21, 0x23, LogicalKeyboardKey.end, PhysicalKeyboardKey.end, 'End');
  named(
    19,
    0x21,
    LogicalKeyboardKey.pageUp,
    PhysicalKeyboardKey.pageUp,
    'PageUp',
  );
  named(
    20,
    0x22,
    LogicalKeyboardKey.pageDown,
    PhysicalKeyboardKey.pageDown,
    'PageDown',
  );
  named(
    23,
    0x25,
    LogicalKeyboardKey.arrowLeft,
    PhysicalKeyboardKey.arrowLeft,
    'Left',
  );
  named(
    25,
    0x27,
    LogicalKeyboardKey.arrowRight,
    PhysicalKeyboardKey.arrowRight,
    'Right',
  );
  named(
    24,
    0x26,
    LogicalKeyboardKey.arrowUp,
    PhysicalKeyboardKey.arrowUp,
    'Up',
  );
  named(
    26,
    0x28,
    LogicalKeyboardKey.arrowDown,
    PhysicalKeyboardKey.arrowDown,
    'Down',
  );

  named(
    84,
    0x6A,
    LogicalKeyboardKey.numpadMultiply,
    PhysicalKeyboardKey.numpadMultiply,
    'Multiply',
  );
  named(
    85,
    0x6B,
    LogicalKeyboardKey.numpadAdd,
    PhysicalKeyboardKey.numpadAdd,
    'Add',
  );
  named(
    87,
    0x6D,
    LogicalKeyboardKey.numpadSubtract,
    PhysicalKeyboardKey.numpadSubtract,
    'Subtract',
  );
  named(
    88,
    0x6E,
    LogicalKeyboardKey.numpadDecimal,
    PhysicalKeyboardKey.numpadDecimal,
    'Decimal',
  );
  named(
    89,
    0x6F,
    LogicalKeyboardKey.numpadDivide,
    PhysicalKeyboardKey.numpadDivide,
    'Divide',
  );

  named(
    140,
    0xBA,
    LogicalKeyboardKey.semicolon,
    PhysicalKeyboardKey.semicolon,
    'OemSemicolon',
  );
  named(
    141,
    0xBB,
    LogicalKeyboardKey.equal,
    PhysicalKeyboardKey.equal,
    'OemPlus',
  );
  named(
    142,
    0xBC,
    LogicalKeyboardKey.comma,
    PhysicalKeyboardKey.comma,
    'OemComma',
  );
  named(
    143,
    0xBD,
    LogicalKeyboardKey.minus,
    PhysicalKeyboardKey.minus,
    'OemMinus',
  );
  named(
    144,
    0xBE,
    LogicalKeyboardKey.period,
    PhysicalKeyboardKey.period,
    'OemPeriod',
  );
  named(
    145,
    0xBF,
    LogicalKeyboardKey.slash,
    PhysicalKeyboardKey.slash,
    'OemQuestion',
  );
  named(
    146,
    0xC0,
    LogicalKeyboardKey.backquote,
    PhysicalKeyboardKey.backquote,
    'OemTilde',
  );
  named(
    149,
    0xDB,
    LogicalKeyboardKey.bracketLeft,
    PhysicalKeyboardKey.bracketLeft,
    'OemOpenBrackets',
  );
  named(
    150,
    0xDC,
    LogicalKeyboardKey.backslash,
    PhysicalKeyboardKey.backslash,
    'OemPipe',
  );
  named(
    151,
    0xDD,
    LogicalKeyboardKey.bracketRight,
    PhysicalKeyboardKey.bracketRight,
    'OemCloseBrackets',
  );
  named(
    152,
    0xDE,
    LogicalKeyboardKey.quote,
    PhysicalKeyboardKey.quote,
    'OemQuotes',
  );

  named(
    30,
    0x2C,
    LogicalKeyboardKey.printScreen,
    PhysicalKeyboardKey.printScreen,
    'PrintScreen',
  );
  named(7, 0x13, LogicalKeyboardKey.pause, PhysicalKeyboardKey.pause, 'Pause');
  named(
    114,
    0x90,
    LogicalKeyboardKey.numLock,
    PhysicalKeyboardKey.numLock,
    'NumLock',
  );
  named(
    115,
    0x91,
    LogicalKeyboardKey.scrollLock,
    PhysicalKeyboardKey.scrollLock,
    'Scroll',
  );

  return out;
}
