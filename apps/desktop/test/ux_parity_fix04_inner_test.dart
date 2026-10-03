// FIX-04: real-bridge regression for the frozen `v2rayn://` inner URI loop.
// Loads the built `bridge_api.dll`, points the engine at a temporary data
// directory and walks the full UI chain: parse frozen PascalCase URI ->
// UI-side persist -> table query -> export as inner URI -> re-import.
// Synthetic payloads only (example.invalid, synthetic UUIDs); no network,
// no listeners, no user data.
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/subs.dart' as subs;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';

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
  final env = Platform.environment['V2RAYN_R_BRIDGE_DLL'];
  if (env != null && File(env).existsSync()) return env;
  return null;
}

String _innerUri(String token, Map<String, Object?> payload) {
  final encoded = base64Url
      .encode(utf8.encode(jsonEncode(payload)))
      .replaceAll('=', '');
  return 'v2rayn://$token/$encoded';
}

Map<String, dynamic> _decodeInnerPayload(String uri) {
  var segment = uri.trim().split('/').last;
  while (segment.length % 4 != 0) {
    segment += '=';
  }
  return jsonDecode(utf8.decode(base64Url.decode(segment)))
      as Map<String, dynamic>;
}

c.ProfilePageDto _page() => engine.queryProfiles(
  filter: const c.ProfileFilterDto(
    text: null,
    configTypes: <ConfigType>[],
    subid: null,
  ),
  sort: c.ProfileSortDto.indexId,
  cursor: BigInt.zero,
  pageSize: 100000,
);

void main() {
  test('frozen inner URI imports, persists, exports and re-imports', () async {
    final libraryPath = _findLibrary();
    if (libraryPath == null) {
      markTestSkipped(
        'bridge_api.dll not built; run `cargo build -p bridge_api`',
      );
      return;
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
    final dir = Directory.systemTemp.createTempSync('fix04_inner_');
    addTearDown(() {
      try {
        dir.deleteSync(recursive: true);
      } catch (_) {}
    });
    expect(engine.initEngine(dataDir: dir.path).ok, isTrue);

    final uri = _innerUri('vless', <String, Object?>{
      'IndexId': 'fix04-flutter-1',
      'ConfigType': 5,
      'CoreType': 2,
      'ConfigVersion': 4,
      'Remarks': '合成内部链接',
      'Address': 'node.example.invalid',
      'Port': 11985,
      'Password': '11111111-2222-3333-4444-555555555555',
      'Network': 'ws',
      'StreamSecurity': 'tls',
      'Sni': 'tls.example.invalid',
      'ProtoExtraObj': {'VlessEncryption': 'none'},
      'TransportExtraObj': {'Path': '/synthetic'},
    });

    final result = await subs.importFromText(text: uri, deduplicate: true);
    expect(result.ok, isTrue);
    expect(result.imported, 1);
    expect(result.profiles.single.configType, ConfigType.vless);
    expect(result.profiles.single.remarks, '合成内部链接');

    final before = _page().total.toInt();
    final persisted = persistImportedProfiles(
      const FrbBridgePort(),
      result.profiles,
    );
    expect(persisted.saved, 1, reason: persisted.firstErrorCode);
    expect(_page().total.toInt() - before, 1);

    final id = result.profiles.single.indexId;
    final exported = await subs.exportProfiles(
      ids: <String>[id],
      kind: 'inner',
    );
    expect(exported.ok, isTrue);
    expect(exported.text, startsWith('v2rayn://vless/'));
    final wire = _decodeInnerPayload(exported.text);
    expect(wire['ConfigType'], 5);
    expect(wire['ConfigVersion'], 4);
    expect(wire['Remarks'], '合成内部链接');
    expect(wire['Address'], 'node.example.invalid');
    expect(wire['ProtoExtraObj']['VlessEncryption'], 'none');
    expect(wire.containsKey('Subid'), isFalse);

    final roundtrip = await subs.importFromText(
      text: exported.text,
      deduplicate: false,
    );
    expect(roundtrip.ok, isTrue);
    expect(roundtrip.profiles.single.configType, ConfigType.vless);
    expect(roundtrip.profiles.single.remarks, '合成内部链接');

    // Reopen the same data directory: the imported node must survive.
    expect(engine.initEngine(dataDir: dir.path).ok, isTrue);
    final names = _page().items.map((p) => p.remarks).toList();
    expect(names, contains('合成内部链接'));
  });
}
