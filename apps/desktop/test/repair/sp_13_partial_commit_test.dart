// SP-13 准备:路由多选删除部分失败 + 读失败的正确失败合同.
//
// 范围声明:本次只做可独立部分.完整接线(权威 snapshot 对账、
// receipt/版本合同)依赖 SP-12(A04 在途),见证据 README.
// 故障注入只用合成数据与内存 fake,不触真实路由/OS.
//
// 正确合同:
// 1. 多选删除部分成功后,已提交的删除不复活,失败项保留,确定不再回写旧全集.
// 2. 读失败(抛异常)不进入可编辑草稿:无确定按钮,只有重试,不调用 save.
// 3. 畸形快照文本不可解码为空草稿,必须抛 RoutingEditorLoadException.
// 4. 取消/关闭不写:已提交操作关闭只 close,不再 save 全量草稿.
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

r.RoutingProfileDto _profile(String id) => r.RoutingProfileDto(
  id: id,
  remarks: 'Synthetic $id',
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

RoutingEditorSnapshot _snapshot(Iterable<String> ids) => RoutingEditorSnapshot(
  schemes: <RoutingSchemeSnapshot>[
    for (final id in ids)
      RoutingSchemeSnapshot(profile: _profile(id), rules: const []),
  ],
  domainStrategy: 'AsIs',
  domainStrategySbox: '',
  outboundTags: const ['proxy', 'direct', 'block'],
);

/// 部分失败 host:删 a 成功,删 b 失败;save 按旧全量语义回写(用于探测复活).
class _PartialFailHost implements RoutingEditorHost, RoutingCommitHost {
  final stored = <String>{'a', 'b'};
  final List<String> draftIdsOnSave = <String>[];
  int saveCalls = 0;
  int closeCalls = 0;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async =>
      _snapshot(stored.toList()..sort());

  @override
  Future<RoutingEditorOutcome> commit(String text) async {
    final action = jsonDecode(text) as Map<String, dynamic>;
    if (action['kind'] == 'deleteScheme') {
      if (action['id'] == 'b') {
        return const RoutingEditorOutcome(
          ok: false,
          message: 'Synthetic 删 b 失败',
        );
      }
      stored.remove(action['id']);
      return const RoutingEditorOutcome(ok: true);
    }
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    saveCalls++;
    draftIdsOnSave
      ..clear()
      ..addAll(draft.schemes.map((s) => s.profile.id));
    stored.addAll(draftIdsOnSave);
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

class _ThrowingHost implements RoutingEditorHost {
  int saveCalls = 0;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async =>
      throw const RoutingEditorLoadException('Synthetic 读取失败');

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    saveCalls++;
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {}
}

Future<void> _pump(WidgetTester tester, RoutingEditorHost host) async {
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(MaterialApp(home: RoutingEditorWindow(host: host)));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('SP-13:部分删除后确定不复活已删规则', (tester) async {
    final host = _PartialFailHost();
    await _pump(tester, host);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.delete);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('routing-delete-confirm')));
    await tester.pumpAndSettle();

    // a 已在后端删除,b 因失败保留.
    expect(host.stored, <String>{'b'});

    await tester.tap(find.byKey(const ValueKey('routing-ok')));
    await tester.pumpAndSettle();

    expect(
      host.stored.contains('a'),
      isFalse,
      reason: '增量提交成功后,确定不得用旧全集复活已删规则',
    );
    expect(host.stored, <String>{'b'});
    if (host.saveCalls > 0) {
      expect(
        host.draftIdsOnSave.contains('a'),
        isFalse,
        reason: '确定回写的草稿不得包含已删规则',
      );
    }
    // 失败项 b 仍可见,可重试.
    expect(find.byKey(const ValueKey('routing-row-b')), findsOneWidget);
    expect(find.byKey(const ValueKey('routing-row-a')), findsNothing);
  });

  testWidgets('SP-13:读失败不进入可编辑草稿', (tester) async {
    final host = _ThrowingHost();
    await _pump(tester, host);

    expect(find.byKey(const ValueKey('routing-load-error')), findsOneWidget);
    expect(find.byKey(const ValueKey('routing-load-retry')), findsOneWidget);
    expect(find.byKey(const ValueKey('routing-ok')), findsNothing);
    expect(find.byKey(const ValueKey('routing-cancel')), findsNothing);
    expect(host.saveCalls, 0);
  });

  test('SP-13:畸形快照不得解码为空草稿', () {
    expect(
      () => decodeRoutingSnapshot('not json{{'),
      throwsA(isA<RoutingEditorLoadException>()),
    );
    expect(
      () => decodeRoutingSnapshot('{"schemes":"oops"}'),
      throwsA(isA<RoutingEditorLoadException>()),
    );
    expect(
      () => decodeRoutingSnapshot('[1,2]'),
      throwsA(isA<RoutingEditorLoadException>()),
    );
  });

  testWidgets('SP-13:取消已提交操作不补写全量草稿', (tester) async {
    final host = _PartialFailHost();
    await _pump(tester, host);

    // 只删 a(成功),然后取消关闭.
    await tester.tap(find.byKey(const ValueKey('routing-row-a')));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.delete);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('routing-delete-confirm')));
    await tester.pumpAndSettle();
    expect(host.stored, <String>{'b'});

    await tester.tap(find.byKey(const ValueKey('routing-cancel')));
    await tester.pumpAndSettle();
    expect(host.closeCalls, 1);
    expect(host.saveCalls, 0, reason: '取消不得触发全量 save;已提交的删除保持,不假称未改变');
    expect(host.stored, <String>{'b'});
  });
}
