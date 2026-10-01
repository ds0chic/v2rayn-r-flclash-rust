// T06a: exercise the real flutter_rust_bridge + SQLite path (no mocks).
//
// Loads the freshly built `bridge_api.dll`, points the engine at a temporary
// data directory, saves a synthesized VLESS node and reads it back. When the
// native library is not built the test is skipped with a clear reason rather
// than silently passing a mock.
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
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
  final env = Platform.environment['V2RAYN_R_BRIDGE_DLL'];
  if (env != null && File(env).existsSync()) return env;
  return null;
}

void main() {
  test('real FRB save/read round-trip persists to SQLite', () async {
    final libraryPath = _findLibrary();
    if (libraryPath == null) {
      markTestSkipped(
        'bridge_api.dll not built; run `cargo build -p bridge_api`',
      );
      return;
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));

    final dir = Directory.systemTemp.createTempSync('t06a_frb_');
    addTearDown(() {
      try {
        dir.deleteSync(recursive: true);
      } catch (_) {}
    });
    final init = engine.initEngine(dataDir: dir.path);
    expect(init.ok, isTrue);

    final draft = c.ProfileDto(
      indexId: '',
      configType: ConfigType.vless,
      coreType: CoreType.singBox,
      configVersion: 4,
      subid: '',
      isSub: true,
      displayLog: true,
      remarks: 'T06a 真实桥接 🚀',
      address: '192.0.2.77',
      port: 443,
      password: '11111111-2222-3333-4444-555555555555',
      username: '',
      network: 'ws',
      security: const c.SecurityDto(
        streamSecurity: 'tls',
        sni: 'sni.example.com',
        alpn: 'h2,http/1.1',
      ),
      protoExtra: const c.ProtocolExtraDto(
        flow: 'xtls-rprx-vision',
        vlessEncryption: 'none',
        extraJson: '{}',
      ),
      transportExtra: const c.TransportExtraDto(
        host: 'example.com',
        path: '/t06a',
        extraJson: '{}',
      ),
      extraJson: '{}',
    );

    final revision = engine.profileRevision();
    final saved = engine.saveProfile(draft: draft, expectedRevision: revision);
    expect(saved.ok, isTrue, reason: saved.error?.messageKey);
    final savedId = saved.profile!.indexId;
    expect(savedId, isNotEmpty);

    final loaded = engine.getProfile(indexId: savedId);
    expect(loaded, isNotNull);
    expect(loaded!.remarks, 'T06a 真实桥接 🚀');
    expect(loaded.network, 'ws');
    expect(loaded.security.sni, 'sni.example.com');
    expect(loaded.protoExtra.flow, 'xtls-rprx-vision');
    expect(loaded.transportExtra.path, '/t06a');
    expect(loaded.coreType, CoreType.singBox);

    // Reusing the stale revision is a structured conflict.
    final stale = engine.saveProfile(draft: draft, expectedRevision: revision);
    expect(stale.ok, isFalse);
    expect(stale.error!.code, 'E_REVISION_STALE');

    // The row is visible to the paged query used by the table.
    final page = engine.queryProfiles(
      filter: const c.ProfileFilterDto(
        text: null,
        configTypes: <ConfigType>[],
        subid: null,
      ),
      sort: c.ProfileSortDto.indexId,
      cursor: BigInt.zero,
      pageSize: 100,
    );
    expect(page.items.any((p) => p.indexId == savedId), isTrue);
  });
}
