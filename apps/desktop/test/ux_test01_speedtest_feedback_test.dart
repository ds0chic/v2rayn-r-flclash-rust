// UX-TEST-01: the speedtest action must always leave visible, explainable
// feedback (start failure / empty node set / all-failed / success). These are
// controller-level tests against the synthetic bridge; the real-window path is
// covered by integration_test/ux_speedtest_diag_test.dart.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';

class _FailStartBridge extends SyntheticBridgePort {
  _FailStartBridge() : super(count: 10);

  @override
  speedtest.SpeedTestStartDto startSpeedTest(int kind, List<String> indexIds) =>
      const speedtest.SpeedTestStartDto(
        ok: false,
        jobId: null,
        total: 0,
        error: c.ErrorDto(
          code: 'E_TEST_START',
          messageKey: 'error.test_start_failed',
          retryable: false,
        ),
      );
}

ProviderContainer _container(BridgePort bridge, {int rows = 10}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(rows),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('start failure leaves a structured, visible message', () {
    final container = _container(_FailStartBridge());
    final controller = container.read(profilesControllerProvider.notifier);
    controller.selectRow('syn-000001');
    final result = controller.startSpeedTest(ProfileAction.tcping);
    expect(result.ok, isFalse);
    final state = container.read(profilesControllerProvider);
    expect(state.speedTestRunning, isFalse);
    expect(state.speedTestStage, 'SpeedtestingFailed');
    expect(state.speedTestMessage, contains('E_TEST_START'));
    expect(state.speedTestMessage, contains('error.test_start_failed'));
  });

  test('zero stored nodes are reported instead of a silent no-op', () {
    final container = _container(SyntheticBridgePort(count: 0), rows: 0);
    final controller = container.read(profilesControllerProvider.notifier);
    controller.startSpeedTest(ProfileAction.tcping);
    final state = container.read(profilesControllerProvider);
    expect(state.speedTestMessage, '没有可测试节点');
    expect(state.speedTestRunning, isFalse);
  });

  test('all-failed run summarizes the failure reason', () async {
    final bridge = SyntheticBridgePort(count: 10);
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    const id = 'syn-000001';
    controller.selectRow(id);
    bridge.seedSpeedResult(id, -1, 0, message: 'error.port_conflict');
    controller.startSpeedTest(ProfileAction.tcping);
    // SyntheticBridgePort reports zero active jobs, so the 150 ms poll settles.
    await Future<void>.delayed(const Duration(milliseconds: 300));
    final message = container.read(profilesControllerProvider).speedTestMessage;
    expect(message, contains('测速完成'));
    expect(message, contains('失败 1'));
    // UX-TEST-02 maps the stable error key to a human reason for the summary.
    expect(message, contains('测试端口冲突'));
  });

  test(
    'structured TLS failure keys are summarized as a human reason',
    () async {
      final bridge = SyntheticBridgePort(count: 10);
      final container = _container(bridge);
      final controller = container.read(profilesControllerProvider.notifier);
      const id = 'syn-000003';
      controller.selectRow(id);
      bridge.seedSpeedResult(id, -1, 0, message: 'speedtest.tls_failed');
      controller.startSpeedTest(ProfileAction.realping);
      await Future<void>.delayed(const Duration(milliseconds: 300));
      final message = container
          .read(profilesControllerProvider)
          .speedTestMessage;
      expect(message, contains('测速完成'));
      expect(message, contains('TLS 证书校验失败'));
    },
  );

  test('successful run summarizes success', () async {
    final bridge = SyntheticBridgePort(count: 10);
    final container = _container(bridge);
    final controller = container.read(profilesControllerProvider.notifier);
    const id = 'syn-000002';
    controller.selectRow(id);
    bridge.seedSpeedResult(id, 42, 3.2);
    controller.startSpeedTest(ProfileAction.tcping);
    await Future<void>.delayed(const Duration(milliseconds: 300));
    final message = container.read(profilesControllerProvider).speedTestMessage;
    expect(message, contains('成功 1'));
  });

  test('tested-and-failed renders as 失败, never-tested as -', () {
    final columns = defaultProfileColumns();
    final delay = columns.firstWhere((c) => c.key == 'DelayVal');
    expect(delay.display(_row(delay: -2)), '失败');
    expect(delay.display(_row(delay: -1)), '-');
    expect(delay.display(_row(delay: 0)), '0 ms');
    expect(delay.display(_row(delay: 12)), '12 ms');
  });
}

ProfileSummary _row({required int delay}) => ProfileSummary(
  id: 'x',
  configType: ConfigType.vless,
  remarks: 'x',
  address: '127.0.0.1',
  port: 443,
  network: 'raw',
  streamSecurity: 'none',
  subRemarks: '',
  delay: delay,
  speed: '-',
  todayUp: BigInt.zero,
  ipInfo: '',
  todayDown: BigInt.zero,
  totalUp: BigInt.zero,
  totalDown: BigInt.zero,
  coreType: CoreType.xray,
);
