// T15b bridge smoke probe. Guarded by `V2RAYN_T15B_PROBE=1` so the normal
// suite skips it. Starts a local TCP listener, seeds one profile pointing at
// it, runs the real bridge TCPing job and asserts the `ProfileExItem` result
// closure receives a positive delay.
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/speedtest.dart' as speedtest;
import 'package:v2rayn_desktop/bridge/frb_generated.dart';

void main() {
  test('bridge TCPing writes a real ProfileExItem delay', () async {
    if (Platform.environment['V2RAYN_T15B_PROBE'] != '1') {
      markTestSkipped('probe disabled');
      return;
    }
    final dll = Platform.environment['V2RAYN_R_BRIDGE_DLL']!;
    final dataDir = Directory.systemTemp.createTempSync('t15b_probe_');
    final listener = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() {
      listener.close();
      try {
        dataDir.deleteSync(recursive: true);
      } catch (_) {}
    });

    await RustLib.init(externalLibrary: ExternalLibrary.open(dll));
    expect(engine.initEngine(dataDir: dataDir.path).ok, isTrue);

    final draft = c.ProfileDto(
      indexId: '',
      configType: ConfigType.vless,
      coreType: CoreType.xray,
      configVersion: 4,
      subid: '',
      isSub: false,
      displayLog: true,
      remarks: 'Probe',
      address: '127.0.0.1',
      port: listener.port,
      password: '11111111-2222-3333-4444-555555555555',
      username: '',
      network: 'raw',
      security: const c.SecurityDto(streamSecurity: null),
      protoExtra: const c.ProtocolExtraDto(
        vlessEncryption: 'none',
        extraJson: '{}',
      ),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );
    final saved = engine.saveProfile(
      draft: draft,
      expectedRevision: engine.profileRevision(),
    );
    expect(saved.ok, isTrue, reason: saved.error?.messageKey);

    speedtest.speedtestConfigure(
      pageSize: 1000,
      mixedConcurrency: 10,
      timeoutSecs: 10,
      speedTestUrl: 'http://127.0.0.1/',
      speedPingTestUrl: 'http://127.0.0.1/',
      ipapiUrl: null,
      udpTestTarget: null,
      delayIntervalSecs: 0,
    );
    final start = speedtest.speedtestStart(
      kind: 0,
      indexIds: <String>[saved.profile!.indexId],
    );
    expect(start.ok, isTrue);
    expect(start.total, 1);

    // The job runs on a worker thread; poll the closure until it settles.
    var rows = <speedtest.SpeedTestResultDto>[];
    for (var i = 0; i < 40; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 100));
      rows = speedtest.speedtestResults();
      if (rows.any((r) => r.delay > 0) &&
          speedtest.speedtestActiveJobs() == 0) {
        break;
      }
    }
    expect(rows, isNotEmpty, reason: 'no ProfileExItem result written');
    expect(rows.first.delay, greaterThan(0));
    expect(speedtest.speedtestActiveJobs(), 0);
  });
}
