// R3-PROF-06: Custom raw-config export. The byte-preserving raw passthrough is
// asserted in Rust (`raw_custom_export_preserves_bytes_and_original_extension`);
// this exercises the Dart seam: the save dialog is offered the original file
// extension and a Custom clipboard export fails explicitly (upstream passes
// `fileName=null` and `GenerateClientCustomConfig` returns CheckServerSettings).
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

ProviderContainer _container(
  SyntheticBridgePort bridge, {
  ClientConfigSavePicker? picker,
}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(40),
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

String _customId(ProviderContainer container) {
  final state = container.read(profilesControllerProvider);
  for (final row in state.all) {
    if (row.configType == ConfigType.custom) return row.id;
  }
  fail('synthetic bridge produced no Custom profile');
}

String _addressOf(ProviderContainer container, String id) {
  for (final row in container.read(profilesControllerProvider).all) {
    if (row.id == id) return row.address;
  }
  return '';
}

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

  testWidgets('Custom file export keeps the original file extension', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 40)
      ..fakeWriteExportFileSucceeds = true;
    String? suggested;
    final container = _container(
      bridge,
      picker: (name) async {
        suggested = name;
        return 'C:/tmp/$name';
      },
    );
    final id = _customId(container);
    final ext = _addressOf(container, id).split('.').last;

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(body: const _ExportButton(toClipboard: false)),
        ),
      ),
    );
    container.read(profilesControllerProvider.notifier).selectRow(id);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('export-client-config')));
    await tester.pumpAndSettle();

    expect(suggested, isNotNull);
    expect(suggested, isNot('config.json'));
    expect(suggested!.endsWith('.$ext'), isTrue);
    expect(bridge.writtenExportFiles.single, endsWith('.$ext'));
  });

  testWidgets('Custom clipboard export fails instead of faking success', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort(count: 40);
    final container = _container(bridge);
    final id = _customId(container);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(body: const _ExportButton(toClipboard: true)),
        ),
      ),
    );
    container.read(profilesControllerProvider.notifier).selectRow(id);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('export-client-config')));
    await tester.pumpAndSettle();

    expect(bridge.exportedClientConfigIds, contains(id));
    expect(clipboardCalls, isEmpty);
  });
}
