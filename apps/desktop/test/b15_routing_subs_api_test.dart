// B15-DART-ROUTING-SUBS: tests for newly added helpers (fail to compile
// until the fix lands: buildExportFilePath, isPrivateNetworkHost, and the
// http-warning + three-segment rule ids).
// D3a-03 export filename, D3a-06 http warning, D3a-08 rule-id shape.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/subs/sub_edit_window.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

import 'support/subs_harness.dart';

void main() {
  // D3a-03: export filenames are unguessable and stay in the temp dir.
  test('export file path carries randomness, temp dir and txt suffix', () {
    final a = buildExportFilePath('/tmp', 1234567890, 'abcd');
    final b = buildExportFilePath('/tmp', 1234567890, 'ef01');
    expect(a, isNot(equals(b)));
    expect(a.startsWith('/tmp'), isTrue);
    expect(a.endsWith('.txt'), isTrue);
    expect(a, contains('1234567890'));
    expect(a, contains('abcd'));
  });

  // D3a-06: http URL warns (non-blocking) and the save still proceeds.
  testWidgets('http subscription URL warns but still saves', (tester) async {
    final container = makeSubsContainer();
    addTearDown(container.dispose);
    final draft = container.read(subsControllerProvider.notifier).newDraft();
    c.SubItemDto? result;
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: Builder(
              builder: (context) => TextButton(
                key: const ValueKey('b15-open-edit'),
                onPressed: () async {
                  result = await showSubEditWindow(context, draft);
                },
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('b15-open-edit')));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byKey(const ValueKey('sub-field-remarks')),
      'b15-http',
    );
    await tester.enterText(
      find.byKey(const ValueKey('sub-field-url')),
      'http://203.0.2.1/sub',
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('sub-edit-save')));
    await tester.pump();

    expect(
      find.text('该订阅使用 http 明文传输，请注意安全'),
      findsNWidgets(2),
      reason: 'persistent banner plus the save-time snackbar',
    );
    expect(result, isNotNull);
    expect(result!.url, 'http://203.0.2.1/sub');
  });

  test('private-network host classification', () {
    expect(isPrivateNetworkHost('127.0.0.1'), isTrue);
    expect(isPrivateNetworkHost('10.0.0.5'), isTrue);
    expect(isPrivateNetworkHost('172.16.5.4'), isTrue);
    expect(isPrivateNetworkHost('172.31.255.255'), isTrue);
    expect(isPrivateNetworkHost('172.32.0.1'), isFalse);
    expect(isPrivateNetworkHost('192.168.1.1'), isTrue);
    expect(isPrivateNetworkHost('169.254.10.20'), isTrue);
    expect(isPrivateNetworkHost('::1'), isTrue);
    expect(isPrivateNetworkHost('8.8.8.8'), isFalse);
    expect(isPrivateNetworkHost('203.0.2.1'), isFalse);
    expect(isPrivateNetworkHost('example.com'), isFalse);
    expect(isPrivateNetworkHost(''), isFalse);
  });

  // D3a-08: rule ids carry a random segment (cross-isolate uniqueness).
  test('new rule ids are unique and carry three segments', () {
    final ids = <String>{
      for (var i = 0; i < 200; i++) RoutingController.newRuleId(),
    };
    expect(ids.length, 200);
    for (final id in ids) {
      expect(id.split('-').length, 3);
    }
  });
}
