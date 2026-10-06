import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Method channel used by the native option-window host. Both the main engine
/// and the settings-window engine bind this channel on their own messenger.
const String kOptionWindowChannel = 'v2rayn/option_window';

/// SP-11: outcome status of one independent-window request. Mirrors
/// `WindowOutcomeStatus` in `crates/ipc_contract/src/stable.rs`. The
/// MethodChannel transport ACK (a `saveDraft` call returning) only means the
/// request arrived; the business result travels as a structured outcome with
/// one of these statuses. `pendingConfirmation` means the bounded wait
/// expired with an unknown result: the caller must query the mutation instead
/// of replaying a non-idempotent save, and must never wait forever.
enum WindowOutcomeStatus { ok, failed, pendingConfirmation }

/// Wire name of a [WindowOutcomeStatus] (matches the Rust snake_case JSON).
String windowOutcomeStatusName(WindowOutcomeStatus status) => switch (status) {
  WindowOutcomeStatus.ok => 'ok',
  WindowOutcomeStatus.failed => 'failed',
  WindowOutcomeStatus.pendingConfirmation => 'pendingConfirmation',
};

/// Tolerant decode: unknown/missing names are failures, never silent success.
WindowOutcomeStatus windowOutcomeStatusFromName(Object? name) => switch (name) {
  'ok' => WindowOutcomeStatus.ok,
  'pendingConfirmation' => WindowOutcomeStatus.pendingConfirmation,
  _ => WindowOutcomeStatus.failed,
};

/// Identifies one timed-out request for the reconciler seam. The reconciler
/// (wired by the root integrator to `querySettingsMutation`/operation query)
/// only observes; it must not resend the save.
class WindowPendingQuery {
  const WindowPendingQuery({
    required this.windowId,
    required this.windowGeneration,
    required this.requestId,
    this.mutationId,
  });

  final String windowId;
  final int windowGeneration;
  final String requestId;
  final String? mutationId;
}

class _PendingWindowReply {
  _PendingWindowReply({required this.generation, required this.onExpire});

  final int generation;
  final void Function() onExpire;
  Timer? timer;
  bool settled = false;
}

/// SP-11 reply gate: tracks one window's in-flight requests by
/// `(windowGeneration, requestId)`.
///
/// - Late, duplicate, or after-close replies are dropped by the caller via
///   [settle]; only a live request with a matching generation settles.
/// - [begin]'s timer bounds the wait; expiry runs `onExpire` (the caller
///   completes PendingConfirmation there) instead of hanging.
/// - [abandonAll] settles everything (the caller fails each id) and rotates
///   the generation so a reopened window never accepts the old window's
///   replies.
class WindowReplyGate {
  WindowReplyGate({Duration? outcomeTimeout, int? initialGeneration})
    : outcomeTimeout = outcomeTimeout ?? const Duration(seconds: 10),
      generation = initialGeneration ?? 1;

  /// Bounded wait for one outcome before entering PendingConfirmation.
  final Duration outcomeTimeout;

  /// Current window generation. Bumped by [abandonAll] (close/reopen).
  int generation;

  int _seq = 0;
  final Map<String, _PendingWindowReply> _pending = {};

  /// Registers a request; returns its stable request id.
  String begin(void Function() onExpire) {
    _seq += 1;
    final requestId = 'req-$generation-$_seq';
    final entry = _PendingWindowReply(
      generation: generation,
      onExpire: onExpire,
    );
    entry.timer = Timer(outcomeTimeout, () => expire(requestId));
    _pending[requestId] = entry;
    return requestId;
  }

  /// Settles a live request with a matching generation. Late, duplicate, or
  /// old-generation replies return false and are dropped by the caller.
  bool settle(String requestId, int generation) {
    final entry = _pending[requestId];
    if (entry == null ||
        entry.settled ||
        entry.generation != generation ||
        generation != this.generation) {
      return false;
    }
    _pending.remove(requestId);
    entry.settled = true;
    entry.timer?.cancel();
    return true;
  }

  /// Cancels a registration whose transport failed; the caller answers
  /// directly, so no timeout may fire afterwards.
  void cancel(String requestId) {
    final entry = _pending.remove(requestId);
    if (entry == null || entry.settled) return;
    entry.settled = true;
    entry.timer?.cancel();
  }

  /// Fires the expiry path early (test seam for the bounded wait).
  @visibleForTesting
  void expireForTest(String requestId) => expire(requestId);

  void expire(String requestId) {
    final entry = _pending.remove(requestId);
    if (entry == null || entry.settled || entry.generation != generation) {
      return;
    }
    entry.settled = true;
    entry.timer?.cancel();
    entry.onExpire();
  }

  /// Returns live request ids, clears them, and rotates the generation so
  /// late replies from the closing window are filtered out.
  List<String> abandonAll() {
    final ids = _pending.keys.toList();
    for (final entry in _pending.values) {
      entry.settled = true;
      entry.timer?.cancel();
    }
    _pending.clear();
    generation += 1;
    return ids;
  }

  /// Live in-flight request count (observability for tests and UI).
  int get pendingCount => _pending.length;
}

/// Result of asking the main window to persist an edited settings draft.
class SettingsEditorOutcome {
  const SettingsEditorOutcome({
    required this.ok,
    this.message,
    this.pendingConfirmation = false,
  });

  final bool ok;

  /// User-facing text. When [ok] is false it is the error to keep the window
  /// open with; when [ok] is true it may carry a non-fatal notice (e.g. the
  /// "needs restart" hint) for the window to surface before it closes.
  final String? message;

  /// SP-11: the bounded wait expired with an unknown result. The window must
  /// stay open showing [message], offer a mutation query/retry owned by the
  /// integrator, and must not auto-replay the save.
  final bool pendingConfirmation;
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
    final args =
        (call.arguments as Map?)?.cast<String, dynamic>() ??
        <String, dynamic>{};
    final id = args['id'];
    final requestId = args['requestId'] as String?;
    final generation = (args['windowGeneration'] as num?)?.toInt();
    final draftJson = args['draft'] as String? ?? '{}';
    final save = _save;
    // SP-11: an exception is a structured failed outcome, never a dropped
    // reply. Without this the settings engine would wait forever.
    SettingsEditorOutcome outcome;
    if (save == null) {
      outcome = const SettingsEditorOutcome(ok: false, message: '保存配置失败');
    } else {
      try {
        outcome = await save(draftJson);
      } catch (_) {
        outcome = const SettingsEditorOutcome(ok: false, message: '保存配置失败');
      }
    }
    try {
      await _channel.invokeMethod<void>('reportOutcome', <String, dynamic>{
        'id': id,
        'ok': outcome.ok,
        'requestId': ?requestId,
        'windowGeneration': ?generation,
        'status': windowOutcomeStatusName(
          outcome.pendingConfirmation
              ? WindowOutcomeStatus.pendingConfirmation
              : (outcome.ok
                    ? WindowOutcomeStatus.ok
                    : WindowOutcomeStatus.failed),
        ),
        if (outcome.message != null) 'message': outcome.message,
      });
    } catch (_) {
      // Window already closed: the settings engine's bounded wait settles it.
    }
    return null;
  }

  @visibleForTesting
  Future<void> debugDispatchApplyDraft(Map<String, dynamic> args) =>
      _handle(MethodCall('applyDraft', args));
}

/// Settings-window side of the native host. Runs in the second Flutter engine,
/// which has no Rust bridge handle; every mutation is relayed to the main
/// engine, which performs the actual save.
///
/// SP-11: the `saveDraft` transport ACK is not the result. The result arrives
/// as `saveOutcome` keyed by `(windowGeneration, requestId)`. A missing reply
/// settles as PendingConfirmation after [WindowReplyGate.outcomeTimeout];
/// close settles pending requests and rotates the generation so late replies
/// from the closing window are dropped.
class NativeSettingsEditorHost implements SettingsEditorHost {
  NativeSettingsEditorHost({
    Duration? outcomeTimeout,
    this._reconciler,
    this._onReconciled,
  }) : _gate = WindowReplyGate(outcomeTimeout: outcomeTimeout) {
    _ready = _init();
  }

  static const MethodChannel _channel = MethodChannel(kOptionWindowChannel);

  final WindowReplyGate _gate;
  final Future<SettingsEditorOutcome?> Function(WindowPendingQuery query)?
  _reconciler;
  final void Function(SettingsEditorOutcome reconciled)? _onReconciled;
  final Map<String, Completer<SettingsEditorOutcome>> _pending =
      <String, Completer<SettingsEditorOutcome>>{};
  // Legacy numeric ids for runners/handlers that do not echo `requestId`.
  final Map<Object?, String> _legacyIds = <Object?, String>{};
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
    late final String requestId;
    requestId = _gate.begin(() {
      _pending.remove(requestId);
      _legacyIds.remove(id);
      if (!completer.isCompleted) {
        completer.complete(
          const SettingsEditorOutcome(
            ok: false,
            pendingConfirmation: true,
            message: '保存结果待确认',
          ),
        );
      }
      _reconcileAfterTimeout(requestId);
    });
    _pending[requestId] = completer;
    _legacyIds[id] = requestId;
    try {
      // Transport ACK only: a successful return means the runner/main engine
      // received the request. The business result arrives via `saveOutcome`.
      await _channel.invokeMethod<void>('saveDraft', <String, dynamic>{
        'id': id,
        'draft': jsonEncode(draft),
        'requestId': requestId,
        'windowGeneration': _gate.generation,
        'mutationId': 'win-settings-$requestId',
      });
    } catch (_) {
      _gate.cancel(requestId);
      _pending.remove(requestId);
      _legacyIds.remove(id);
      return const SettingsEditorOutcome(ok: false, message: '保存配置失败');
    }
    return completer.future;
  }

  /// SP-11 reconciler seam: after the bounded wait expires, an integrator
  /// wired query (e.g. `querySettingsMutation`) may observe the real result.
  /// Never resends `saveDraft`: replaying a non-idempotent save is forbidden.
  void _reconcileAfterTimeout(String requestId) {
    final reconciler = _reconciler;
    if (reconciler == null) return;
    unawaited(() async {
      try {
        final outcome = await reconciler(
          WindowPendingQuery(
            windowId: 'settings',
            windowGeneration: _gate.generation,
            requestId: requestId,
            mutationId: 'win-settings-$requestId',
          ),
        );
        if (outcome != null) _onReconciled?.call(outcome);
      } catch (_) {}
    }());
  }

  @override
  Future<void> close() async {
    for (final requestId in _gate.abandonAll()) {
      final completer = _pending.remove(requestId);
      if (completer != null && !completer.isCompleted) {
        completer.complete(
          const SettingsEditorOutcome(ok: false, message: '窗口已关闭'),
        );
      }
    }
    _legacyIds.clear();
    try {
      await _channel.invokeMethod<void>('close');
    } catch (_) {}
  }

  Future<dynamic> _handle(MethodCall call) async {
    if (call.method != 'saveOutcome') return null;
    final args =
        (call.arguments as Map?)?.cast<String, dynamic>() ??
        <String, dynamic>{};
    final requestId = _resolveRequestId(args);
    if (requestId == null) return null;
    final generation =
        (args['windowGeneration'] as num?)?.toInt() ?? _gate.generation;
    // Late, duplicate, or old-window replies are dropped here.
    if (!_gate.settle(requestId, generation)) return null;
    final completer = _pending.remove(requestId);
    _legacyIds.removeWhere((_, value) => value == requestId);
    if (completer != null && !completer.isCompleted) {
      // A reply carrying `status` is authoritative; a legacy reply without
      // it falls back to the `ok` flag (old runners echo only id/ok).
      final status = args.containsKey('status')
          ? windowOutcomeStatusFromName(args['status'])
          : (args['ok'] == true
                ? WindowOutcomeStatus.ok
                : WindowOutcomeStatus.failed);
      completer.complete(
        SettingsEditorOutcome(
          ok: status == WindowOutcomeStatus.ok,
          pendingConfirmation:
              status == WindowOutcomeStatus.pendingConfirmation,
          message: args['message'] as String?,
        ),
      );
    }
    return null;
  }

  /// Prefers the stable `requestId`; falls back to the legacy numeric `id`
  /// for runners that relay old payloads without echoing the envelope.
  String? _resolveRequestId(Map<String, dynamic> args) {
    final direct = args['requestId'] as String?;
    if (direct != null && direct.isNotEmpty) return direct;
    final id = args['id'];
    return _legacyIds[id];
  }

  @visibleForTesting
  Future<void> debugInjectReply(Map<String, dynamic> args) =>
      _handle(MethodCall('saveOutcome', args));

  @visibleForTesting
  int get debugPendingCount => _gate.pendingCount;

  @visibleForTesting
  int get debugGeneration => _gate.generation;
}
