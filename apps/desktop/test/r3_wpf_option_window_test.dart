import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

/// In-memory [SettingsEditorHost] so the standalone settings editor can be
/// tested without the native window host or the Rust bridge.
class _FakeHost implements SettingsEditorHost {
  _FakeHost({required this.snapshot, this.saveOk = true});

  final Map<String, dynamic> snapshot;
  final bool saveOk;
  int saveCalls = 0;
  int closeCalls = 0;
  Map<String, dynamic>? lastDraft;

  @override
  Future<Map<String, dynamic>> loadSnapshot() async => snapshot;

  @override
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft) async {
    saveCalls++;
    lastDraft = draft;
    return SettingsEditorOutcome(ok: saveOk, message: saveOk ? null : '保存配置失败');
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

Map<String, dynamic> _snapshot({int localPort = 11999}) => <String, dynamic>{
  'Inbound': <dynamic>[
    <String, dynamic>{
      'LocalPort': localPort,
      'Protocol': 0,
      'UdpEnabled': true,
      'SniffingEnabled': true,
      'DestOverride': <dynamic>['http', 'tls'],
      'RouteOnly': false,
      'AllowLANConn': false,
      'NewPort4LAN': false,
      'User': '',
      'Pass': '',
      'SecondLocalPortEnabled': false,
    },
  ],
  'GuiItem': <String, dynamic>{},
};

Future<void> _pumpEditor(WidgetTester tester, _FakeHost host) async {
  tester.view.physicalSize = const Size(1200, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(home: OptionSettingWindow(host: host, standalone: true)),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('standalone editor renders the frozen five tabs from the host '
      'snapshot', (tester) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pumpEditor(tester, host);

    for (final tab in <String>[
      'Core: 基础设置',
      'v2rayN 设置',
      '系统代理设置',
      'Tun 模式设置',
      'Core 类型设置',
    ]) {
      expect(find.text(tab), findsOneWidget, reason: 'missing tab $tab');
    }
    // Wave K form: exactly the upstream 确定/取消 pair, no 应用/保存 extras.
    expect(find.text('确定'), findsOneWidget);
    expect(find.text('取消'), findsOneWidget);
    expect(find.text('应用'), findsNothing);
    expect(host.saveCalls, 0);
    expect(host.closeCalls, 0);
  });

  testWidgets('确定 relays the visible draft to the host then closes', (
    tester,
  ) async {
    final host = _FakeHost(snapshot: _snapshot(localPort: 11999));
    await _pumpEditor(tester, host);

    await tester.tap(find.text('确定'));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 1);
    expect(host.closeCalls, 1);
    final inbound = (host.lastDraft!['Inbound'] as List).first as Map;
    expect(inbound['LocalPort'], 11999);
  });

  testWidgets('取消 discards the draft: no save, window closes', (tester) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pumpEditor(tester, host);

    await tester.tap(find.text('取消'));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 0);
    expect(host.closeCalls, 1);
  });

  testWidgets('Esc closes without writing the draft', (tester) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pumpEditor(tester, host);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    expect(host.saveCalls, 0);
    expect(host.closeCalls, 1);
  });

  testWidgets('a failed save keeps the window open and shows the error', (
    tester,
  ) async {
    final host = _FakeHost(snapshot: _snapshot(), saveOk: false);
    await _pumpEditor(tester, host);

    await tester.tap(find.text('确定'));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 1);
    expect(host.closeCalls, 0);
    expect(find.text('保存配置失败'), findsOneWidget);
  });
}
