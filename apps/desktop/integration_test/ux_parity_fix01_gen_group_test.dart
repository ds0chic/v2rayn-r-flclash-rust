// FIX-01 / ACT-PROF-007/008: real Windows window + real FRB/Rust/SQLite proof
// that "节点右键 → 一键生成策略组" persists a PolicyGroup for the subscription
// group selected in the table, needs no selected node, and survives a reopen
// (separate process against the same isolated data dir).
//
// Run modes:
//   V2RAYN_R_FIX01_MODE=generate  (default) creates a synthetic subscription,
//     imports two synthetic region nodes, generates all + region groups.
//   V2RAYN_R_FIX01_MODE=reopen    launches a fresh process on the same data dir
//     and asserts the generated groups still load from SQLite.
// No kernel is started, no port is bound; the synthetic URL is loopback and is
// never requested.
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

const _frame = ValueKey('fix01-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 500));

Future<void> _wait(
  WidgetTester tester,
  bool Function() ready, {
  int tries = 100,
}) async {
  for (var i = 0; i < tries && !ready(); i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  if (Platform.environment['V2RAYN_R_FIX01_SKIP_IMAGES'] == '1') return;
  await _settle(tester);
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_frame),
  );
  final image = await boundary.toImage(pixelRatio: 1);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  if (bytes != null) {
    await File('$dir/$name.png').writeAsBytes(bytes.buffer.asUint8List());
  }
  image.dispose();
}

c.SubItemDto _subDraft(String remarks) => c.SubItemDto(
  id: '',
  remarks: remarks,
  url: 'http://127.0.0.1:11998/not-requested',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 1,
  autoUpdateInterval: 0,
  updateTime: 0,
);

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('FIX-01 generate group persists and survives reopen', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir = Platform.environment['V2RAYN_R_FIX01_EVIDENCE'];
    final mode = Platform.environment['V2RAYN_R_FIX01_MODE'] ?? 'generate';
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    await Directory(evidenceDir!).create(recursive: true);
    final checks = <Map<String, Object?>>[];
    final failures = <String>[];
    final extra = <String, Object?>{'mode': mode};
    final resultName = mode == 'reopen'
        ? 'reopen-observations.json'
        : 'observations.json';
    void write({bool complete = false}) {
      File('$evidenceDir/$resultName').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'applicationCommit': '1251cbc',
          'recordingComplete': complete,
          ...extra,
          'failures': failures,
          'checks': checks,
        }),
      );
    }

    void check(String step, bool passed, Map<String, Object?> actual) {
      checks.add({'step': step, 'passed': passed, 'actual': actual});
      if (!passed) failures.add(step);
      write();
    }

    RustBridgeInit.configure(RustLib.init);
    await RustBridgeInit.init();
    runApp(
      ProviderScope(
        overrides: [
          uiStateStoreProvider.overrideWithValue(
            FileUiStateStore(overridePath: '$dataDir/ui_state.json'),
          ),
        ],
        child: const RepaintBoundary(key: _frame, child: V2rayNRApp()),
      ),
    );
    await _wait(
      tester,
      () => find.byKey(const ValueKey('menu-配置项')).evaluate().isNotEmpty,
    );
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );

    if (mode == 'reopen') {
      final previous = File('$evidenceDir/observations.json');
      final prior = previous.existsSync()
          ? jsonDecode(previous.readAsStringSync()) as Map
          : const <String, Object?>{};
      final expectedId = prior['generatedGroupId'] as String?;
      final subId = prior['subId'] as String?;
      final state = container.read(profilesControllerProvider);
      final groups = state.profiles
          .where((p) => p.configType == ConfigType.policyGroup)
          .toList();
      final matching = groups
          .where((g) => expectedId == null || g.indexId == expectedId)
          .toList();
      final byGroup = groups.where((g) => subId == null || g.subid == subId);
      c.ProfileDto? found;
      if (matching.isNotEmpty) {
        found = matching.first;
      } else if (byGroup.isNotEmpty) {
        found = byGroup.first;
      }
      final regionCount = groups.where((g) => g.subid == subId).length;
      check(
        'reopen-generated-group-exists',
        found != null && found.configType == ConfigType.policyGroup,
        {
          'expectedGroupId': expectedId,
          'foundGroupId': found?.indexId,
          'foundSubId': found?.subid,
          'totalProfiles': state.profiles.length,
        },
      );
      check('reopen-region-groups-exist', regionCount >= 3, {
        'regionGroupCount': regionCount,
      });
      await _shot(tester, evidenceDir, 'reopen-window');
      write(complete: true);
      expect(failures, isEmpty, reason: 'FIX-01 reopen gaps: $failures');
      return;
    }

    final bridge = container.read(bridgePortProvider);
    final subs = container.read(subsControllerProvider.notifier);
    final saved = subs.save(_subDraft('FIX01合成订阅'));
    final subId = saved.item?.id ?? '';
    check('subscription-created', saved.ok && subId.isNotEmpty, {
      'subId': subId,
      'ok': saved.ok,
      'error': saved.error?.code,
    });
    extra['subId'] = subId;

    const links =
        'vless://00000001-1111-1111-1111-111111111111@127.0.0.1:11998?encryption=none#FIX01-HK-01\n'
        'vless://00000002-2222-2222-2222-222222222222@127.0.0.1:11998?encryption=none#FIX01-US-01';
    final imported = await bridge.importFromText(links, subid: subId);
    container.read(profilesControllerProvider.notifier).reload();
    await _settle(tester);
    check('nodes-imported-into-group', imported.ok && imported.imported == 2, {
      'ok': imported.ok,
      'imported': imported.imported,
      'error': imported.error?.code,
    });

    await tester.tap(find.byKey(ValueKey('group-filter-$subId')));
    await _settle(tester);
    check(
      'group-selected',
      container.read(profilesControllerProvider).groupSubId == subId,
      {
        'groupSubId': container.read(profilesControllerProvider).groupSubId,
        'visible': container.read(profilesControllerProvider).visible.length,
      },
    );

    // No node selected: prove generation does not depend on one.
    container.read(profilesControllerProvider.notifier).clearSelection();
    await _settle(tester);
    check(
      'no-selection',
      container.read(profilesControllerProvider).selected.isEmpty,
      {'selected': container.read(profilesControllerProvider).selected.length},
    );

    final tableRect = tester.getRect(find.byType(ProfilesTable));
    Future<void> openGenerate(String item) async {
      await tester.tapAt(
        Offset(tableRect.right - 24, tableRect.bottom - 24),
        kind: PointerDeviceKind.mouse,
        buttons: kSecondaryButton,
      );
      await _settle(tester);
      await tester.tap(find.byKey(const ValueKey('ctx-一键生成策略组')));
      await _settle(tester);
      await tester.tap(find.byKey(ValueKey('ctx-$item')));
      await _settle(tester);
    }

    final beforeAll = container
        .read(profilesControllerProvider)
        .profiles
        .map((p) => p.indexId)
        .toSet();
    await openGenerate('全部配置项');
    final allGroups = container
        .read(profilesControllerProvider)
        .profiles
        .where(
          (p) =>
              p.configType == ConfigType.policyGroup &&
              p.subid == subId &&
              !beforeAll.contains(p.indexId),
        )
        .toList();
    check('generate-all-persisted', allGroups.length == 1, {
      'created': allGroups.length,
      'remarks': allGroups.isEmpty ? null : allGroups.first.remarks,
    });
    final allId = allGroups.isEmpty ? null : allGroups.first.indexId;
    extra['generatedGroupId'] = allId;

    // Generated object fields must mirror ConfigHandler.AddGroupAllServer:
    // PolicyGroup / CoreType.Xray / GroupType "PolicyGroup" / SubChildItems
    // = the subscription id / no explicit ChildItems / the default-all filter /
    // LeastPing (EMultipleLoad 0).
    final allGroup = allGroups.isEmpty ? null : allGroups.first;
    final allExtra = allGroup?.protoExtra;
    final allCore = allGroup?.coreType;
    check(
      'generate-all-fields-match-upstream',
      allGroup != null &&
          allGroup.coreType == CoreType.xray &&
          allExtra?.groupType == 'PolicyGroup' &&
          allExtra?.subChildItems == subId &&
          (allExtra?.childItems == null || allExtra!.childItems!.isEmpty) &&
          (allExtra?.filter?.contains('emaining') ?? false) &&
          allExtra?.multipleLoad == 0,
      {
        'coreType': allCore?.name,
        'groupType': allExtra?.groupType,
        'subChildItems': allExtra?.subChildItems,
        'childItems': allExtra?.childItems,
        'filter': allExtra?.filter,
        'multipleLoad': allExtra?.multipleLoad,
        'isSub': allGroup?.isSub,
      },
    );

    final afterAll = container.read(profilesControllerProvider);
    final inTable =
        allId != null &&
        afterAll.all.any((r) => r.id == allId) &&
        afterAll.visible.any((r) => r.id == allId);
    check('generated-group-in-node-table', inTable, {
      'generatedGroupId': allId,
      'inAll': allId != null && afterAll.all.any((r) => r.id == allId),
      'inVisible': allId != null && afterAll.visible.any((r) => r.id == allId),
      'totalAfter': afterAll.totalCount,
    });
    await _shot(tester, evidenceDir, 'generate-all');

    final beforeRegion = container
        .read(profilesControllerProvider)
        .profiles
        .map((p) => p.indexId)
        .toSet();
    await openGenerate('按地区分组');
    final regionGroups = container
        .read(profilesControllerProvider)
        .profiles
        .where(
          (p) =>
              p.configType == ConfigType.policyGroup &&
              p.subid == subId &&
              !beforeRegion.contains(p.indexId),
        )
        .toList();
    final regions = <String>{
      for (final g in regionGroups) g.remarks.split(' - ').last,
    };
    check(
      'generate-region-persisted',
      regionGroups.length == 2 &&
          regions.containsAll(<String>{'HK', 'US'}) &&
          regionGroups.every(
            (g) =>
                g.coreType == CoreType.xray &&
                g.protoExtra.groupType == 'PolicyGroup' &&
                g.protoExtra.subChildItems == subId &&
                (g.protoExtra.filter?.isNotEmpty ?? false),
          ),
      {
        'created': regionGroups.length,
        'regions': regions.toList(),
        'filters': regionGroups.map((g) => g.protoExtra.filter).toList(),
      },
    );
    extra['regionGroupIds'] = regionGroups.map((p) => p.indexId).toList();
    await _shot(tester, evidenceDir, 'generate-region');

    write(complete: true);
    expect(failures, isEmpty, reason: 'FIX-01 generate gaps: $failures');
  });
}
