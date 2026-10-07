// SP-16 主窗分组右键菜单（合成数据专用，独立文件、单次整壳构建）。
//
// flutter_tester 在单进程多次整页构建后崩溃（见 test/support/profiles_harness.dart
// 注记），故整壳 MainShell 测试独占一文件。内容：分组 chip 右键弹出上游
// `ProfilesView.axaml` 组 ListBox.ContextMenu 对等的三项（编辑/新增/删除），
// 其中删除走与工具栏 `toolbar-sub-delete` 同一 `deleteCurrentSub` 入口与门控；
// 取消保留一切，确认删当前组回 All；All 视图右键删除直接门控（不弹确认框）。
// 不读用户秘密，不触网络/10808。
import 'package:flutter/gestures.dart';
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
  testWidgets('group chip right-click menu mirrors toolbar delete gating', (
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

    // 右键当前组 chip：弹出三项菜单（与上游组 ListBox.ContextMenu 对等）。
    final chipCenter = tester.getCenter(
      find.byKey(ValueKey('group-filter-$aId')),
    );
    await tester.tapAt(chipCenter, buttons: kSecondaryButton);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-chip-menu-edit')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-chip-menu-add')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('group-chip-menu-delete')),
      findsOneWidget,
    );

    // 菜单删除先确认：取消则组与订阅均保留。
    await tester.tap(find.byKey(const ValueKey('group-chip-menu-delete')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-delete-confirm')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('sub-delete-cancel')));
    await tester.pumpAndSettle();
    expect(container.read(profilesControllerProvider).groupSubId, aId);
    expect(bridge.getSubItem(aId), isNotNull);

    // 确认则删当前组回 All（与工具栏入口同一语义）。
    await tester.tapAt(chipCenter, buttons: kSecondaryButton);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-chip-menu-delete')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('sub-delete-confirm-ok')));
    await tester.pumpAndSettle();
    expect(bridge.getSubItem(aId), isNull);
    expect(container.read(profilesControllerProvider).groupSubId, isNull);

    // All 视图右键删除直接门控：不弹确认框，无操作。
    final allCenter = tester.getCenter(
      find.byKey(const ValueKey('group-filter-all')),
    );
    await tester.tapAt(allCenter, buttons: kSecondaryButton);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-chip-menu-delete')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-delete-confirm')), findsNothing);
    expect(container.read(subsControllerProvider).items.length, 1);
  });
}
