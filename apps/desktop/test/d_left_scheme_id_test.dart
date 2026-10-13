// D-LEFT D3a-08 (not changed, reason documented): the independent scheme
// editor keeps minting a non-empty client `rt-` id.
//
// The Rust side would allocate safely for `''` (`normalize_routing` assigns
// `new_routing_id()`, crates/application/src/routing.rs:75-77), but the Dart
// SP-13 relay verifies the commit by re-querying the authoritative list for
// the exact relayed id (`verifyRoutingIncrement` `saveScheme` branch,
// routing_actions.dart:121-123). An empty id would fail verification after a
// successful save, and a user retry would duplicate the row. The backend keeps
// non-empty client ids, so the client-minted id stays consistent end to end.
// This test locks that contract: a regression to `''` fails here, not in
// production verification.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

void main() {
  testWidgets('new scheme carries a non-empty client id for SP-13 verify', (
    tester,
  ) async {
    RoutingSchemeSnapshot? result;
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => TextButton(
            onPressed: () async {
              result = await showRoutingSchemeEditor(
                context,
                null,
                const <String>['proxy'],
              );
            },
            child: const Text('open'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('ruleset-remarks')),
      'new scheme',
    );
    await tester.tap(find.byKey(const ValueKey('ruleset-save')));
    await tester.pumpAndSettle();

    expect(result, isNotNull);
    expect(result!.profile.id, isNotEmpty);
    expect(result!.profile.id, startsWith('rt-'));
    expect(result!.profile.remarks, 'new scheme');
  });
}
