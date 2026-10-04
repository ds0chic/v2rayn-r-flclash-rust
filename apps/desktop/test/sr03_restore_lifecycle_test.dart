import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

// SR-03: a restore/import swaps the database and config. The restore lifecycle
// must stop the old session/timer, reload every long-lived provider, then
// re-apply the restored active node and restart the scheduler; a failure must
// never claim the configuration was reloaded.

class _LifecycleBridge extends SyntheticBridgePort {
  bool schedulerRunning = true;
  int startCalls = 0;
  int stopCalls = 0;
  int profileReloads = 0;
  int subsReloads = 0;
  int settingsLoads = 0;

  @override
  bool subSchedulerRunning() => schedulerRunning;

  @override
  c.SimpleResult startSubScheduler() {
    startCalls += 1;
    schedulerRunning = true;
    return const c.SimpleResult(ok: true);
  }

  @override
  c.SimpleResult stopSubScheduler() {
    stopCalls += 1;
    schedulerRunning = false;
    return const c.SimpleResult(ok: true);
  }

  @override
  c.RestoreResultDto t16BackupRestore(String bundleDir) {
    // Simulate the Rust lifecycle stopping the scheduler before the swap.
    schedulerRunning = false;
    return super.t16BackupRestore(bundleDir);
  }

  @override
  c.ImportSummaryDto t16BackupImportUpstream(String path) {
    schedulerRunning = false;
    return super.t16BackupImportUpstream(path);
  }

  @override
  List<c.ProfileDto> queryAllProfiles() {
    profileReloads += 1;
    return super.queryAllProfiles();
  }

  @override
  c.SubsPageDto listSubItems() {
    subsReloads += 1;
    return super.listSubItems();
  }

  @override
  settings.SettingsLoadDto getSettings() {
    settingsLoads += 1;
    return super.getSettings();
  }
}

class _RecordingRuntimeBridge implements RuntimeBridge {
  _RecordingRuntimeBridge({this.active = 'restored-active'});

  final String? active;
  int stops = 0;
  int applies = 0;
  RuntimeView _view = const RuntimeView();

  @override
  String? activeProfileId() => active;

  @override
  Future<RuntimeView> snapshot() async => _view;

  @override
  Future<RuntimeActionResult> stop() async {
    stops += 1;
    _view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async {
    applies += 1;
    _view = RuntimeView(
      state: 'Running',
      hostAlive: true,
      pid: 4242,
      ports: const <int>[11808],
      desiredRevision: expectedRevision,
      appliedRevision: expectedRevision,
    );
    return const RuntimeActionResult(ok: true, operationId: 'op-sr03');
  }

  @override
  Stream<RuntimeEvent> events() => const Stream<RuntimeEvent>.empty();
}

ProviderContainer _container(_LifecycleBridge bridge, RuntimeBridge runtime) =>
    ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        runtimeBridgeProvider.overrideWithValue(runtime),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      ],
    );

void main() {
  test('successful restore refreshes providers, restarts scheduler and applies active', () async {
    final bridge = _LifecycleBridge();
    final runtime = _RecordingRuntimeBridge();
    final container = _container(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(backupControllerProvider.notifier);
    await controller.restoreBundle('/tmp/good-bundle');

    expect(controller.state.status?.isSuccess, isTrue);
    expect(controller.state.status?.message, contains('已重载'));
    // Every long-lived provider was reloaded from the new storage.
    expect(bridge.settingsLoads, greaterThan(0));
    expect(bridge.profileReloads, greaterThan(0));
    expect(bridge.subsReloads, greaterThan(0));
    // The scheduler that the engine stopped is started again.
    expect(bridge.startCalls, greaterThan(0));
    expect(bridge.schedulerRunning, isTrue);
    // The old session is dropped and the restored active node re-applied.
    expect(runtime.stops, greaterThan(0));
    expect(runtime.applies, 1);
  });

  test('failed restore reports the error and never claims a reload', () async {
    final bridge = _LifecycleBridge();
    final runtime = _RecordingRuntimeBridge();
    final container = _container(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(backupControllerProvider.notifier);
    await controller.restoreBundle('/tmp/broken');

    expect(controller.state.status?.isError, isTrue);
    expect(controller.state.status?.message, isNot(contains('已重载')));
    // A session stopped for the attempt is restored even though the swap failed.
    expect(runtime.stops, greaterThan(0));
    expect(runtime.applies, 1);
  });

  test('successful upstream import goes through the same lifecycle', () async {
    final bridge = _LifecycleBridge();
    final runtime = _RecordingRuntimeBridge();
    final container = _container(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(backupControllerProvider.notifier);
    await controller.importUpstream('/tmp/upstream.zip');

    expect(controller.state.status?.isSuccess, isTrue);
    expect(bridge.profileReloads, greaterThan(0));
    expect(bridge.subsReloads, greaterThan(0));
    expect(runtime.applies, 1);
  });

  test(
    'restore with no active node stays stopped and never invents a session',
    () async {
      final bridge = _LifecycleBridge();
      final runtime = _RecordingRuntimeBridge(active: null);
      final container = _container(bridge, runtime);
      addTearDown(container.dispose);

      final controller = container.read(backupControllerProvider.notifier);
      await controller.restoreBundle('/tmp/good-bundle');

      expect(controller.state.status?.isSuccess, isTrue);
      expect(runtime.stops, greaterThan(0));
      expect(runtime.applies, 0);
    },
  );
}
