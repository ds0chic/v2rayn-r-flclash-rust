// T21-E: real flutter_rust_bridge + SQLite regression for the clipboard import
// link. Loads the built `bridge_api.dll`, points the engine at a temporary data
// directory and asserts that a `subid`-less import becomes visible to the paged
// query used by the node table after the UI-side persistence fix.
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
  test('subid-less import reaches the table after UI-side persistence', () async {
    final libraryPath = _findLibrary();
    if (libraryPath == null) {
      markTestSkipped(
        'bridge_api.dll not built; run `cargo build -p bridge_api`',
      );
      return;
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
    final dir = Directory.systemTemp.createTempSync('t21e_import_');
    addTearDown(() {
      try {
        dir.deleteSync(recursive: true);
      } catch (_) {}
    });
    expect(engine.initEngine(dataDir: dir.path).ok, isTrue);

    final before = _page().total.toInt();
    const text =
        'vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one\n'
        'vmess://eyJ2IjoiMiIsInBzIjoidHdvIiwiYWRkIjoiYi5leGFtcGxlIiwicG9ydCI6IjQ0MyIsImlkIjoiMjIyMjIyMjItMjIyMi0yMjIyLTIyMjItMjIyMjIyMjIyMjIyIiwiYWlkIjoiMCIsIm5ldCI6IndzIiwidHlwZSI6Im5vbmUiLCJob3N0IjoiIiwicGF0aCI6Ii8iLCJ0bHMiOiIifQ==\n'
        'trojan://password@c.example:443#three\n'
        'ss://YWVzLTI1Ni1nY206c2VjcmV0@d.example:8388#four\n';

    final result = await subs.importFromText(text: text, deduplicate: true);
    expect(result.ok, isTrue, reason: 'parse must succeed');
    expect(result.imported, 4);
    expect(result.profiles.length, 4);

    // The UI persistence seam: the Rust bridge drops subid-less imports, so the
    // UI writes the parsed profiles through the optimistic saveProfile path.
    final persisted = persistImportedProfiles(
      const FrbBridgePort(),
      result.profiles,
    );
    expect(persisted.saved, 4, reason: persisted.firstErrorCode);
    expect(persisted.failed, 0);

    final after = _page().total.toInt();
    expect(after - before, 4, reason: 'import must persist rows');
  });
}
