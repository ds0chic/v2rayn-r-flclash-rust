import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/shared/widgets/status_badge.dart';

import 'support/dpi_assertions.dart';
import 'support/profiles_harness.dart';

/// U-FIX (54px overflow family): the status bar must fit 1280/1024/800 logical
/// widths with no RenderFlex overflow. At 800 the captioned selectors shrink
/// (captions ellipsize, faces keep full text under tooltips); chrome text
/// never truncates.
void main() {
  testWidgets('status bar fits 1280/1024/800 without overflow', (tester) async {
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
    container
        .read(uiShellControllerProvider.notifier)
        .setLayout(AppLayoutMode.horizontal);
    for (final width in <double>[1280, 1024, 800]) {
      applyDpi(tester, 1.0, Size(width, 800));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      expectNoLayoutProblems(
        tester,
        'status bar ${width.toInt()}px',
        ignoreText: (text) => true,
      );
      expectChromeNotTruncated(tester, 'status bar ${width.toInt()}px');
    }
    // The group captions stay in the tree at every width.
    expect(find.byKey(const ValueKey('label-sysproxy')), findsOneWidget);
    expect(find.byKey(const ValueKey('label-rule-mode')), findsOneWidget);
    expect(find.byKey(const ValueKey('label-routing-scheme')), findsOneWidget);
  });

  testWidgets('running badge with longest label fits 800/1024/1280', (
    tester,
  ) async {
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
    // Longest badge: running + longest mode word + unapplied suffix, plus a
    // long node summary line (session/ports/pid) for the center. The routing
    // scheme face stays synthetic: it is cap-bounded (160) by construction, so
    // remarks length cannot overflow Row A only the badge/center can.
    container.read(routingControllerProvider.notifier).setRuleMode('Global');
    container.read(runtimeControllerProvider.notifier).state = RuntimeView(
      state: 'Running',
      sessionId: '0123456789ABCDEFGHIJ',
      ports: const <int>[11809, 11810, 11811],
      pid: 12345,
      desiredRevision: BigInt.two,
      appliedRevision: BigInt.one,
    );
    container
        .read(uiShellControllerProvider.notifier)
        .setLayout(AppLayoutMode.horizontal);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    final runtime = container.read(runtimeControllerProvider);
    expect(runtime.isRunning, isTrue);
    expect(runtime.hasUnappliedChanges, isTrue);
    final mode = container.read(routingControllerProvider).ruleMode;
    final badgeLabel = badgeLabelFor(
      badgeKindForRuntime(runtime),
      mode: mode,
      hasUnappliedChanges: runtime.hasUnappliedChanges,
    );
    for (final width in <double>[1280, 1024, 800]) {
      applyDpi(tester, 1.0, Size(width, 800));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      expectNoLayoutProblems(
        tester,
        'running badge ${width.toInt()}px',
        ignoreText: (text) => true,
      );
    }
    // Degradation keeps information: the full label stays in the tooltip and
    // the text node stays in the tree at every width.
    expect(find.byTooltip(badgeLabel), findsOneWidget);
    expect(find.text(badgeLabel), findsOneWidget);
  });
}
