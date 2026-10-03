// FIX-04B: real-bridge regression for complete-config import (FIX-04 follow-up).
// Loads the built `bridge_api.dll`, points the engine at a temporary data
// directory and walks the UI chain: paste a complete v2ray JSON config ->
// `importFromText` -> file-type Custom node under `<data>/config/` ->
// UI-side persist -> node table -> reopen. Synthetic payloads only
// (example.invalid, synthetic UUIDs); no network, no listeners, no user data.
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/subs.dart' as subs;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';

import 'support/profiles_harness.dart';
import 'support/subs_harness.dart';

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

const _fullXrayJson =
    '{"inbounds":[{"port":11888,"protocol":"socks","settings":{"udp":true}}],'
    '"outbounds":[{"protocol":"vmess","tag":"fix04b-marker","settings":{"vnext":'
    '[{"address":"node.example.invalid","port":11980,"users":'
    '[{"id":"11111111-2222-3333-4444-555555555555"}]}]},"streamSettings":'
    '{"network":"tcp"}}],"x-future":[1,2,3]}';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'complete Xray config imports file-type, persists and reopens',
    () async {
      final libraryPath = _findLibrary();
      if (libraryPath == null) {
        markTestSkipped(
          'bridge_api.dll not built; run `cargo build -p bridge_api`',
        );
        return;
      }
      await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
      final dir = Directory.systemTemp.createTempSync('fix04b_config_');
      addTearDown(() {
        try {
          dir.deleteSync(recursive: true);
        } catch (_) {}
      });
      expect(engine.initEngine(dataDir: dir.path).ok, isTrue);

      final result = await subs.importFromText(
        text: _fullXrayJson,
        deduplicate: true,
      );
      expect(result.ok, isTrue);
      expect(result.imported, 1);
      final profile = result.profiles.single;
      expect(profile.configType, ConfigType.custom);
      expect(profile.coreType, CoreType.xray);
      expect(profile.address.trim(), isNotEmpty);

      final file = File(
        '${dir.path}${Platform.pathSeparator}config'
        '${Platform.pathSeparator}${profile.address}',
      );
      expect(file.existsSync(), isTrue, reason: 'materialized config file');
      final text = file.readAsStringSync();
      expect(text, contains('fix04b-marker'));
      expect(text, contains('x-future'));

      final before = _page().total.toInt();
      final persisted = persistImportedProfiles(
        const FrbBridgePort(),
        result.profiles,
      );
      expect(persisted.saved, 1, reason: persisted.firstErrorCode);
      expect(_page().total.toInt() - before, 1);

      // Reopen the same data directory: the imported node must survive.
      expect(engine.initEngine(dataDir: dir.path).ok, isTrue);
      final names = _page().items.map((p) => p.remarks).toList();
      expect(names, contains(profile.remarks));

      // Unrecognized complete-config text fails without creating a node.
      expect(_page().total.toInt(), before + 1);
      final bad = await subs.importFromText(
        text: '这不是任何链接或配置',
        deduplicate: false,
      );
      expect(bad.ok, isFalse);
      expect(bad.imported, 0);
      expect(_page().total.toInt(), before + 1);
    },
  );

  testWidgets('paste dialog cancel imports nothing', (tester) async {
    final container = makeContainer(rows: 3);
    addTearDown(container.dispose);
    (BuildContext, WidgetRef)? captured;
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: RefProbe(onRef: (context, ref) => captured = (context, ref)),
          ),
        ),
      ),
    );
    await tester.pump();
    final (context, ref) = captured!;
    final before = container.read(profilesControllerProvider).profiles.length;

    final pending = importFromTextDialog(context, ref);
    await tester.pump();
    expect(find.byKey(const ValueKey('import-paste-dialog')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('import-paste-cancel')));
    await tester.pump();
    await pending;

    expect(container.read(profilesControllerProvider).profiles.length, before);
  });
}
