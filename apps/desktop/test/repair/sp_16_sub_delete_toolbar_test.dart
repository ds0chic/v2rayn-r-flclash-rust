// SP-16 主窗工具栏删除入口端到端（合成数据专用，独立文件、单次整壳构建）。
//
// flutter_tester 在单进程多次整页构建后崩溃（见 test/support/profiles_harness.dart
// 注记），故整壳 MainShell 测试独占一文件。内容：工具栏 `toolbar-sub-delete`
// 直达当前组 G；取消保留一切；确认删当前组回 All。
// 不读用户秘密，不触网络/10808。
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import '../support/fake_monitor_bridge.dart';
import '../support/fake_platform_bridge.dart';

c.SubItemDto synSub(String remarks) => c.SubItemDto(
  id: '',
  remarks: remarks,
  url: 'https://example.com/${remarks.hashCode & 0xffff}',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 0,
  autoUpdateInterval: 0,
  updateTime: 0,
);

void main() {
  testWidgets('toolbar delete confirms, cancel keeps, ok falls back to All', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = SyntheticBridgePort();
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(50),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      ],
    );
    addTearDown(container.dispose);
    final subs = container.read(subsControllerProvider.notifier);
    subs.save(synSub('A'));
    subs.save(synSub('B'));
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: MainShell()),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));

    final deleteBtn = find.byKey(const ValueKey('toolbar-sub-delete'));
    expect(deleteBtn, findsOneWidget);

    final aId = container
        .read(subsControllerProvider)
        .items
        .firstWhere((s) => s.remarks == 'A')
        .id;
    expect(
      container.read(profilesControllerProvider.notifier).setGroupSubId(aId),
      isTrue,
    );
    await tester.pump();

    // 取消：组与订阅均保留。
    await tester.tap(deleteBtn);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-delete-confirm')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('sub-delete-cancel')));
    await tester.pumpAndSettle();
    expect(container.read(profilesControllerProvider).groupSubId, aId);
    expect(bridge.getSubItem(aId), isNotNull);

    // 确认：删当前组回 All。
    await tester.tap(deleteBtn);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-delete-confirm')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('sub-delete-confirm-ok')));
    await tester.pumpAndSettle();
    expect(bridge.getSubItem(aId), isNull);
    expect(container.read(profilesControllerProvider).groupSubId, isNull);
  });
}
