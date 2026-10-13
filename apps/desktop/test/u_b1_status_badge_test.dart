import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/shared/widgets/status_badge.dart';

import 'support/profiles_harness.dart';

void main() {
  test('badge kind follows the runtime snapshot, failure wins', () {
    expect(badgeKindForRuntime(const RuntimeView()), StatusBadgeKind.stopped);
    expect(
      badgeKindForRuntime(const RuntimeView(state: 'Running')),
      StatusBadgeKind.running,
    );
    expect(
      badgeKindForRuntime(const RuntimeView(commandPending: true)),
      StatusBadgeKind.connecting,
    );
    expect(
      badgeKindForRuntime(const RuntimeView(state: 'Starting')),
      StatusBadgeKind.connecting,
    );
    const failed = RuntimeView(
      state: 'Running',
      error: RuntimeErrorView(
        code: 'E_TUN_HELPER_UNAVAILABLE',
        messageKey: 'error.tun_apply_failed',
      ),
    );
    expect(badgeKindForRuntime(failed), StatusBadgeKind.failed);
  });

  test('badge labels carry mode and unapplied suffix', () {
    expect(
      badgeLabelFor(StatusBadgeKind.running, mode: 'Rule'),
      contains('Rule'),
    );
    expect(
      badgeLabelFor(
        StatusBadgeKind.running,
        mode: 'Rule',
        hasUnappliedChanges: true,
      ),
      contains('未应用'),
    );
    expect(badgeLabelFor(StatusBadgeKind.stopped), '未启动');
    expect(badgeLabelFor(StatusBadgeKind.failed), contains('查看'));
  });

  testWidgets('status bar shows the stopped badge with no session', (
    tester,
  ) async {
    await pumpApp(tester, rows: 5);
    final badge = find.byKey(const ValueKey('status-badge'));
    expect(badge, findsOneWidget);
    expect(
      find.descendant(of: badge, matching: find.text('未启动')),
      findsOneWidget,
    );
  });

  testWidgets('badge icons differ per kind (no color-only signal)', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: Column(
            children: <Widget>[
              StatusBadge(kind: StatusBadgeKind.running, label: 'a'),
              StatusBadge(kind: StatusBadgeKind.stopped, label: 'b'),
              StatusBadge(kind: StatusBadgeKind.failed, label: 'c'),
              StatusBadge(kind: StatusBadgeKind.connecting, label: 'd'),
            ],
          ),
        ),
      ),
    );
    await tester.pump();
    final icons = tester
        .widgetList<Icon>(find.byType(Icon))
        .map((icon) => icon.icon)
        .toSet();
    // Three static icons differ; connecting renders a spinner instead.
    expect(icons.length, 3);
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
  });

  testWidgets('failed badge tap fires, other states stay display-only', (
    tester,
  ) async {
    var tapped = false;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Column(
            children: <Widget>[
              StatusBadge(
                kind: StatusBadgeKind.failed,
                label: 'x',
                onTap: () => tapped = true,
              ),
              const StatusBadge(kind: StatusBadgeKind.running, label: 'y'),
            ],
          ),
        ),
      ),
    );
    await tester.pump();
    expect(find.byType(InkWell), findsOneWidget);
    await tester.tap(find.text('x'));
    await tester.pump();
    expect(tapped, isTrue);
  });
}
