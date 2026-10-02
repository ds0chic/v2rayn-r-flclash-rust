// T21-E: probe how the real Rust parser handles common real-world share-link
// forms (hysteria2 params, IPv6 authority, emoji remarks, CRLF, uppercase
// scheme). Failures are recorded in docs/evidence/T21-E.md; the Dart side only
// classifies the error.
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/subs.dart' as subs;
import 'package:v2rayn_desktop/bridge/frb_generated.dart';

String? _findLibrary() {
  final candidates = <String>[
    '../../target/debug/bridge_api.dll',
    '../../target/release/bridge_api.dll',
    'bridge_api.dll',
  ];
  for (final path in candidates) {
    final file = File(path);
    if (file.existsSync()) return file.absolute.path;
  }
  return null;
}

void main() {
  late Directory dir;

  setUpAll(() async {
    final libraryPath = _findLibrary();
    if (libraryPath == null) {
      markTestSkipped('bridge_api.dll not built');
      return;
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
    dir = Directory.systemTemp.createTempSync('t21e_forms_');
    engine.initEngine(dataDir: dir.path);
  });

  tearDownAll(() {
    try {
      if (dir.existsSync()) dir.deleteSync(recursive: true);
    } catch (_) {
      // The process-global engine still holds the SQLite handle; the OS cleans
      // the temp directory up.
    }
  });

  test('hysteria2 with params parses', () {
    final result = subs.parseShareUri(
      line: 'hysteria2://secret@h.example:443/?sni=x.example&insecure=1#hy2',
    );
    expect(result.ok, isTrue, reason: result.error?.code);
    expect(result.profile!.configType.name.toLowerCase(), contains('hyst'));
  });

  test('IPv6 authority parses', () {
    final result = subs.parseShareUri(
      line: 'vless://11111111-1111-1111-1111-111111111111@[2001:db8::1]:443?encryption=none&type=tcp#ipv6',
    );
    expect(result.ok, isTrue, reason: result.error?.code);
    expect(result.profile!.address, contains('2001:db8'));
  });

  test('emoji remarks survive', () {
    final result = subs.parseShareUri(
      line: 'vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#emoji%20%F0%9F%9A%80',
    );
    expect(result.ok, isTrue, reason: result.error?.code);
    expect(result.profile!.remarks, contains('\u{1F680}'));
  });

  test('CRLF-separated list parses', () async {
    const text =
        'vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one\r\n'
        'trojan://pw@b.example:443#two\r\n';
    final result = await subs.importFromText(text: text, deduplicate: true);
    expect(result.ok, isTrue);
    expect(result.imported, 2);
  });

  test('uppercase scheme probe (recorded, no parity claim)', () {
    final result = subs.parseShareUri(
      line: 'VLESS://11111111-1111-1111-1111-111111111111@a.example:443#upper',
    );
    // Recorded for T21-E.md: upstream schemes are lowercase; an uppercase
    // scheme should surface a located parse error instead of being dropped.
    // ignore: avoid_print
    print('uppercase scheme ok=${result.ok} code=${result.error?.code}');
  });
}
