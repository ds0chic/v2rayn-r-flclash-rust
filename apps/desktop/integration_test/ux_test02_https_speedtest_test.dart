// UX-TEST-02: real-window proof that the default HTTPS speedtest path works
// end-to-end through a temporary core.
//
// Topology (all loopback, no 10808):
//   temp Xray core (socks inbound) -> socks outbound node -> local SOCKS5 relay
//     -> local HTTPS server (leaf signed by the repo test CA)
// The repo CA is handed to the Rust probe via V2RAYN_SPEEDTEST_EXTRA_CA, so
// certificate verification stays enabled (an extra root is trusted, never an
// insecure mode).
//
//   $env:V2RAYN_R_DATA_DIR = <temp>
//   $env:V2RAYN_UX_TEST02_EVIDENCE_DIR = <repo>\docs\evidence\UX-TEST-02
//   flutter test integration_test/ux_test02_https_speedtest_test.dart -d windows
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

const _frameKey = ValueKey('ux-test02-frame');
const _portFloor = 11808;
const _portCeil = 11890;

Future<T> _until<T>(
  WidgetTester tester,
  T? Function() probe, {
  Duration timeout = const Duration(seconds: 40),
  String what = 'condition',
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    final value = probe();
    if (value != null) return value;
    await tester.pump(const Duration(milliseconds: 150));
  }
  throw TestFailure('timed out waiting for $what');
}

Future<ServerSocket> _bindTcp() async {
  for (var port = _portFloor; port <= _portCeil; port++) {
    try {
      return await ServerSocket.bind(InternetAddress.loopbackIPv4, port);
    } on SocketException {
      continue;
    }
  }
  throw StateError('no free TCP port in $_portFloor..$_portCeil');
}

/// Minimal SOCKS5 no-auth forwarder used as the "node" the temporary core
/// dials. It parses the handshake on a single socket subscription, then pipes
/// bytes both ways (the same subscription stays live, so no payload is lost).
class _SocksForwarder {
  final ServerSocket server;
  final List<Socket> _clients = <Socket>[];

  _SocksForwarder._(this.server) {
    server.listen((socket) {
      _clients.add(socket);
      _SocksRelay(socket).start();
    });
  }

  static Future<_SocksForwarder> start() async {
    final server = await _bindTcp();
    return _SocksForwarder._(server);
  }

  Future<void> close() async {
    for (final socket in _clients) {
      socket.destroy();
    }
    await server.close();
  }
}

class _SocksRelay {
  _SocksRelay(this.client);

  final Socket client;
  final List<int> _buf = <int>[];
  late final StreamSubscription<Uint8List> _sub;
  Socket? _upstream;
  bool _done = false;
  int _phase = 0;
  int _methodBytes = 0;
  int _addrType = 0;
  int _addrNeed = 0;
  String _host = '';
  final List<int> _port = <int>[];

  void start() {
    _sub = client.listen(_onData, onError: (_) => _destroy(), onDone: _destroy);
  }

  void _onData(Uint8List data) {
    if (_done) return;
    if (_phase == 9) {
      _upstream?.add(data);
      return;
    }
    _buf.addAll(data);
    _pump();
  }

  void _pump() {
    while (!_done) {
      switch (_phase) {
        case 0:
          if (_buf.length < 2) return;
          if (_buf[0] != 0x05) {
            _destroy();
            return;
          }
          _methodBytes = _buf[1];
          _buf.removeRange(0, 2);
          _phase = 1;
        case 1:
          if (_buf.length < _methodBytes) return;
          _buf.removeRange(0, _methodBytes);
          client.add(<int>[0x05, 0x00]);
          _phase = 2;
        case 2:
          if (_buf.length < 4) return;
          if (_buf[1] != 0x01) {
            _destroy();
            return;
          }
          _addrType = _buf[3];
          _buf.removeRange(0, 4);
          _phase = 3;
          _addrNeed = switch (_addrType) {
            0x01 => 4,
            0x04 => 16,
            0x03 => 0, // length byte first
            _ => -1,
          };
          if (_addrNeed < 0) {
            _destroy();
            return;
          }
        case 3:
          if (_addrType == 0x03) {
            if (_buf.isEmpty) return;
            _addrNeed = _buf[0];
            _buf.removeAt(0);
            _phase = 4;
          } else {
            _phase = 4;
          }
        case 4:
          if (_buf.length < _addrNeed) return;
          final addr = _buf.sublist(0, _addrNeed);
          _buf.removeRange(0, _addrNeed);
          _host = _addrType == 0x01
              ? '${addr[0]}.${addr[1]}.${addr[2]}.${addr[3]}'
              : String.fromCharCodes(addr);
          _phase = 5;
        case 5:
          if (_buf.length < 2) return;
          _port
            ..add(_buf[0])
            ..add(_buf[1]);
          _buf.removeRange(0, 2);
          _phase = 6;
        case 6:
          _connect();
          return;
        default:
          return;
      }
    }
  }

  Future<void> _connect() async {
    final port = (_port[0] << 8) | _port[1];
    try {
      final upstream = await Socket.connect(_host, port);
      if (_done) {
        upstream.destroy();
        return;
      }
      _upstream = upstream;
      client.add(<int>[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]);
      // Flush any payload buffered after the request, then stream both ways.
      if (_buf.isNotEmpty) {
        upstream.add(List<int>.from(_buf));
        _buf.clear();
      }
      _phase = 9;
      upstream.listen(client.add, onDone: _finish, onError: (_) => _destroy());
    } catch (_) {
      client.add(<int>[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0]);
      _destroy();
    }
  }

  /// Upstream closed: flush everything already forwarded, then half-close the
  /// client so a TLS reader sees a clean end instead of an abrupt reset.
  void _finish() {
    if (_done) return;
    _done = true;
    _sub.cancel();
    _upstream?.destroy();
    client.flush().then((_) => client.close()).catchError((_) {
      client.destroy();
    });
  }

  void _destroy() {
    if (_done) return;
    _done = true;
    _upstream?.destroy();
    client.destroy();
    _sub.cancel();
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('UX-TEST-02 HTTPS real ping and download', (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir =
        Platform.environment['V2RAYN_UX_TEST02_EVIDENCE_DIR'] ??
        '${Directory.current.path}${Platform.pathSeparator}'
            'docs${Platform.pathSeparator}evidence${Platform.pathSeparator}'
            'UX-TEST-02';
    expect(dataDir, isNotNull, reason: 'V2RAYN_R_DATA_DIR must be set');
    await Directory(evidenceDir).create(recursive: true);

    final observations = <Map<String, Object?>>[];
    void record(String step, Map<String, Object?> actual) {
      observations.add(<String, Object?>{'step': step, 'actual': actual});
      File('$evidenceDir/integration-https.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert(<String, Object?>{
          'dataDir': dataDir,
          'observations': observations,
        }),
      );
    }

    // --- Repo test CA + leaf cert for the local HTTPS target ---------------
    final fixtures = Directory('../../crates/application/tests/fixtures/tls')
        .absolute;
    final caPath = '${fixtures.path}${Platform.pathSeparator}ca.pem';
    final serverPath = '${fixtures.path}${Platform.pathSeparator}server.pem';
    final keyPath = '${fixtures.path}${Platform.pathSeparator}server.key.pem';
    expect(File(caPath).existsSync(), isTrue, reason: 'test CA missing');
    // The synthetic CA must already be exported by the harness as
    // V2RAYN_SPEEDTEST_EXTRA_CA (Platform.environment is read-only). The Rust
    // probe loads it as an *additional* root; verification stays enabled.
    expect(
      Platform.environment['V2RAYN_SPEEDTEST_EXTRA_CA'],
      isNotNull,
      reason:
          'set V2RAYN_SPEEDTEST_EXTRA_CA to the repo test CA before running',
    );

    final https = await HttpServer.bindSecure(
      InternetAddress.loopbackIPv4,
      0,
      SecurityContext()
        ..useCertificateChain(serverPath)
        ..usePrivateKey(keyPath),
    );
    https.listen((request) {
      if (request.uri.path == '/ping') {
        request.response.statusCode = HttpStatus.noContent;
        request.response.close();
        return;
      }
      request.response.headers.contentType = ContentType.binary;
      request.response.add(List<int>.filled(512 * 1024, 0x42));
      request.response.close();
    });
    addTearDown(() => https.close(force: true));

    final socks = await _SocksForwarder.start();
    addTearDown(socks.close);

    record('servers', <String, Object?>{
      'httpsPort': https.port,
      'socksPort': socks.server.port,
    });

    RustBridgeInit.configure(RustLib.init);
    await RustBridgeInit.init();
    runApp(
      ProviderScope(
        overrides: [
          uiStateStoreProvider.overrideWithValue(
            FileUiStateStore(overridePath: '$dataDir/ui_state.json'),
          ),
        ],
        child: const RepaintBoundary(key: _frameKey, child: V2rayNRApp()),
      ),
    );
    await _until(
      tester,
      () => find.byType(V2rayNRApp).evaluate().isNotEmpty ? true : null,
      what: 'profile toolbar',
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    final bridge = container.read(bridgePortProvider);
    final controller = container.read(profilesControllerProvider.notifier);

    Future<void> screenshot(String name) async {
      await tester.pump(const Duration(milliseconds: 200));
      final boundary = tester.renderObject<RenderRepaintBoundary>(
        find.byKey(_frameKey),
      );
      final image = await boundary.toImage(pixelRatio: 1);
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      if (bytes != null) {
        await File('$evidenceDir/$name.png')
            .writeAsBytes(bytes.buffer.asUint8List());
      }
      image.dispose();
    }

    // --- Seed one local SOCKS node, then point speedtest URLs at HTTPS ----
    final share = 'socks://127.0.0.1:${socks.server.port}#UX-T2';
    final import = await bridge.importFromText(
      share,
      subid: 'ux-test-02',
      deduplicate: false,
    );
    controller.reload();
    final seeded = container.read(profilesControllerProvider).visible;
    expect(import.ok, isTrue, reason: import.error?.messageKey);

    final load = bridge.getSettings();
    final doc = jsonDecode(load.settingsJson) as Map<String, dynamic>;
    final item = (doc['SpeedTestItem'] as Map<String, dynamic>?) ?? {};
    item['SpeedPingTestUrl'] = 'https://127.0.0.1:${https.port}/ping';
    item['SpeedTestUrl'] = 'https://127.0.0.1:${https.port}/speed';
    item['MixedConcurrencyCount'] = 1;
    item['SpeedTestTimeout'] = 15;
    item['SpeedTestDelayInterval'] = 0;
    doc['SpeedTestItem'] = item;
    final saved = bridge.saveSettingsJson(
      jsonEncode(doc),
      load.revision.toInt(),
    );
    record('seed', <String, Object?>{
      'imported': import.imported,
      'rows': seeded.length,
      'saved': saved.ok,
    });
    expect(saved.ok, isTrue);
    expect(seeded, isNotEmpty);
    final targetId = seeded.first.id;
    controller.selectRow(targetId);

    Future<void> runKind(String name, int kind) async {
      final started = DateTime.now();
      controller.emitAction(kind == 1 ? 'realping' : 'speedtest');
      final deadline = DateTime.now().add(const Duration(seconds: 40));
      var cancelled = false;
      while (DateTime.now().isBefore(deadline)) {
        await tester.pump(const Duration(milliseconds: 200));
        if (!container.read(profilesControllerProvider).speedTestRunning &&
            bridge.speedTestActiveJobs() == 0) {
          break;
        }
      }
      if (container.read(profilesControllerProvider).speedTestRunning) {
        controller.cancelSpeedTest();
        cancelled = true;
        await tester.pump(const Duration(milliseconds: 400));
      }
      final state = container.read(profilesControllerProvider);
      final row = state.all.firstWhere((r) => r.id == targetId);
      final result = bridge
          .speedTestResults()
          .where((r) => r.indexId == targetId)
          .toList();
      record(name, <String, Object?>{
        'elapsedMs': DateTime.now().difference(started).inMilliseconds,
        'cancelled': cancelled,
        'stage': state.speedTestStage,
        'message': state.speedTestMessage,
        'delaySentinel': row.delay,
        'speed': row.speed,
        'resultDelay': result.isEmpty ? null : result.first.delay,
        'resultSpeed': result.isEmpty ? null : result.first.speed,
        'resultMessage': result.isEmpty ? null : result.first.message,
      });
      await screenshot(name);
    }

    await runKind('https-realping', 1);
    final afterPing = container.read(profilesControllerProvider);
    final pingRow = afterPing.all.firstWhere((r) => r.id == targetId);
    expect(
      pingRow.delay,
      greaterThan(0),
      reason: 'HTTPS real ping must produce a real delay',
    );

    await runKind('https-download', 3);
    final afterDownload = container.read(profilesControllerProvider);
    final downloadResult = bridge
        .speedTestResults()
        .where((r) => r.indexId == targetId)
        .toList();
    expect(downloadResult, isNotEmpty);
    expect(
      downloadResult.first.speed,
      greaterThan(0),
      reason: 'HTTPS download must produce a real speed',
    );
    expect(afterDownload.speedTestMessage, contains('测速完成'));
  });
}
