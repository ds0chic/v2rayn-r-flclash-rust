// R4-12 窗口和三布局 contract tests.
//
// Covers the card's must-pass scenarios with synthetic, in-memory fakes only:
//  * the three main layouts (horiz/vert/tab) keep the table/menu/status-bar
//    structure and restore from the UiItem orientation state source;
//  * a failed/empty host read never degrades into a submittable empty draft;
//  * the original routing sub-editor/immediate commit is not undone by the
//    whole-window 取消.
//
// No native library, core, port, system proxy/route/TUN or real window is
// touched.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';

import 'support/profiles_harness.dart';

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

class _FailingSettingsHost implements SettingsEditorHost {
  _FailingSettingsHost({this.empty = false});

  final bool empty;
  int saveCalls = 0;
  int closeCalls = 0;

  @override
  Future<Map<String, dynamic>> loadSnapshot() async {
    if (empty) return <String, dynamic>{};
    throw const SettingsEditorLoadException('读取配置失败');
  }

  @override
  Future<SettingsEditorOutcome> save(Map<String, dynamic> draft) async {
    saveCalls++;
    return const SettingsEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

class _RecordingRoutingHost implements RoutingEditorHost, RoutingCommitHost {
  _RecordingRoutingHost(this.snapshot);

  final RoutingEditorSnapshot snapshot;
  int saveCalls = 0;
  int closeCalls = 0;
  int commitCalls = 0;
  final List<String> actions = <String>[];
  RoutingDraft? lastDraft;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async => snapshot;

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    saveCalls++;
    lastDraft = draft;
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }

  @override
  Future<RoutingEditorOutcome> commit(String actionJson) async {
    commitCalls++;
    actions.add(actionJson);
    return const RoutingEditorOutcome(ok: true);
  }
}

class _FailingRoutingHost implements RoutingEditorHost {
  int saveCalls = 0;
  int closeCalls = 0;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async =>
      throw const RoutingEditorLoadException('读取路由设置失败');

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    saveCalls++;
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

r.RoutingProfileDto _profile(String id, String remarks) => r.RoutingProfileDto(
  id: id,
  remarks: remarks,
  url: '',
  ruleSet: '[]',
  ruleNum: 0,
  enabled: true,
  locked: false,
  customIcon: '',
  customRulesetPath4Singbox: '',
  domainStrategy: '',
  domainStrategy4Singbox: '',
  sort: 0,
  isActive: id == 'a',
);

RoutingEditorSnapshot _routingSnapshot() => RoutingEditorSnapshot(
  schemes: <RoutingSchemeSnapshot>[
    RoutingSchemeSnapshot(profile: _profile('a', 'V4-绕过大陆'), rules: const []),
    RoutingSchemeSnapshot(profile: _profile('b', 'V4-全局'), rules: const []),
  ],
  domainStrategy: 'AsIs',
  domainStrategySbox: '',
  outboundTags: const <String>['proxy', 'direct', 'block'],
);

Future<void> _pumpRouting(WidgetTester tester, RoutingEditorHost host) async {
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(MaterialApp(home: RoutingEditorWindow(host: host)));
  await tester.pumpAndSettle();
}

ProviderContainer _shellContainer(UiStateStore store) => ProviderContainer(
  overrides: [uiStateStoreProvider.overrideWithValue(store)],
);

void main() {
  // -- Contract 1: three layouts ------------------------------------------
  group('R4-12 three main layouts', () {
    test('layout mode persists and is restored after reopen', () {
      final store = MemoryUiStateStore();
      final container = _shellContainer(store);
      addTearDown(container.dispose);
      final notifier = container.read(uiShellControllerProvider.notifier);

      for (final mode in AppLayoutMode.values) {
        notifier.setLayout(mode);
        expect(container.read(uiShellControllerProvider).layout, mode);

        final reopened = _shellContainer(store);
        addTearDown(reopened.dispose);
        expect(
          reopened.read(uiShellControllerProvider).layout,
          mode,
          reason: 'reopen should keep ${mode.id}',
        );
      }
    });

    test('UiItem.MainGirdOrientation is the layout state source', () {
      final container = _shellContainer(MemoryUiStateStore());
      addTearDown(container.dispose);
      final notifier = container.read(uiShellControllerProvider.notifier);

      void apply(int orientation) =>
          notifier.applySettingsDocument(<String, dynamic>{
            'UiItem': <String, dynamic>{'MainGirdOrientation': orientation},
            'GuiItem': <String, dynamic>{},
          });

      apply(0);
      expect(
        container.read(uiShellControllerProvider).layout,
        AppLayoutMode.horizontal,
      );
      apply(1);
      expect(
        container.read(uiShellControllerProvider).layout,
        AppLayoutMode.vertical,
      );
      apply(2);
      expect(
        container.read(uiShellControllerProvider).layout,
        AppLayoutMode.tab,
      );
    });

    testWidgets('main shell keeps table, menu and status bar in each layout', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1600, 1000);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final container = makeContainer(rows: 6);
      addTearDown(container.dispose);
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(home: MainShell()),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));

      final shell = container.read(uiShellControllerProvider.notifier);
      for (final mode in AppLayoutMode.values) {
        shell.setLayout(mode);
        await tester.pump();

        // Menu (top) and status bar (bottom) live outside the grid in every
        // layout (MainWindow.xaml LAY-MAIN-004 / LAY-MAIN-006).
        expect(
          find.byKey(const ValueKey('menu-配置项')),
          findsOneWidget,
          reason: 'menu missing in ${mode.id}',
        );
        expect(
          find.byKey(const ValueKey('status-inbound')),
          findsOneWidget,
          reason: 'status bar missing in ${mode.id}',
        );

        switch (mode) {
          case AppLayoutMode.horizontal:
            expect(
              find.byKey(const ValueKey('split-horizontal')),
              findsOneWidget,
            );
            expect(
              find.byKey(const ValueKey('main-tab-content-info')),
              findsOneWidget,
            );
          case AppLayoutMode.vertical:
            expect(
              find.byKey(const ValueKey('split-vertical')),
              findsOneWidget,
            );
            expect(
              find.byKey(const ValueKey('main-tab-content-info')),
              findsOneWidget,
            );
          case AppLayoutMode.tab:
            // Tab layout: profiles is the first tab, no splitter.
            expect(
              find.byKey(const ValueKey('main-tab-content-profiles')),
              findsOneWidget,
            );
            expect(
              find.byKey(const ValueKey('split-horizontal')),
              findsNothing,
            );
            expect(find.byKey(const ValueKey('split-vertical')), findsNothing);
        }
      }
    });
  });

  // -- Contract 3: failed read must not submit an empty draft -------------
  group('R4-12 read failure is not an empty draft', () {
    testWidgets('settings: throwing load shows an error and no 确定', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1200, 900);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final host = _FailingSettingsHost();
      await tester.pumpWidget(
        MaterialApp(home: OptionSettingWindow(host: host, standalone: true)),
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const ValueKey('settings-load-error')), findsOneWidget);
      expect(find.byKey(const ValueKey('settings-load-retry')), findsOneWidget);
      expect(find.text('确定'), findsNothing);
      expect(host.saveCalls, 0);
    });

    testWidgets('settings: empty snapshot is treated as a failed read', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1200, 900);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final host = _FailingSettingsHost(empty: true);
      await tester.pumpWidget(
        MaterialApp(home: OptionSettingWindow(host: host, standalone: true)),
      );
      await tester.pumpAndSettle();

      expect(find.byKey(const ValueKey('settings-load-error')), findsOneWidget);
      expect(find.text('确定'), findsNothing);
      expect(host.saveCalls, 0);
    });

    testWidgets('routing: throwing load shows an error and no 确定', (
      tester,
    ) async {
      final host = _FailingRoutingHost();
      await _pumpRouting(tester, host);

      expect(find.byKey(const ValueKey('routing-load-error')), findsOneWidget);
      expect(find.byKey(const ValueKey('routing-load-retry')), findsOneWidget);
      expect(find.byKey(const ValueKey('routing-ok')), findsNothing);
      expect(host.saveCalls, 0);
    });
  });

  // -- Contract 4: original sub/immediate commit is not overridden --------
  group('R4-12 sub-editor/immediate commit survives whole-window cancel', () {
    testWidgets('sub-editor 确定 commits immediately; 取消 keeps it', (
      tester,
    ) async {
      final host = _RecordingRoutingHost(_routingSnapshot());
      await _pumpRouting(tester, host);

      await tester.tap(find.byKey(const ValueKey('routing-add')));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const ValueKey('ruleset-remarks')),
        '新增方案',
      );
      await tester.tap(find.byKey(const ValueKey('ruleset-save')));
      await tester.pumpAndSettle();

      // The sub-editor 确定 persisted its own action; nothing waits for 确定.
      expect(host.commitCalls, 1);
      expect(host.actions.first, contains('"kind":"saveScheme"'));
      expect(host.saveCalls, 0);

      // The whole-window 取消 only closes; the committed sub-edit stays.
      await tester.tap(find.byKey(const ValueKey('routing-cancel')));
      await tester.pumpAndSettle();
      expect(host.closeCalls, 1);
      expect(host.saveCalls, 0);
    });

    testWidgets('strategy change commits immediately', (tester) async {
      final host = _RecordingRoutingHost(_routingSnapshot());
      await _pumpRouting(tester, host);

      await tester.tap(find.byKey(const ValueKey('routing-domain-strategy')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('UseIP').last);
      await tester.pumpAndSettle();

      expect(host.commitCalls, 1);
      expect(host.actions.first, contains('"kind":"strategy"'));
      expect(host.saveCalls, 0);
    });
  });
}
