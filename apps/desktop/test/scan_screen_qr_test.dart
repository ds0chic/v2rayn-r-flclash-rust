// FIX-05B (ACT-MAIN-017): screen QR scan → import.
//
// Uses an injectable screenshot source and an injectable window control so the
// hide/show sequence and every branch can be asserted without a real window or
// screen. The QR fixture is generated at runtime from a synthetic vless:// URI;
// no network, no listeners, no user data.
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;
import 'package:qr/qr.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/subs/scan_screen_qr.dart';

import 'support/subs_harness.dart';

const _scanUri =
    'vless://11111111-2222-3333-4444-555555555555@127.0.0.1:11998'
    '?encryption=none&security=none&type=raw#fixed05b-screen-node';

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

Uint8List _blankPng() {
  final image = img.Image(width: 96, height: 96);
  img.fill(image, color: img.ColorRgb8(255, 255, 255));
  return Uint8List.fromList(img.encodePng(image));
}

class _RecordingWindow implements ScanWindowControl {
  final List<String> calls = <String>[];

  @override
  Future<void> hide() async => calls.add('hide');

  @override
  Future<void> show() async => calls.add('show');
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
  testWidgets('success captures, restores the window and imports', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 0);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final probe = await _pumpProbe(tester, container);
    final window = _RecordingWindow();

    await tester.runAsync(
      () => scanScreenQr(
        probe.context,
        probe.ref,
        capture: () async => _makeQrPng(_scanUri),
        window: window,
        settle: Duration.zero,
      ),
    );

    expect(window.calls, <String>['hide', 'show']);
    expect(
      container.read(uiShellControllerProvider).message,
      contains('已从屏幕扫描导入 1 个节点'),
    );
  });

  testWidgets('cancelled capture is silent but still restores the window', (
    tester,
  ) async {
    final container = makeSubsContainer(bridge: SyntheticBridgePort(count: 0));
    addTearDown(container.dispose);
    final probe = await _pumpProbe(tester, container);
    final window = _RecordingWindow();
    final before = container.read(uiShellControllerProvider).message;

    await tester.runAsync(
      () => scanScreenQr(
        probe.context,
        probe.ref,
        capture: () async => null,
        window: window,
        settle: Duration.zero,
      ),
    );

    expect(window.calls, <String>['hide', 'show']);
    expect(container.read(uiShellControllerProvider).message, before);
  });

  testWidgets('image without a QR reports an actionable message', (
    tester,
  ) async {
    final container = makeSubsContainer(bridge: SyntheticBridgePort(count: 0));
    addTearDown(container.dispose);
    final probe = await _pumpProbe(tester, container);
    final window = _RecordingWindow();

    await tester.runAsync(
      () => scanScreenQr(
        probe.context,
        probe.ref,
        capture: () async => _blankPng(),
        window: window,
        settle: Duration.zero,
      ),
    );

    expect(window.calls, <String>['hide', 'show']);
    expect(
      container.read(uiShellControllerProvider).message,
      contains('未发现有效二维码'),
    );
  });

  testWidgets('capture exception restores the window and reports failure', (
    tester,
  ) async {
    final container = makeSubsContainer(bridge: SyntheticBridgePort(count: 0));
    addTearDown(container.dispose);
    final probe = await _pumpProbe(tester, container);
    final window = _RecordingWindow();

    await tester.runAsync(
      () => scanScreenQr(
        probe.context,
        probe.ref,
        capture: () async => throw const ScreenCaptureException('boom'),
        window: window,
        settle: Duration.zero,
      ),
    );

    expect(window.calls, <String>['hide', 'show']);
    expect(
      container.read(uiShellControllerProvider).message,
      contains('屏幕截图失败'),
    );
  });
}
