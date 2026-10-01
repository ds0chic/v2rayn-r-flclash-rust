import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';

import 'support/profiles_harness.dart';
import 'support/subs_harness.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late List<String> clipboard;
  late Map<String, dynamic> clipboardData;

  setUp(() {
    clipboard = <String>[];
    clipboardData = <String, dynamic>{};
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          switch (call.method) {
            case 'Clipboard.getData':
              return clipboardData;
            case 'Clipboard.setData':
              clipboard.add((call.arguments as Map)['text'] as String);
              return null;
            default:
              return null;
          }
        });
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null);
  });

  testWidgets('clipboard import reads injected content and imports nodes', (
    tester,
  ) async {
    clipboardData = <String, dynamic>{
      'text':
          'vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one\n'
          'vless://22222222-2222-2222-2222-222222222222@b.example:443?encryption=none#two',
    };
    final container = makeContainer(rows: 20);
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
    await importFromClipboard(context, ref);
    await tester.pump();

    // The synthetic bridge reports success through the shell status message.
    expect(container.read(profilesControllerProvider).events, isNotEmpty);
  });

  testWidgets('export writes share text to the injected clipboard', (
    tester,
  ) async {
    final container = makeContainer(rows: 20);
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
    final id = container.read(profilesControllerProvider).all.first.id;
    container.read(profilesControllerProvider.notifier).selectRow(id);
    await tester.pump();

    await exportProfiles(context, ref, kind: 'share');
    await tester.pump();
    expect(clipboard, isNotEmpty);
    expect(clipboard.last, contains('://'));

    await exportProfiles(context, ref, kind: 'base64');
    await tester.pump();
    expect(clipboard.length, 2);
    expect(clipboard.last.length, greaterThan(4));
  });

  testWidgets('share dialog renders a QR view', (tester) async {
    tester.view.physicalSize = const Size(1200, 1000);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final container = makeContainer(rows: 20);
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
    final id = container.read(profilesControllerProvider).all.first.id;
    container.read(profilesControllerProvider.notifier).selectRow(id);
    await tester.pump();

    final future = shareProfilesQr(context, ref);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.byKey(const ValueKey('profile-share-qr')), findsOneWidget);
    expect(find.byType(QrImageView), findsOneWidget);
    // Close the dialog so the pending future completes.
    await tester.tap(find.text('关闭'));
    await tester.pumpAndSettle();
    await future;
  });

  test('synthetic file writes return a structured not-wired result', () {
    final bridge = SyntheticBridgePort(count: 1);
    final result = bridge.writeExportFile('C:/tmp/export.txt', 'vless://x');
    // Must not fake success: callers/tests cannot cite the test double as
    // real export evidence.
    expect(result.ok, isFalse);
    expect(result.error?.code, 'E_NOT_WIRED');
  });
}
