import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/frb_generated.dart';

Future<void> main() async {
  final repo = Directory.current.path;
  final evidence = '$repo/docs/evidence/complete-port-audit-2026-10-06';
  await RustLib.init(externalLibrary: ExternalLibrary.open('$repo/target/release/bridge_api.dll'));
  final opened = engine.initEngine(dataDir: '$evidence/synthetic-frb-data-100000-batched');
  if (!opened.ok || engine.profileCount() != BigInt.from(100000)) {
    throw StateError('isolated 100000-row fixture is not ready');
  }
  print('AUDIT Dart VM without Flutter tester; real release bridge and isolated 100000-row SQLite');
  final timer = Stopwatch()..start();
  var cursor = 0;
  var count = 0;
  final retained = <c.ProfileDto>[];
  for (;;) {
    final page = engine.queryProfiles(
      filter: const c.ProfileFilterDto(text: null, configTypes: [], subid: null),
      sort: c.ProfileSortDto.indexId, cursor: BigInt.from(cursor), pageSize: 500,
    );
    retained.addAll(page.items);
    count += page.items.length;
    if (count % 10000 == 0) print('AUDIT fetched=$count elapsed_ms=${timer.elapsedMilliseconds}');
    final next = page.nextCursor?.toInt();
    if (next == null || next <= cursor || page.items.isEmpty) break;
    cursor = next;
  }
  timer.stop();
  if (retained.length != 100000) throw StateError('expected all rows');
  final result = {'scope': 'standalone Dart VM, current release DLL, actual SQLite, same 500-row synchronous query loop as FrbBridgePort; no Flutter frames or OS operations', 'rows': count, 'query_all_ms': timer.elapsedMicroseconds / 1000};
  await File('$evidence/frb-native-100000-observations.json').writeAsString(const JsonEncoder.withIndent('  ').convert(result));
  print(jsonEncode(result));
  RustLib.dispose();
}
