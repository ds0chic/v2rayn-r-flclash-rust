// FIX-05: real flutter_rust_bridge + SQLite regression for the image QR scan
// entry (ACT-MAIN-018). The QR fixture is generated at runtime from a synthetic
// vless:// URI; no network, no listeners, no user data.
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;
import 'package:qr/qr.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/subs/scan_image_qr.dart';

import 'support/subs_harness.dart';

const _scanUri =
    'vless://11111111-2222-3333-4444-555555555555@127.0.0.1:11998'
    '?encryption=none&security=none&type=raw#fixed05-scan-node';

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

Uint8List _makeQrPng(String text, {int scale = 6, int quietZone = 4}) {
  final qr = QrImage(
    QrCode.fromData(data: text, errorCorrectLevel: QrErrorCorrectLevel.M),
  );
  final modules = qr.moduleCount;
  final size = (modules + quietZone * 2) * scale;
  final image = img.Image(width: size, height: size);
  img.fill(image, color: img.ColorRgb8(255, 255, 255));
  final black = img.ColorRgb8(0, 0, 0);
  for (var row = 0; row < modules; row++) {
    for (var col = 0; col < modules; col++) {
      if (!qr.isDark(row, col)) continue;
      final x = (col + quietZone) * scale;
      final y = (row + quietZone) * scale;
      img.fillRect(
        image,
        x1: x,
        y1: y,
        x2: x + scale - 1,
        y2: y + scale - 1,
        color: black,
      );
    }
  }
  return Uint8List.fromList(img.encodePng(image));
}

Future<({WidgetRef ref, BuildContext context})> _pumpProbe(
  WidgetTester tester,
  ProviderContainer container,
) async {
  late WidgetRef capturedRef;
  late BuildContext capturedContext;
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: RefProbe(
          onRef: (context, ref) {
            capturedRef = ref;
            capturedContext = context;
          },
        ),
      ),
    ),
  );
  return (ref: capturedRef, context: capturedContext);
}

void main() {
  String? libraryPath;

  setUpAll(() async {
    libraryPath = _findLibrary();
    if (libraryPath != null) {
      await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath!));
    }
  });

  testWidgets('scan image imports, persists and survives reopen', (
    tester,
  ) async {
    if (libraryPath == null) {
      markTestSkipped(
        'bridge_api.dll not built; run `cargo build -p bridge_api`',
      );
      return;
    }
    final dir = Directory.systemTemp.createTempSync('fix05_scan_');
    addTearDown(() {
      try {
        dir.deleteSync(recursive: true);
      } catch (_) {}
    });
    expect(engine.initEngine(dataDir: dir.path).ok, isTrue);
    final container = makeSubsContainer(bridge: const FrbBridgePort());
    addTearDown(container.dispose);
    final probe = await _pumpProbe(tester, container);

    await tester.runAsync(() async {
      await scanImageQr(
        probe.context,
        probe.ref,
        picker: () async => _makeQrPng(_scanUri),
      );
    });

    expect(
      container.read(uiShellControllerProvider).message,
      contains('已从扫描导入 1 个节点'),
    );
    expect(_page().items.map((p) => p.remarks), contains('fixed05-scan-node'));

    // Reopen the same data directory: the scanned node must survive.
    expect(engine.initEngine(dataDir: dir.path).ok, isTrue);
    expect(_page().items.map((p) => p.remarks), contains('fixed05-scan-node'));
  });

  testWidgets('cancel and non-QR image leave the database untouched', (
    tester,
  ) async {
    if (libraryPath == null) {
      markTestSkipped(
        'bridge_api.dll not built; run `cargo build -p bridge_api`',
      );
      return;
    }
    final dir = Directory.systemTemp.createTempSync('fix05_scan_skip_');
    addTearDown(() {
      try {
        dir.deleteSync(recursive: true);
      } catch (_) {}
    });
    expect(engine.initEngine(dataDir: dir.path).ok, isTrue);
    final container = makeSubsContainer(bridge: const FrbBridgePort());
    addTearDown(container.dispose);
    final probe = await _pumpProbe(tester, container);
    final before = _page().total.toInt();

    await tester.runAsync(() async {
      await scanImageQr(probe.context, probe.ref, picker: () async => null);
    });
    expect(_page().total.toInt(), before);

    await tester.runAsync(() async {
      await scanImageQr(
        probe.context,
        probe.ref,
        picker: () async => Uint8List(0),
      );
    });
    expect(_page().total.toInt(), before);
    expect(
      container.read(uiShellControllerProvider).message,
      contains('所选图片为空'),
    );
  });
}
