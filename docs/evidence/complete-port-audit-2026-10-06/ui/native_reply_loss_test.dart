import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

void main() {
  testWidgets('settings missing reply reaches a bounded failure', (tester) async {
    const channel = MethodChannel(kOptionWindowChannel);
    final messenger = TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'ready') return '{"GuiItem":{"AutoRun":false}}';
      return null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final host = NativeSettingsEditorHost();
    await host.loadSnapshot();
    var settled = false;
    host.save(<String, dynamic>{'GuiItem': <String, dynamic>{'AutoRun': false}}).then((_) { settled = true; });
    await tester.pump(const Duration(seconds: 31));
    expect(settled, isTrue, reason: 'saveDraft acknowledgement is not the save result; a lost saveOutcome must not hang forever');
  });

  testWidgets('routing missing reply reaches a bounded failure', (tester) async {
    const channel = MethodChannel('v2rayn/routing_window');
    final messenger = TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'ready') return '{"schemes":[],"domainStrategy":"AsIs"}';
      return null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final host = NativeRoutingEditorHost();
    await host.loadSnapshot();
    var settled = false;
    host.commit('{"kind":"strategy","domainStrategy":"AsIs"}').then((_) { settled = true; });
    await tester.pump(const Duration(seconds: 31));
    expect(settled, isTrue, reason: 'a missing saveOutcome must resolve as unknown/failed and offer reconciliation');
  });

  testWidgets('malformed routing snapshot is rejected, not editable empty data', (tester) async {
    const channel = MethodChannel('v2rayn/routing_window');
    final messenger = TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'ready') return '{"unexpectedSchema":true}';
      return null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    final host = NativeRoutingEditorHost();
    await expectLater(host.loadSnapshot(), throwsA(isA<RoutingEditorLoadException>()));
  });
}
