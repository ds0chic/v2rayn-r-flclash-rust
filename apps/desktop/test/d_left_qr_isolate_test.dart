// D-LEFT D3a-11: QR decode runs off the UI isolate behind an injectable seam.
//
// The fake-decoder tests below never run real decoding (no isolate, no hang).
// The single default-seam test proves `decodeQrImageAsync` terminates on a
// small synthetic PNG and returns exactly what the sync core returns.
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/subs/scan_image_qr.dart';
import 'package:v2rayn_desktop/features/subs/scan_screen_qr.dart';

import 'scan_image_qr_test.dart' as img_fixture;
import 'support/subs_harness.dart';

const _seamUri =
    'vless://11111111-2222-3333-4444-555555555555@127.0.0.1:11998'
    '?encryption=none&security=none&type=raw#dleft-seam-node';

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
  test('decodeQrImageAsync matches the sync core on a small PNG', () async {
    final png = img_fixture.makeQrPng(_seamUri);
    expect(await decodeQrImageAsync(png), _seamUri);
    expect(await decodeQrImageAsync(png), decodeQrImage(png));
  }, timeout: const Timeout(Duration(seconds: 300)));

  testWidgets('screen scan maps a throwing decoder to no-QR-found UX', (
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
        capture: () async => Uint8List.fromList(<int>[1, 2, 3]),
        window: window,
        settle: Duration.zero,
        decoder: (_) => throw Exception('isolate gone'),
      ),
    );

    expect(window.calls, <String>['hide', 'show']);
    expect(
      container.read(uiShellControllerProvider).message,
      contains('未发现有效二维码'),
    );
    expect(bridge.queryAllProfiles(), isEmpty);
  });

  testWidgets('image scan maps a throwing decoder to no-QR-found UX', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 0);
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    final probe = await _pumpProbe(tester, container);

    await scanImageQr(
      probe.context,
      probe.ref,
      picker: () async => Uint8List.fromList(<int>[1, 2, 3]),
      decoder: (_) => throw Exception('isolate gone'),
    );

    expect(
      container.read(uiShellControllerProvider).message,
      contains('未发现有效二维码'),
    );
    expect(bridge.queryAllProfiles(), isEmpty);
  });
}
