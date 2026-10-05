// R4-11 field form + default-value repro.
//
// These assertions express the R4-11 contract and intentionally fail on the
// pre-fix implementation (field linkage gates are missing, the number field
// keeps an unparseable draft, and a missing value is shown as 'null'). They are
// copied from the audit points in
// docs/evidence/user-flow-audit-2026-10-05/settings-audit.md and
// profiles-audit.md, and are NOT adjusted to the current buggy behavior.
//
// Synthetic only: in-memory settings host / plain widget. No native library, no
// kernel, no port, no host proxy/TUN/registry, no user data.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

class _FakeHost implements SettingsEditorHost {
  _FakeHost({required this.snapshot});

  final Map<String, dynamic> snapshot;
  int saveCalls = 0;
  Map<String, dynamic>? lastDraft;

  @override
  Future<Map<String, dynamic>> loadSnapshot() async => snapshot;

  @override
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft) async {
    saveCalls++;
    lastDraft = draft;
    return const SettingsEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {}
}

Map<String, dynamic> _snapshot(String nullDnsText) => <String, dynamic>{
  'Inbound': <dynamic>[
    <String, dynamic>{
      'LocalPort': 11808,
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
  'SimpleDNSItem': <String, dynamic>{
    'FakeIP': false,
    'GlobalFakeIp': null,
    'DirectDNS': nullDnsText,
    'EnableHappyEyeballs': false,
  },
  'HappyEyeballs4RayItem': <String, dynamic>{
    'TryDelayMs': 250,
    'PrioritizeIPv6': false,
    'Interleave': 1,
    'MaxConcurrentTry': 4,
  },
  'TunModeItem': <String, dynamic>{'Mtu': 9000, 'Stack': null},
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
  testWidgets('R4-11 link: GlobalFakeIp only visible when FakeIP is on', (
    tester,
  ) async {
    final host = _FakeHost(snapshot: _snapshot('1.1.1.1'));
    await _pumpEditor(tester, host);
    // The "v2rayN 设置" tab carries the historical DNS keep section.
    await tester.tap(find.text('v2rayN 设置'));
    await tester.pumpAndSettle();
    // The display tab is a single child scroll view, so every row is built.
    // Upstream `DNSSettingWindow` hides GlobalFakeIp unless FakeIP is checked.
    expect(
      find.byKey(const ValueKey('fakeip-toggle')),
      findsOneWidget,
      reason: 'FakeIP field must be visible on the v2rayN settings tab',
    );
    expect(
      find.byKey(const ValueKey('global-fakeip-toggle')),
      findsNothing,
      reason: 'GlobalFakeIp must stay collapsed while FakeIP is off',
    );
    await tester.ensureVisible(find.byKey(const ValueKey('fakeip-toggle')));
    await tester.tap(find.byKey(const ValueKey('fakeip-toggle')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('global-fakeip-toggle')),
      findsOneWidget,
      reason: 'toggling FakeIP must reveal the linked GlobalFakeIp row',
    );
  });

  testWidgets(
    'R4-11 default: null/empty group falls back to upstream defaults',
    (tester) async {
      // A snapshot that omits CoreBasicItem/GuiItem (e.g. a partial read) must
      // still render the upstream defaults, not CLR zero/blank values, and must
      // never persist a literal `null`.
      final host = _FakeHost(
        snapshot: <String, dynamic>{
          'Inbound': <dynamic>[
            <String, dynamic>{'LocalPort': 11808, 'Protocol': 0},
          ],
          'CoreBasicItem': <String, dynamic>{}, // EnableCacheFile4Sbox -> true
          'GuiItem': <String, dynamic>{}, // TrayMenuServersLimit -> 20
          'UiItem': <String, dynamic>{}, // CurrentLanguage -> zh-Hans
        },
      );
      await _pumpEditor(tester, host);

      // Upstream `CoreBasicItem.EnableCacheFile4Sbox = true`.
      final cacheBox = tester.widget<Checkbox>(
        find.descendant(
          of: find.ancestor(
            of: find.text('启用 sing-box (规则集文件) 的缓存文件'),
            matching: find.byType(Row),
          ),
          matching: find.byType(Checkbox),
        ),
      );
      expect(
        cacheBox.value,
        isTrue,
        reason: 'missing bool must use the upstream true default',
      );

      // Upstream `GuiItem.TrayMenuServersLimit = 20` (on the v2rayN tab).
      await tester.tap(find.text('v2rayN 设置'));
      await tester.pumpAndSettle();
      expect(
        find.widgetWithText(TextField, '20'),
        findsWidgets,
        reason: 'missing int must use the upstream default 20',
      );

      await tester.tap(find.text('确定'));
      await tester.pumpAndSettle();
      expect(host.saveCalls, 1);
      // The persisted draft must carry the defaulted value, not null/false.
      final savedCore = host.lastDraft!['CoreBasicItem'] as Map;
      expect(savedCore['EnableCacheFile4Sbox'], isTrue);
      expect(savedCore['Loglevel'], 'warning');
    },
  );
}
