import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/onboarding/onboarding_store.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/shared/widgets/danger_confirm.dart';
import 'package:v2rayn_desktop/shared/widgets/undo_snackbar.dart';

import 'support/profiles_harness.dart';

Future<void> _pumpDangerButton(
  WidgetTester tester,
  MemoryUiStateStore store,
  ValueChanged<bool> onResult,
) async {
  final container = ProviderContainer(
    overrides: [uiStateStoreProvider.overrideWithValue(store)],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Consumer(
          builder: (context, ref, _) => TextButton(
            key: const ValueKey('open-danger'),
            onPressed: () async {
              onResult(
                await showDangerConfirm(
                  context,
                  ref,
                  title: '危险操作',
                  target: '目标：测试项',
                  impact: '影响：测试影响说明。',
                  confirmLabel: '执行',
                  dontAskKey: 'test-gate',
                  dialogKey: const ValueKey('test-danger-confirm'),
                ),
              );
            },
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('danger confirm executes and suppresses on demand', (
    tester,
  ) async {
    final store = MemoryUiStateStore();
    var result = false;
    await _pumpDangerButton(tester, store, (value) => result = value);

    await tester.tap(find.byKey(const ValueKey('open-danger')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('test-danger-confirm')), findsOneWidget);
    // Opt into "don't ask again", then confirm.
    await tester.tap(find.byKey(const ValueKey('test-gate-checkbox')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('danger-confirm-ok')));
    await tester.pumpAndSettle();
    expect(result, isTrue);
    expect(OnboardingHints.isSuppressed(store, 'test-gate'), isTrue);

    // Second call passes straight through with no dialog.
    result = false;
    await tester.tap(find.byKey(const ValueKey('open-danger')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('test-danger-confirm')), findsNothing);
    expect(result, isTrue);
  });

  testWidgets('danger confirm cancel is side-effect free', (tester) async {
    final store = MemoryUiStateStore();
    var result = true;
    await _pumpDangerButton(tester, store, (value) => result = value);

    await tester.tap(find.byKey(const ValueKey('open-danger')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('danger-confirm-cancel')));
    await tester.pumpAndSettle();
    expect(result, isFalse);
    expect(OnboardingHints.isSuppressed(store, 'test-gate'), isFalse);
  });

  testWidgets('enabling TUN asks first; cancel leaves it off', (tester) async {
    await pumpApp(tester, rows: 5);
    final toggle = find.byKey(const ValueKey('tun-toggle'));
    expect(tester.widget<Switch>(toggle).value, isFalse);
    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('tun-enable-confirm')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('danger-confirm-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('tun-enable-confirm')), findsNothing);
    expect(tester.widget<Switch>(toggle).value, isFalse);
  });

  testWidgets('writing the host proxy asks first; cancel is a no-op', (
    tester,
  ) async {
    await pumpApp(tester, rows: 5);
    await tester.tap(find.byKey(const ValueKey('system-proxy-selector')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('自动配置系统代理'));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('sysproxy-apply-confirm')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('danger-confirm-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sysproxy-apply-confirm')), findsNothing);
  });

  testWidgets('undo snackbar fires the callback within its window', (
    tester,
  ) async {
    var undone = false;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              key: const ValueKey('open-undo'),
              onPressed: () => showUndoSnackbar(
                context,
                message: '已删除 2 个节点',
                onUndo: () => undone = true,
              ),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('open-undo')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('undo-snackbar-action')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('undo-snackbar-action')));
    await tester.pumpAndSettle();
    expect(undone, isTrue);
  });
}
