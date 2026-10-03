import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;
import 'package:qr/qr.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/scan_image_qr.dart';

import 'support/subs_harness.dart';

const _scanUri =
    'vless://11111111-2222-3333-4444-555555555555@127.0.0.1:11998'
    '?encryption=none&security=none&type=raw#fixed05-scan-node';

/// Synthetic QR fixture: renders the same matrix `qr_flutter` paints, with a
/// 4-module quiet zone, to PNG bytes (synthetic data only).
Uint8List makeQrPng(String text, {int scale = 6, int quietZone = 4}) {
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

Uint8List makeBlankPng() {
  final image = img.Image(width: 64, height: 64);
  img.fill(image, color: img.ColorRgb8(255, 255, 255));
  return Uint8List.fromList(img.encodePng(image));
}

Future<({WidgetRef ref, BuildContext context})> pumpProbe(
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
  test('decodeQrImage reads a generated QR PNG', () {
    expect(decodeQrImage(makeQrPng(_scanUri)), _scanUri);
  });

  test('decodeQrImage retries a horizontally mirrored code', () {
    final decoded = img.decodeImage(makeQrPng(_scanUri))!;
    final mirrored = img.encodePng(
      img.flip(decoded, direction: img.FlipDirection.horizontal),
    );
    expect(decodeQrImage(Uint8List.fromList(mirrored)), _scanUri);
  });

  test('decodeQrImage returns null for non-QR and unreadable bytes', () {
    expect(decodeQrImage(makeBlankPng()), isNull);
    expect(decodeQrImage(Uint8List(0)), isNull);
    expect(
      decodeQrImage(Uint8List.fromList(List<int>.filled(64, 0x41))),
      isNull,
    );
  });

  testWidgets('scan imports a node through the pipeline and keeps it stored', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 0);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final probe = await pumpProbe(tester, container);

    await scanImageQr(
      probe.context,
      probe.ref,
      picker: () async => makeQrPng(_scanUri),
    );

    expect(
      container.read(uiShellControllerProvider).message,
      contains('已从扫描导入 1 个节点'),
    );
    final stored = bridge
        .queryAllProfiles()
        .where((p) => p.remarks == 'fixed05-scan-node')
        .toList();
    expect(stored, hasLength(1));
    expect(stored.single.subid, isEmpty);

    // Reopen: a fresh container over the same persisted store sees the node.
    final reopened = makeSubsContainer(bridge: bridge);
    addTearDown(reopened.dispose);
    final controller = reopened.read(profilesControllerProvider.notifier);
    controller.reload();
    expect(
      reopened.read(profilesControllerProvider).profiles.map((p) => p.remarks),
      contains('fixed05-scan-node'),
    );
  });

  testWidgets('cancelling the picker is silent and does not mutate state', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 0);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final probe = await pumpProbe(tester, container);
    final before = container.read(uiShellControllerProvider).message;

    await scanImageQr(probe.context, probe.ref, picker: () async => null);

    expect(container.read(uiShellControllerProvider).message, before);
    expect(bridge.queryAllProfiles(), isEmpty);
  });

  testWidgets('non-QR image reports an actionable error without crashing', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 0);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final probe = await pumpProbe(tester, container);

    await scanImageQr(
      probe.context,
      probe.ref,
      picker: () async => makeBlankPng(),
    );

    expect(
      container.read(uiShellControllerProvider).message,
      contains('未发现有效二维码'),
    );
    expect(bridge.queryAllProfiles(), isEmpty);
  });

  testWidgets('empty image bytes report an actionable error', (tester) async {
    final bridge = SyntheticBridgePort(count: 0);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final probe = await pumpProbe(tester, container);

    await scanImageQr(
      probe.context,
      probe.ref,
      picker: () async => Uint8List(0),
    );

    expect(
      container.read(uiShellControllerProvider).message,
      contains('所选图片为空'),
    );
  });

  testWidgets('picker failure reports an actionable error', (tester) async {
    final bridge = SyntheticBridgePort(count: 0);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final probe = await pumpProbe(tester, container);

    await scanImageQr(
      probe.context,
      probe.ref,
      picker: () async => throw Exception('dialog unavailable'),
    );

    expect(
      container.read(uiShellControllerProvider).message,
      contains('打开图片失败'),
    );
    expect(bridge.queryAllProfiles(), isEmpty);
  });
}
