// T21-F real-device test: add a REAL subscription through actual UI clicks
// (top menu -> subscription settings -> add -> save -> update) against the
// real Rust bridge, SQLite and network stack.
//
// The subscription URL is injected at run time and is never stored in the repo:
//   $env:V2RAYN_R_DATA_DIR = <temp>
//   flutter test integration_test/t21f_real_subscription_test.dart -d windows `
//     --dart-define=T21F_SUB_URL=<url>
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/main.dart' as app;

const String _subUrl = String.fromEnvironment('T21F_SUB_URL');

Future<void> _pumpUntil(
  WidgetTester tester,
  bool Function() done, {
  Duration timeout = const Duration(seconds: 30),
  String? reason,
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 200));
    if (done()) return;
  }
  throw TestFailure(reason ?? 'condition not met within $timeout');
}

Future<void> _openMenu(WidgetTester tester, String group, String item) async {
  await tester.tap(find.byKey(ValueKey('menu-$group')));
  await tester.pump(const Duration(milliseconds: 400));
  await tester.tap(find.byKey(ValueKey('menu-item-$item')));
  await tester.pump(const Duration(milliseconds: 400));
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('real subscription survives actual menu clicks + real network',
      (tester) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    expect(dataDir, isNotNull, reason: 'V2RAYN_R_DATA_DIR must be set');
    expect(_subUrl, isNotEmpty, reason: 'pass --dart-define=T21F_SUB_URL');

    await app.main();
    await _pumpUntil(
      tester,
      () => find.byKey(const ValueKey('menu-订阅分组')).evaluate().isNotEmpty,
      reason: 'main window never appeared',
    );

    // 1) Open the subscription settings window through the real top menu.
    await _openMenu(tester, '订阅分组', '订阅分组设置');
    await _pumpUntil(
      tester,
      () => find.byKey(const ValueKey('sub-setting-window')).evaluate().isNotEmpty,
      reason: 'subscription settings window did not open',
    );

    // 2) Add the real subscription: remarks + URL, then save.
    await tester.tap(find.byKey(const ValueKey('sub-add')));
    await tester.pump(const Duration(milliseconds: 500));
    await _pumpUntil(
      tester,
      () => find.byKey(const ValueKey('sub-edit-window')).evaluate().isNotEmpty,
      reason: 'subscription edit window did not open',
    );
    await tester.enterText(
      find.byKey(const ValueKey('sub-field-remarks')),
      'T21F 真实订阅',
    );
    await tester.enterText(
      find.byKey(const ValueKey('sub-field-url')),
      _subUrl,
    );
    await tester.tap(find.byKey(const ValueKey('sub-edit-save')));
    await tester.pump(const Duration(milliseconds: 800));
    await _pumpUntil(
      tester,
      () => find.textContaining('T21F 真实订阅').evaluate().isNotEmpty,
      timeout: const Duration(seconds: 20),
      reason: 'saved subscription row never appeared',
    );

    // 3) Close the settings window and update all subscriptions (direct).
    await tester.tap(find.byKey(const ValueKey('sub-close')));
    await tester.pump(const Duration(milliseconds: 400));
    await _openMenu(tester, '订阅分组', '更新全部订阅 (不通过代理)');

    // 4) Wait for the real download, parse and insert to reach the node table.
    await _pumpUntil(
      tester,
      () =>
          find.textContaining('香港').evaluate().isNotEmpty ||
          find.textContaining('日本').evaluate().isNotEmpty ||
          find.textContaining('新加坡').evaluate().isNotEmpty ||
          find.textContaining('美国').evaluate().isNotEmpty,
      timeout: const Duration(seconds: 120),
      reason: 'real subscription nodes never appeared in the table',
    );

    // The subscription itself must be listed in the settings window too.
    await _openMenu(tester, '订阅分组', '订阅分组设置');
    await _pumpUntil(
      tester,
      () => find.textContaining('T21F 真实订阅').evaluate().isNotEmpty,
      reason: 'subscription missing from settings list after update',
    );
  });
}
