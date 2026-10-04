import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'fake_monitor_bridge.dart';
import 'fake_platform_bridge.dart';

/// Shared WidgetTester harness for the T01/T05 profiles table interaction
/// tests. It now pumps the full [MainShell] so the tests exercise the real
/// menu/layout/status-bar composition.
///
/// The locked Flutter build leaks native resources per `pumpWidget`, so the
/// flutter_tester process segfaults after a handful of page builds. Tests are
/// therefore split into small files with a single page build each; this file is
/// intentionally not named `*_test.dart` so it is never run as a suite.
ProviderContainer makeContainer({int rows = 2000}) {
  return ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(rows),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
    ],
  );
}

Future<ProviderContainer> pumpApp(
  WidgetTester tester, {
  int rows = 2000,
  double width = 1440,
  double height = 900,
  bool setViewSize = true,
  MemoryUiStateStore? store,
  FakeMonitorBridge? monitor,
  bool? dragDropSort,
}) async {
  if (setViewSize) {
    tester.view.physicalSize = Size(width, height);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
  }

  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(store ?? MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(rows),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(monitor ?? FakeMonitorBridge()),
      if (dragDropSort != null)
        profilesEnableDragDropSortProvider.overrideWithValue(dragDropSort),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: MainShell()),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return container;
}

ProfilesState readState(ProviderContainer container) =>
    container.read(profilesControllerProvider);

Future<void> pressWithCtrl(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

Future<void> pressPlain(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyEvent(key);
  await tester.pump();
}

Future<void> tapRow(WidgetTester tester, Key key) async {
  await tester.tap(find.byKey(key));
  // onTap waits out the double-tap timeout because onDoubleTap is registered.
  await tester.pump(const Duration(milliseconds: 400));
}

Future<void> pressWithCtrlTap(WidgetTester tester, Key key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.tap(find.byKey(key));
  await tester.pump(const Duration(milliseconds: 400));
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

Future<void> doubleTap(WidgetTester tester, Finder finder) async {
  await tester.tap(finder);
  await tester.pump(const Duration(milliseconds: 40));
  await tester.tap(finder);
  await tester.pump(const Duration(milliseconds: 400));
}
