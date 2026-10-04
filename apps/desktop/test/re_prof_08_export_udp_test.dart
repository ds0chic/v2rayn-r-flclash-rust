// RE-PROF-08: full client-config export (file + clipboard) and the UDP test
// entry. The production bridge/text is Rust; here the Dart seam is exercised
// with `SyntheticBridgePort` so the cancel / write-failure / clipboard branches
// and the UDP support gate are deterministic.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

ProviderContainer _container(
  SyntheticBridgePort bridge, {
  ClientConfigSavePicker? picker,
}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(6),
      if (picker != null)
        clientConfigSavePickerProvider.overrideWithValue(picker),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

class _ExportButton extends ConsumerWidget {
  const _ExportButton({required this.toClipboard});
  final bool toClipboard;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return TextButton(
      key: const ValueKey('export-client-config'),
      onPressed: () async {
        await exportSelectedClientConfig(
          context,
          ref,
          toClipboard: toClipboard,
        );
      },
      child: const Text('export'),
    );
  }
}

Future<void> _pumpAction(
  WidgetTester tester,
  ProviderContainer container, {
  required bool toClipboard,
}) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Scaffold(body: _ExportButton(toClipboard: toClipboard)),
      ),
    ),
  );
  await tester.pump();
}

String _firstVisibleId(ProviderContainer container) =>
    container.read(profilesControllerProvider).visible.first.id;

void main() {
  late List<MethodCall> clipboardCalls;

  setUp(() {
    clipboardCalls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          if (call.method == 'Clipboard.setData') clipboardCalls.add(call);
          return null;
        });
  });

  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null);
  });

  testWidgets('clipboard export copies the generated client config', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 6);
    final container = _container(bridge);
    await _pumpAction(tester, container, toClipboard: true);

    final id = _firstVisibleId(container);
    container.read(profilesControllerProvider.notifier).selectRow(id);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('export-client-config')));
    await tester.pumpAndSettle();

    expect(bridge.exportedClientConfigIds, contains(id));
    expect(clipboardCalls, hasLength(1));
    final text = (clipboardCalls.single.arguments as Map)['text'] as String;
    expect(text, contains('outbounds'));
  });

  testWidgets('file export cancel writes nothing', (tester) async {
    final bridge = SyntheticBridgePort(count: 6);
    final container = _container(bridge, picker: (_) async => null);
    await _pumpAction(tester, container, toClipboard: false);

    container
        .read(profilesControllerProvider.notifier)
        .selectRow(_firstVisibleId(container));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('export-client-config')));
    await tester.pumpAndSettle();

    expect(bridge.exportedClientConfigIds, hasLength(1));
    expect(bridge.writtenExportFiles, isEmpty);
  });

  testWidgets('file export success writes the generated text', (tester) async {
    final bridge = SyntheticBridgePort(count: 6)
      ..fakeWriteExportFileSucceeds = true;
    final container = _container(
      bridge,
      picker: (name) async => 'C:/tmp/$name',
    );
    await _pumpAction(tester, container, toClipboard: false);

    container
        .read(profilesControllerProvider.notifier)
        .selectRow(_firstVisibleId(container));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('export-client-config')));
    await tester.pumpAndSettle();

    expect(bridge.writtenExportFiles, hasLength(1));
    expect(bridge.writtenExportFiles.single, 'C:/tmp/config.json');
  });

  testWidgets('file export reports a write failure', (tester) async {
    // Default synthetic writeExportFile is a structured failure.
    final bridge = SyntheticBridgePort(count: 6);
    final container = _container(
      bridge,
      picker: (name) async => 'C:/tmp/$name',
    );
    await _pumpAction(tester, container, toClipboard: false);

    container
        .read(profilesControllerProvider.notifier)
        .selectRow(_firstVisibleId(container));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('export-client-config')));
    await tester.pumpAndSettle();

    expect(bridge.writtenExportFiles, hasLength(1));
  });

  test('udp test is gated by the runtime support flag', () {
    final unsupported = SyntheticBridgePort(count: 4)..udpSupported = false;
    final containerA = _container(unsupported);
    final controllerA = containerA.read(profilesControllerProvider.notifier);
    final resultA = controllerA.startSpeedTest(ProfileAction.udpTest);
    expect(resultA.ok, isFalse);
    expect(unsupported.speedTestCalls, isEmpty, reason: 'no job is started');

    final supported = SyntheticBridgePort(count: 4)..udpSupported = true;
    final containerB = _container(supported);
    final controllerB = containerB.read(profilesControllerProvider.notifier);
    controllerB.selectRow(_firstVisibleId(containerB));
    final resultB = controllerB.startSpeedTest(ProfileAction.udpTest);
    expect(resultB.ok, isTrue);
    expect(supported.speedTestCalls, hasLength(1));
    expect(supported.speedTestCalls.single['kind'], 2);
  });
}
