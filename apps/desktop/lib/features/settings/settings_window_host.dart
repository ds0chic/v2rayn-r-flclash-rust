import 'dart:async';
import 'dart:convert';

import 'package:flutter/services.dart';

/// Method channel used by the native option-window host. Both the main engine
/// and the settings-window engine bind this channel on their own messenger.
const String kOptionWindowChannel = 'v2rayn/option_window';

/// Result of asking the main window to persist an edited settings draft.
class SettingsEditorOutcome {
  const SettingsEditorOutcome({required this.ok, this.message});

  final bool ok;

  /// User-facing text. When [ok] is false it is the error to keep the window
  /// open with; when [ok] is true it may carry a non-fatal notice (e.g. the
  /// "needs restart" hint) for the window to surface before it closes.
  final String? message;
}

/// Thrown when the settings editor cannot read its starting snapshot from the
/// main engine. R4-12/D34: a read failure must surface as an error, never as an
/// empty document that 确定 could persist over the real settings.
class SettingsEditorLoadException implements Exception {
  const SettingsEditorLoadException([this.message = '读取配置失败']);

  final String message;

  @override
  String toString() => 'SettingsEditorLoadException: $message';
}

/// Persistence/close seam for the option settings UI. The real desktop
/// implementation talks to the main window through the native host; tests use
/// an in-memory fake.
abstract class SettingsEditorHost {
  /// The settings snapshot the editor starts from.
  Future<Map<String, dynamic>> loadSnapshot();

  /// Persist [draft] through the main window's settings path.
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft);

  /// Close the settings window without saving.
  Future<void> close();
}

/// Main-window side of the native host. Registers the `applyDraft` callback and
/// asks the native side to open the independent top-level settings window.
class OptionWindowHost {
  OptionWindowHost._();

  static final OptionWindowHost instance = OptionWindowHost._();

  static const MethodChannel _channel = MethodChannel(kOptionWindowChannel);

  Future<SettingsEditorOutcome> Function(String draftJson)? _save;

  /// Opens the settings window with [snapshot]. [onSave] runs in the main
  /// engine and must persist the draft through the existing settings path.
  /// Returns false when the native window could not be created.
  Future<bool> open({
    required Map<String, dynamic> snapshot,
    required Future<SettingsEditorOutcome> Function(String draftJson) onSave,
  }) async {
    _save = onSave;
    _channel.setMethodCallHandler(_handle);
    try {
      final opened = await _channel.invokeMethod<bool>(
        'open',
        jsonEncode(snapshot),
      );
      return opened ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  Future<dynamic> _handle(MethodCall call) async {
    if (call.method != 'applyDraft') return null;
    final args = (call.arguments as Map).cast<String, dynamic>();
    final id = args['id'];
    final draftJson = args['draft'] as String? ?? '{}';
    final save = _save;
    final outcome = save == null
        ? const SettingsEditorOutcome(ok: false, message: '保存配置失败')
        : await save(draftJson);
    await _channel.invokeMethod<void>('reportOutcome', <String, dynamic>{
      'id': id,
      'ok': outcome.ok,
      'message': outcome.message,
    });
    return null;
  }
}

/// Settings-window side of the native host. Runs in the second Flutter engine,
/// which has no Rust bridge handle; every mutation is relayed to the main
/// engine, which performs the actual save.
class NativeSettingsEditorHost implements SettingsEditorHost {
  NativeSettingsEditorHost() {
    _ready = _init();
  }

  static const MethodChannel _channel = MethodChannel(kOptionWindowChannel);

  final Map<int, Completer<SettingsEditorOutcome>> _pending =
      <int, Completer<SettingsEditorOutcome>>{};
  late final Future<void> _ready;
  int _nextId = 1;
  String _snapshotJson = '{}';
  bool _loadFailed = false;

  /// Fetches the snapshot from the native host. The host installs its channel
  /// handler right after the window is created, so retry briefly on the
  /// start-up race. R4-12/D34: exhausting the retries is a hard read failure,
  /// not a valid empty document.
  Future<void> _init() async {
    _channel.setMethodCallHandler(_handle);
    for (var attempt = 0; attempt < 50; attempt++) {
      try {
        final value = await _channel.invokeMethod<String>('ready');
        if (value != null && value.isNotEmpty && value != '{}') {
          _snapshotJson = value;
          _loadFailed = false;
          return;
        }
      } on MissingPluginException {
        // handler not installed yet
      } on PlatformException {
        // transient; retry
      }
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    _loadFailed = true;
  }

  @override
  Future<Map<String, dynamic>> loadSnapshot() async {
    await _ready;
    final raw = _snapshotJson.trim();
    if (_loadFailed || raw.isEmpty || raw == '{}') {
      throw const SettingsEditorLoadException('读取配置失败');
    }
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map<String, dynamic> && decoded.isNotEmpty) {
        return decoded;
      }
    } catch (_) {}
    throw const SettingsEditorLoadException('读取配置失败');
  }

  @override
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft) async {
    final id = _nextId++;
    final completer = Completer<SettingsEditorOutcome>();
    _pending[id] = completer;
    try {
      await _channel.invokeMethod<void>('saveDraft', <String, dynamic>{
        'id': id,
        'draft': jsonEncode(draft),
      });
    } catch (_) {
      _pending.remove(id);
      return const SettingsEditorOutcome(ok: false, message: '保存配置失败');
    }
    return completer.future;
  }

  @override
  Future<void> close() async {
    try {
      await _channel.invokeMethod<void>('close');
    } catch (_) {}
  }

  Future<dynamic> _handle(MethodCall call) async {
    if (call.method != 'saveOutcome') return null;
    final args = (call.arguments as Map).cast<String, dynamic>();
    final id = args['id'] as int?;
    final completer = id == null ? null : _pending.remove(id);
    if (completer != null && !completer.isCompleted) {
      completer.complete(
        SettingsEditorOutcome(
          ok: args['ok'] == true,
          message: args['message'] as String?,
        ),
      );
    }
    return null;
  }
}
