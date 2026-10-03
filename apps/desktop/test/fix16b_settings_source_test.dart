import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

ProviderContainer _container(SyntheticBridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(4),
  ],
);

Future<void> _pumpWindow(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
    ),
  );
  await tester.pump();
  await tester.pumpAndSettle();
}

Map<String, dynamic> _saved(SyntheticBridgePort bridge) =>
    jsonDecode(bridge.getSettings().settingsJson) as Map<String, dynamic>;

Future<void> _enter(WidgetTester tester, String key, String value) async {
  final field = find.byKey(ValueKey<String>(key));
  await tester.ensureVisible(field);
  await tester.pumpAndSettle();
  await tester.enterText(
    find.descendant(of: field, matching: find.byType(TextField)),
    value,
  );
  await tester.pump();
}

void main() {
  testWidgets('FIX-16B: source URLs save then reopen', (tester) async {
    tester.view.physicalSize = const Size(1200, 1000);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final bridge = SyntheticBridgePort(count: 4);
    final container = _container(bridge);
    addTearDown(container.dispose);

    await _pumpWindow(tester, container);
    await tester.tap(find.text('显示'));
    await tester.pumpAndSettle();

    const subConvert = 'https://convert.example/sub?url={0}';
    const geo = 'https://mirror.example/geo/{0}.dat';
    const srs = 'https://mirror.example/srs/{0}/{1}.srs';
    const routeRules = 'https://mirror.example/routing/template.json';

    await _enter(tester, 'settings-sub-convert-url', subConvert);
    await _enter(tester, 'settings-geo-source-url', geo);
    await _enter(tester, 'settings-srs-source-url', srs);
    await _enter(tester, 'settings-route-rules-source-url', routeRules);

    await tester.tap(find.byKey(const ValueKey('settings-save')));
    await tester.pumpAndSettle();

    final doc = _saved(bridge);
    final constItem = doc['ConstItem'] as Map<String, dynamic>;
    expect(constItem['SubConvertUrl'], subConvert);
    expect(constItem['GeoSourceUrl'], geo);
    expect(constItem['SrsSourceUrl'], srs);
    expect(constItem['RouteRulesTemplateSourceUrl'], routeRules);
    expect(bridge.settingsRevision(), 1);

    // Reopen against the same store: values are restored from the document.
    await tester.pumpWidget(const SizedBox());
    await tester.pumpAndSettle();
    final reopen = _container(bridge);
    addTearDown(reopen.dispose);
    await _pumpWindow(tester, reopen);
    await tester.tap(find.text('显示'));
    await tester.pumpAndSettle();

    expect(reopen.read(settingsControllerProvider).loaded, isTrue);
    final restored =
        reopen.read(settingsControllerProvider).document['ConstItem']
            as Map<String, dynamic>;
    expect(restored['SubConvertUrl'], subConvert);
    expect(restored['GeoSourceUrl'], geo);
    expect(restored['SrsSourceUrl'], srs);
    expect(restored['RouteRulesTemplateSourceUrl'], routeRules);
  });
}
