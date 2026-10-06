import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as rust;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as rust;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart' as rust;

/// Structured error surfaced to the status bar. Mirrors `ErrorDto`.
class RuntimeErrorView {
  const RuntimeErrorView({
    required this.code,
    required this.messageKey,
    this.detail,
    this.retryable = false,
    this.operationId,
  });

  final String code;
  final String messageKey;
  final String? detail;

  /// Whether the failed operation may be retried (mirrors `ErrorDto.retryable`).
  final bool retryable;

  /// Backend operation that produced this error, when known (SP-17 failure
  /// identity for view/retry; mirrors `ErrorDto.operation_id`).
  final String? operationId;

  @override
  String toString() =>
      detail == null ? '$code ($messageKey)' : '$code ($messageKey): $detail';
}

/// Live TUN lease facts from the runtime snapshot (never the desired switch).
class RuntimeTunView {
  const RuntimeTunView({
    required this.adapterName,
    required this.interfaceIndex,
    required this.routeCount,
    required this.dryRun,
  });

  final String adapterName;
  final int interfaceIndex;
  final int routeCount;
  final bool dryRun;
}

/// SP-17 actual-runtime descriptor from the snapshot: what is actually
/// running/exited. Every fact is real (frozen target or net-host
/// observation); absent facts stay null/empty.
class RuntimeActualView {
  const RuntimeActualView({
    this.sessionId,
    this.actualGeneration,
    this.operationId,
    this.targetProfileId,
    this.targetCore,
    this.coreVersion,
    this.planHash,
    this.appliedRuntimeRevision,
    this.mainPid,
    this.readyEndpoints = const <int>[],
    this.lastExitPid,
    this.lastExitCode,
    this.lastExitAtMs,
    this.lastExitSidecar,
  });

  final String? sessionId;
  final BigInt? actualGeneration;
  final String? operationId;
  final String? targetProfileId;
  final String? targetCore;
  final String? coreVersion;
  final String? planHash;
  final BigInt? appliedRuntimeRevision;
  final int? mainPid;
  final List<int> readyEndpoints;
  final int? lastExitPid;
  final int? lastExitCode;
  final int? lastExitAtMs;
  final String? lastExitSidecar;

  /// Redacted exit fact label (`pid=.. code=..`), numbers only.
  String get exitFactLabel {
    if (lastExitPid == null) return '';
    final codeText = lastExitCode == null ? '' : ' code=$lastExitCode';
    return 'pid=$lastExitPid$codeText';
  }
}

/// Severity of a status-bar notice (SP-17 message ordering).
enum RuntimeNoticeSeverity { info, warning, error }

/// A timestamped status-bar notice: availability results and other transient
/// feedback. Carries time/severity/operation identity so the status bar can
/// order it against older platform/shell messages instead of letting a stale
/// message cover a newer result.
class RuntimeNotice {
  const RuntimeNotice({
    required this.text,
    required this.severity,
    required this.atMs,
    this.operationId,
  });

  final String text;
  final RuntimeNoticeSeverity severity;
  final int atMs;
  final String? operationId;
}

/// A read model of the runtime, assembled only from snapshot/events.
class RuntimeView {
  const RuntimeView({
    this.state = 'Stopped',
    this.hostAlive = false,
    this.pid,
    this.ports = const <int>[],
    this.sessionId,
    this.configSha256,
    this.operationId,
    this.error,
    this.desiredRevision,
    this.appliedRevision,
    this.tun,
    this.actual,
    this.epoch,
    this.lastSeq,
    this.sequenceWarning,
    this.commandPending = false,
    this.pendingCommands = 0,
    this.staleResponsesDropped = 0,
    this.reconcileNeeded = false,
    this.attemptedTargetId,
    this.failedTargetId,
    this.failureOperationId,
    this.failedAtMs,
    this.notice,
  });

  final String state;
  final bool hostAlive;
  final int? pid;
  final List<int> ports;
  final String? sessionId;
  final String? configSha256;
  final String? operationId;
  final RuntimeErrorView? error;
  final BigInt? desiredRevision;
  final BigInt? appliedRevision;

  /// Live TUN lease facts reported by net-host, or null when the running plan
  /// has no TUN lease. The UI reads this instead of the desired switch
  /// (TUN-A03); it is never synthesized from settings.
  final RuntimeTunView? tun;

  /// SP-17 actual-runtime descriptor, or null when no actual fact exists.
  final RuntimeActualView? actual;

  /// True while a local command (apply/reload/stop) is being submitted but the
  /// backend has not reported its own transition yet. This is a UI command
  /// state, never a substitute for the net-host Running/Starting fact (R4-01).
  final bool commandPending;

  /// Observable command-queue depth (active + queued, coalesced). Bounded by
  /// construction: one active command plus one coalesced apply and one
  /// coalesced stop (R4-04).
  final int pendingCommands;

  /// Count of late snapshot/command responses dropped because a newer command
  /// or session generation had already advanced (R4-04 stale-response eviction).
  final int staleResponsesDropped;

  /// True after a command whose outcome is unknown (timeout/disconnected) was
  /// reconciled from a fresh snapshot instead of being reported as a success or
  /// a definitive failure (R4-04).
  final bool reconcileNeeded;

  /// Explicit target frozen at click time for the in-flight/queued apply
  /// (SP-17); null for the default-path apply. UI-side intent record only,
  /// never a substitute for the backend actual descriptor.
  final String? attemptedTargetId;

  /// Explicit target of the latest failed apply (SP-17 failure identity).
  /// The running view still describes the retained actual session; this field
  /// only tells retry/view-failure where to point. Cleared on success.
  final String? failedTargetId;

  /// Backend operation that produced the current failure, when known.
  final String? failureOperationId;

  /// Wall-clock ms when the current failure was recorded (message ordering).
  final int? failedAtMs;

  /// Latest timestamped notice (availability result, …). Shown by recency,
  /// never covering a current error.
  final RuntimeNotice? notice;

  /// Last event epoch observed by the controller (event stream only; the
  /// snapshot does not carry it). Used to detect a reconnecting/restarted host.
  final BigInt? epoch;

  /// Last event sequence observed on [epoch].
  final BigInt? lastSeq;

  /// Set when an event arrived out of order or with a gap; the UI logs it and
  /// never silently replays. A full reconnect/replay is a T09+ item.
  final String? sequenceWarning;

  /// `desired/applied` revision pair, rendered as `rev: d/a`.
  String get revisionLabel =>
      'rev: ${desiredRevision ?? '-'}/${appliedRevision ?? '-'}';

  /// True when stored state changed after the last apply (T18b): the UI
  /// shows "未应用" and offers an apply entry point instead of pretending
  /// the running core picked the change up.
  bool get hasUnappliedChanges =>
      desiredRevision != null &&
      appliedRevision != null &&
      desiredRevision != appliedRevision;

  bool get isRunning => state == 'Running';

  /// SP-06: the backend reconciled an unsolicited main-core exit (never a
  /// user stop): Stopped with the structured `error.core_exited` cause.
  bool get isCoreExited =>
      !isRunning && error?.messageKey == 'error.core_exited';

  /// SP-06: a sidecar died under a live main core. The session reads
  /// `Degraded` (never a clean Running) while the main endpoint is kept.
  bool get isSidecarDegraded =>
      state == 'Degraded' || error?.messageKey == 'error.sidecar_exited';

  /// SP-06 recovery entry: an exited or degraded runtime can be retried
  /// (`applyActive`/`reload`) or restarted (`stop` then apply). A clean stop
  /// is not an exit and offers no exit recovery.
  bool get canRecover => isCoreExited || isSidecarDegraded;

  /// Actual-exit label for the status surface. The redacted backend detail
  /// (`pid=.. code=..`) is shown verbatim: numbers only, never secrets.
  String get exitStatusLabel {
    if (isCoreExited) {
      final detail = error?.detail;
      return detail == null ? '已退出' : '已退出（$detail）';
    }
    if (isSidecarDegraded) {
      final detail = error?.detail;
      return detail == null ? '已降级' : '已降级（$detail）';
    }
    return statusLabel;
  }

  /// Busy covers both the backend's own transition states and a command this
  /// UI has submitted but whose transition has not arrived yet.
  bool get isBusy =>
      commandPending ||
      state == 'Validating' ||
      state == 'Preparing' ||
      state == 'Starting' ||
      state == 'Checking' ||
      state == 'RollingBack';

  /// The actual local proxy port of an applied session, when known. Never
  /// derived from the desired settings port.
  int? get proxyPort => ports.isEmpty ? null : ports.first;

  /// True only when a session is actually running with a published endpoint:
  /// a desired-but-not-applied plan reports `false`.
  bool get hasAppliedEndpoint =>
      isRunning && ports.isNotEmpty && sessionId != null;

  /// Never fabricates a running label: an actual exit reads as 已退出, a
  /// sidecar failure as 已降级, other unknown or stopped reads as 未运行.
  String get statusLabel {
    if (isRunning) {
      final pidText = pid == null ? '' : ' PID=$pid';
      final portText = ports.isEmpty ? '' : ' 端口=${ports.join(',')}';
      return '运行中$pidText$portText';
    }
    if (isCoreExited || isSidecarDegraded) return exitStatusLabel;
    return '未运行';
  }

  /// SP-17 actual summary for the status bar: built only from actual snapshot
  /// facts (state/session/ports/applied revision/exit). A failed switch keeps
  /// describing the retained actual session; the desired target is never shown
  /// here as if it had applied.
  String get actualSummaryLabel {
    if (isRunning) {
      final shortSession = sessionId == null
          ? ''
          : ' ${sessionId!.length > 12 ? '${sessionId!.substring(0, 12)}…' : sessionId!}';
      final portText = ports.isEmpty ? '' : ' 端口=${ports.join(',')}';
      final pidText = pid == null ? '' : ' PID=$pid';
      return '运行中$shortSession$portText$pidText';
    }
    return exitStatusLabel;
  }

  /// A current structured failure is present (visible, retryable via the
  /// recorded [failedTargetId]).
  bool get hasCurrentFailure => error != null;

  /// True when the current failure remembers its target, so the status bar can
  /// offer view-failure/retry against the failed attempt (never a guess).
  bool get canRetryFailed => error != null && failedTargetId != null;

  RuntimeView copyWith({
    RuntimeErrorView? error,
    bool clearError = false,
    RuntimeTunView? tun,
    bool clearTun = false,
    BigInt? epoch,
    BigInt? lastSeq,
    String? sequenceWarning,
    bool? commandPending,
    int? pendingCommands,
    int? staleResponsesDropped,
    bool? reconcileNeeded,
    String? attemptedTargetId,
    bool clearAttempted = false,
    String? failedTargetId,
    bool clearFailed = false,
    String? failureOperationId,
    int? failedAtMs,
    RuntimeNotice? notice,
    bool clearNotice = false,
  }) {
    return RuntimeView(
      state: state,
      hostAlive: hostAlive,
      pid: pid,
      ports: ports,
      sessionId: sessionId,
      configSha256: configSha256,
      operationId: operationId,
      error: clearError ? null : (error ?? this.error),
      desiredRevision: desiredRevision,
      appliedRevision: appliedRevision,
      tun: clearTun ? null : (tun ?? this.tun),
      actual: actual,
      epoch: epoch ?? this.epoch,
      lastSeq: lastSeq ?? this.lastSeq,
      sequenceWarning: sequenceWarning ?? this.sequenceWarning,
      commandPending: commandPending ?? this.commandPending,
      pendingCommands: pendingCommands ?? this.pendingCommands,
      staleResponsesDropped:
          staleResponsesDropped ?? this.staleResponsesDropped,
      reconcileNeeded: reconcileNeeded ?? this.reconcileNeeded,
      attemptedTargetId: clearAttempted
          ? null
          : (attemptedTargetId ?? this.attemptedTargetId),
      failedTargetId: clearFailed
          ? null
          : (failedTargetId ?? this.failedTargetId),
      failureOperationId: clearFailed
          ? null
          : (failureOperationId ?? this.failureOperationId),
      failedAtMs: clearFailed ? null : (failedAtMs ?? this.failedAtMs),
      notice: clearNotice ? null : (notice ?? this.notice),
    );
  }
}

/// Outcome of an apply/stop command.
class RuntimeActionResult {
  const RuntimeActionResult({required this.ok, this.operationId, this.error});

  final bool ok;
  final String? operationId;
  final RuntimeErrorView? error;
}

/// Read-only status of one prior runtime operation (SP-04 reconcile).
///
/// Returned by [RuntimeBridge.operationStatus]; `null` means the bridge
/// cannot answer (unknown operation or no query capability) and the caller
/// must fall back to the authoritative snapshot. Never throws.
class RuntimeOperationView {
  const RuntimeOperationView({
    required this.found,
    required this.operationId,
    required this.state,
    this.error,
  });

  final bool found;
  final String operationId;

  /// Backend lifecycle label (`running`/`done`/`failed`/`cancelled`, …).
  final String state;
  final RuntimeErrorView? error;
}

/// A raw runtime event (kind + JSON payload + stream position).
class RuntimeEvent {
  const RuntimeEvent({
    required this.kind,
    required this.payloadJson,
    this.epoch,
    this.seq,
  });

  final String kind;
  final String payloadJson;

  /// Stream position; `null` for synthetic test events that bypass the host.
  final BigInt? epoch;
  final BigInt? seq;
}

/// Thin, testable seam over the generated bridge so widget tests can avoid
/// loading the native library.
abstract class RuntimeBridge {
  Future<RuntimeView> snapshot();

  /// The persisted active node id, if any. Used to decide whether a normal
  /// launch has something to restore; never invents an active node.
  String? activeProfileId();

  /// Apply the real persisted plan (active node + settings + routing +
  /// DNS + rule mode) through `apply_runtime` with an empty target id.
  Future<RuntimeActionResult> applyActive({required BigInt expectedRevision});

  Future<RuntimeActionResult> stop();

  Stream<RuntimeEvent> events();
}

/// Optional capability for a bridge that can answer the recorded status of a
/// prior operation (SP-04 unknown-outcome reconcile).
///
/// Kept separate from [RuntimeBridge] so existing test doubles keep
/// compiling; the real FRB bridge implements it from the existing
/// `get_operation` endpoint (no new FRB surface). Callers always fall back
/// to the authoritative snapshot on null.
abstract class OperationQueryBridge {
  Future<RuntimeOperationView?> operationStatus(String operationId);
}

/// Optional capability for a bridge that can apply an explicit frozen target
/// and report the local persisted desired revision (R4-02).
///
/// Kept separate from [RuntimeBridge] so existing test doubles that only model
/// the default command keep compiling; the real FRB bridge and the R4-02 test
/// doubles implement it.
abstract class ExplicitTargetRuntimeBridge {
  Future<RuntimeActionResult> applyTarget({
    required String targetId,
    required BigInt expectedRevision,
  });

  /// Desired revision from local persisted state, used only as a fallback when
  /// the snapshot read itself failed. Never fabricated.
  BigInt desiredRevision();
}

class FrbRuntimeBridge
    implements
        RuntimeBridge,
        ExplicitTargetRuntimeBridge,
        OperationQueryBridge {
  const FrbRuntimeBridge();

  @override
  Future<RuntimeView> snapshot() async {
    final snap = await rust.getSnapshot();
    return _toView(snap);
  }

  @override
  String? activeProfileId() => rust.getActiveProfile();

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async {
    // Empty target id resolves to the persisted active node on the Rust
    // side; a missing node or generator failure returns a structured error
    // (never a hardcoded smoke config).
    return _apply('', expectedRevision);
  }

  @override
  Future<RuntimeActionResult> applyTarget({
    required String targetId,
    required BigInt expectedRevision,
  }) => _apply(targetId, expectedRevision);

  Future<RuntimeActionResult> _apply(
    String targetId,
    BigInt expectedRevision,
  ) async {
    final result = await rust.applyRuntime(
      targetId: targetId,
      expectedRevision: expectedRevision,
    );
    return RuntimeActionResult(
      ok: result.ok,
      operationId: result.operationId,
      error: _error(result.error),
    );
  }

  @override
  BigInt desiredRevision() => rust.profileRevision();

  @override
  Future<RuntimeActionResult> stop() async {
    final result = await rust.stopRuntime();
    return RuntimeActionResult(ok: result.ok, error: _error(result.error));
  }

  /// Serve the SP-04 reconcile query from the existing `get_operation` FRB
  /// endpoint. Never throws: any transport failure reads as "cannot answer".
  @override
  Future<RuntimeOperationView?> operationStatus(String operationId) async {
    try {
      final dto = await rust.getOperation(operationId: operationId);
      if (!dto.found) return null;
      return RuntimeOperationView(
        found: true,
        operationId: dto.operationId,
        state: dto.state.name,
        error: _error(dto.error),
      );
    } on Object catch (_) {
      return null;
    }
  }

  @override
  Stream<RuntimeEvent> events() => rust.subscribeEvents().map(
    (e) => RuntimeEvent(
      kind: e.kind,
      payloadJson: e.payloadJson,
      epoch: e.epoch,
      seq: e.seq,
    ),
  );

  /// Canonical state label matching the Rust `RuntimeState` variant names.
  static String stateLabel(rust.RuntimeState state) {
    switch (state) {
      case rust.RuntimeState.stopped:
        return 'Stopped';
      case rust.RuntimeState.validating:
        return 'Validating';
      case rust.RuntimeState.preparing:
        return 'Preparing';
      case rust.RuntimeState.starting:
        return 'Starting';
      case rust.RuntimeState.checking:
        return 'Checking';
      case rust.RuntimeState.running:
        return 'Running';
      case rust.RuntimeState.rollingBack:
        return 'RollingBack';
      case rust.RuntimeState.degraded:
        return 'Degraded';
    }
  }

  RuntimeView _toView(rust.SnapshotDto snap) {
    return RuntimeView(
      state: stateLabel(snap.runtimeState),
      hostAlive: snap.hostAlive,
      pid: snap.runtimePid,
      ports: snap.runtimePorts.toList(),
      sessionId: snap.runtimeSessionId,
      configSha256: snap.runtimeConfigSha256,
      operationId: snap.runtimeOperationId,
      error: _error(snap.runtimeError),
      desiredRevision: snap.desiredRevision,
      appliedRevision: snap.appliedRevision,
      tun: snap.runtimeTun == null
          ? null
          : RuntimeTunView(
              adapterName: snap.runtimeTun!.adapterName,
              interfaceIndex: snap.runtimeTun!.interfaceIndex,
              routeCount: snap.runtimeTun!.routeCount,
              dryRun: snap.runtimeTun!.dryRun,
            ),
      actual: snap.actual == null
          ? null
          : RuntimeActualView(
              sessionId: snap.actual!.sessionId,
              actualGeneration: snap.actual!.actualGeneration,
              operationId: snap.actual!.operationId,
              targetProfileId: snap.actual!.targetProfileId,
              targetCore: snap.actual!.targetCore,
              coreVersion: snap.actual!.coreVersion,
              planHash: snap.actual!.planHash,
              appliedRuntimeRevision: snap.actual!.appliedRuntimeRevision,
              mainPid: snap.actual!.mainPid,
              readyEndpoints: snap.actual!.readyEndpoints.toList(),
              lastExitPid: snap.actual!.lastExit?.pid,
              lastExitCode: snap.actual!.lastExit?.exitCode,
              lastExitAtMs: snap.actual!.lastExit?.atMs,
              lastExitSidecar: snap.actual!.lastExitSidecar,
            ),
    );
  }

  RuntimeErrorView? _error(rust.ErrorDto? e) {
    if (e == null) return null;
    return RuntimeErrorView(
      code: e.code,
      messageKey: e.messageKey,
      detail: e.detail,
      retryable: e.retryable,
      operationId: e.operationId,
    );
  }
}

final runtimeBridgeProvider = Provider<RuntimeBridge>(
  (ref) => const FrbRuntimeBridge(),
);
