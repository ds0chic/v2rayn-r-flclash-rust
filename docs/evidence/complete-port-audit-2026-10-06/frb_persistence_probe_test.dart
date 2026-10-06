import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/api/subs.dart' as subs;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('production FRB + isolated SQLite persists and reopens synthetic profiles', () async {
    final repo = Directory.current.parent.parent.path;
    final evidence = '$repo/docs/evidence/complete-port-audit-2026-10-06';
    final phase = Platform.environment['V2RAYN_R_AUDIT_PHASE'] ?? 'seed';
    final count = int.parse(Platform.environment['V2RAYN_R_AUDIT_ROWS'] ?? '10000');
    final data = '$evidence/synthetic-frb-data${count == 10000 ? '' : '-$count-batched'}';
    await RustLib.init(
      externalLibrary: ExternalLibrary.open('$repo/target/release/bridge_api.dll'),
    );
    print('AUDIT native library initialized phase=$phase rows=$count');
    expect(engine.initEngine(dataDir: data).ok, isTrue);
    print('AUDIT real SQLite engine opened');
    expect(engine.dataDir(), data);
    final observations = <String, Object?>{'phase': phase, 'rows': count, 'dll': 'target/release/bridge_api.dll', 'effects': 'no core, no scheduler, no socket, no platform API'};
    if (phase == 'seed') {
      expect(engine.profileCount(), BigInt.zero, reason: 'fixture must start empty');
      final group = subs.saveSubItem(item: const c.SubItemDto(
        id: '', remarks: 'audit synthetic local group', url: '', moreUrl: '',
        enabled: false, userAgent: '', sort: 1, autoUpdateInterval: 0, updateTime: 0,
      ));
      expect(group.ok, isTrue);
      final importTimer = Stopwatch()..start();
      for (var start = 0; start < count; start += 10000) {
        final size = count - start < 10000 ? count - start : 10000;
        final lines = List.generate(size, (i) => 'socks://127.0.0.1:11999#audit-${start + i}').join('\n');
        final imported = await subs.importFromText(text: lines, subid: group.item!.id, deduplicate: false);
        expect(imported.ok, isTrue);
        expect(imported.imported, size);
      }
      observations['import_ms'] = importTimer.elapsedMicroseconds / 1000;
      observations['fixture_batch_rows'] = 10000;
      final loaded = settings.getSettings();
      expect(loaded.ok, isTrue);
      final document = jsonDecode(loaded.settingsJson) as Map<String, dynamic>;
      final ui = Map<String, dynamic>.from(document['UiItem'] as Map);
      ui['CurrentLanguage'] = 'en';
      expect(settings.saveSettingsGroup(group: 'UiItem', patchJson: jsonEncode(ui), expectedRevision: loaded.revision).ok, isTrue);
    }
    expect(engine.profileCount(), BigInt.from(count));
    print('AUDIT persisted profile count=$count');
    final loaded = settings.getSettings();
    final document = jsonDecode(loaded.settingsJson) as Map<String, dynamic>;
    expect((document['UiItem'] as Map)['CurrentLanguage'], 'en');
    print('AUDIT saved language confirmed; next synchronous all-profile query');
    await File('$evidence/frb-$phase${count == 10000 ? '' : '-$count'}-progress.json').writeAsString(jsonEncode({'phase': phase, 'persisted_rows': count, 'settings_reopen_confirmed': true, 'next': 'queryAllProfiles'}));
    final bridge = FrbBridgePort();
    var ticks = 0;
    final timer = Timer.periodic(const Duration(milliseconds: 10), (_) => ticks++);
    await Future<void>.delayed(const Duration(milliseconds: 25));
    final beforeTicks = ticks;
    final queryTimer = Stopwatch()..start();
    final rows = bridge.queryAllProfiles();
    queryTimer.stop();
    print('AUDIT synchronous query returned ${rows.length} rows in ${queryTimer.elapsedMilliseconds}ms');
    observations['query_all_ms'] = queryTimer.elapsedMicroseconds / 1000;
    observations['timer_ticks_during_synchronous_query'] = ticks - beforeTicks;
    observations['queried_rows'] = rows.length;
    timer.cancel();
    expect(rows.length, count);
    expect(rows.every((row) => row.remarks.startsWith('audit-')), isTrue);
    await File('$evidence/frb-$phase${count == 10000 ? '' : '-$count'}-observations.json').writeAsString(const JsonEncoder.withIndent('  ').convert(observations));
    // Timing is an observation, not a best-performance assertion.
  }, timeout: const Timeout(Duration(minutes: 5)));
}
