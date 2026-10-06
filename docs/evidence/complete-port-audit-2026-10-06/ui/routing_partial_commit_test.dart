import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

r.RoutingProfileDto profile(String id) => r.RoutingProfileDto(id: id, remarks: 'Synthetic $id', url: '', ruleSet: '[]', ruleNum: 0, enabled: true, locked: false, customIcon: '', customRulesetPath4Singbox: '', domainStrategy: '', domainStrategy4Singbox: '', sort: 0, isActive: id == 'a');

class PartialDeleteHost implements RoutingEditorHost, RoutingCommitHost {
  final stored = <String>{'a', 'b'};
  int saveCalls = 0;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async => RoutingEditorSnapshot(schemes: [for (final id in stored) RoutingSchemeSnapshot(profile: profile(id), rules: const [])], domainStrategy: 'AsIs', domainStrategySbox: '', outboundTags: const ['proxy', 'direct', 'block']);

  @override
  Future<RoutingEditorOutcome> commit(String text) async {
    final action = jsonDecode(text) as Map<String, dynamic>;
    if (action['kind'] == 'deleteScheme' && action['id'] == 'b') {
      return const RoutingEditorOutcome(ok: false, message: 'Synthetic failure deleting b');
    }
    stored.remove(action['id']);
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    saveCalls++;
    // Production _applyRoutingDraft also calls save for each stale draft row.
    stored.addAll(draft.schemes.map((s) => s.profile.id));
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {}
}

void main() {
  testWidgets('partial deletion followed by OK cannot resurrect a deleted scheme', (tester) async {
    tester.view.physicalSize = const Size(1400, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final host = PartialDeleteHost();
    await tester.pumpWidget(MaterialApp(home: RoutingEditorWindow(host: host)));
    await tester.pumpAndSettle();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.delete);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('routing-delete-confirm')));
    await tester.pumpAndSettle();
    expect(host.stored, <String>{'b'}, reason: 'first immediate delete already committed');
    debugPrint('AUDIT stale a still rendered: ${find.byKey(const ValueKey('routing-row-a')).evaluate().isNotEmpty}');
    await tester.tap(find.byKey(const ValueKey('routing-ok')));
    await tester.pumpAndSettle();
    debugPrint('AUDIT store after OK: ${host.stored}; full-draft saves: ${host.saveCalls}');
    expect(host.stored.contains('a'), isFalse, reason: 'after an incremental commit, a stale whole-window draft must not restore a row already deleted');
  });
}
