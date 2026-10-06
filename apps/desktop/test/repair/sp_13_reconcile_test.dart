// SP-13 完整接线：增量提交权威对账 + 确定仅策略字段 + 取消/重开 + DNS 读失败。
//
// 合成数据；内存 fake；不触真实路由/OS/DNS/网络。写锁内文件：
// features/routing/** + 本卡测试。settings_controller/IPC/BridgePort/FRB 均未碰。
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/dns_controller.dart';
import 'package:v2rayn_desktop/features/routing/dns_window.dart';
import 'package:v2rayn_desktop/features/routing/routing_actions.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../support/fake_platform_bridge.dart';
import '../support/synthetic_runtime_bridge.dart';

r.RoutingProfileDto _profile(String id, {bool active = false}) =>
    r.RoutingProfileDto(
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
      isActive: active,
    );

RoutingEditorSnapshot _snapshot(Iterable<String> ids) => RoutingEditorSnapshot(
  schemes: <RoutingSchemeSnapshot>[
    for (final id in ids)
      RoutingSchemeSnapshot(
        profile: _profile(id, active: id == 'a'),
        rules: const [],
      ),
  ],
  domainStrategy: 'AsIs',
  domainStrategySbox: '',
  outboundTags: const ['proxy', 'direct', 'block'],
);

/// 记录确定草稿的增量窗 host（删 a 成功）。
class _CommitHost implements RoutingEditorHost, RoutingCommitHost {
  RoutingDraft? lastDraft;
  int closeCalls = 0;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async => _snapshot(['a', 'b']);

  @override
  Future<RoutingEditorOutcome> commit(String text) async =>
      const RoutingEditorOutcome(ok: true);

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    lastDraft = draft;
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

/// 无增量能力的旧 host：确定走全量回放。
class _LegacyHost implements RoutingEditorHost {
  RoutingDraft? lastDraft;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async => _snapshot(['a', 'b']);

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    lastDraft = draft;
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {}
}

/// 部分失败 host：删 a 成功、删 b 失败（重开无复活用）。
class _PartialFailHost implements RoutingEditorHost, RoutingCommitHost {
  final stored = <String>{'a', 'b'};

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
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async =>
      const RoutingEditorOutcome(ok: true);

  @override
  Future<void> close() async {}
}

Future<void> _pumpRouting(WidgetTester tester, RoutingEditorHost host) async {
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(MaterialApp(home: RoutingEditorWindow(host: host)));
  await tester.pumpAndSettle();
}

ProviderContainer _dnsContainer() {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(4),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      runtimeBridgeProvider.overrideWithValue(SyntheticRuntimeBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Future<void> _pumpDns(WidgetTester tester, ProviderContainer container) async {
  tester.view.physicalSize = const Size(1280, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Scaffold(
          body: Consumer(
            builder: (context, ref, _) => TextButton(
              key: const ValueKey('sp13-open-dns'),
              onPressed: () => showDnsSettingWindow(context, ref),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  test('SP-13:增量窗草稿带已提交标记，旧窗草稿保持全量语义', () {
    final incremental = encodeRoutingDraft(
      RoutingDraft(
        schemes: _snapshot(['a']).schemes,
        domainStrategy: 'AsIs',
        domainStrategySbox: '',
        incrementalCommitted: true,
      ),
    );
    final decoded = decodeRoutingDraft(incremental)!;
    expect(decoded.incrementalCommitted, isTrue);

    final legacy = encodeRoutingDraft(
      RoutingDraft(
        schemes: _snapshot(['a']).schemes,
        domainStrategy: 'AsIs',
        domainStrategySbox: '',
      ),
    );
    expect(decodeRoutingDraft(legacy)!.incrementalCommitted, isFalse);
  });

  test('SP-13:增量确定不从旧全集删方案；旧窗回放才删缺失项', () {
    final authoritative = [_profile('b'), _profile('c', active: true)];
    // 增量窗：并发新增的 c 必须保留，旧全集不得删任何方案。
    expect(
      routingCommitDeletes(
        incrementalCommitted: true,
        draftIds: {'b'},
        authoritative: authoritative,
      ),
      isEmpty,
    );
    // 旧窗：仍按缺失即删回放（保留历史语义，不静默改行为）。
    expect(
      routingCommitDeletes(
        incrementalCommitted: false,
        draftIds: {'b'},
        authoritative: authoritative,
      ),
      <String>{'c'},
    );
  });

  test('SP-13:每次增量提交后对权威快照验一致，不一致即失败不重放', () {
    final authoritative = [_profile('b', active: true)];
    expect(
      verifyRoutingIncrement(
        kind: 'deleteScheme',
        id: 'a',
        authoritative: authoritative,
      ),
      isNull,
    );
    expect(
      verifyRoutingIncrement(
        kind: 'deleteScheme',
        id: 'b',
        authoritative: authoritative,
      ),
      isNotNull,
      reason: '删除后权威仍在：未知结果不得重放非幂等写',
    );
    expect(
      verifyRoutingIncrement(
        kind: 'saveScheme',
        id: 'b',
        authoritative: authoritative,
      ),
      isNull,
    );
    expect(
      verifyRoutingIncrement(
        kind: 'saveScheme',
        id: 'zzz',
        authoritative: authoritative,
      ),
      isNotNull,
    );
    expect(
      verifyRoutingIncrement(
        kind: 'setDefault',
        id: 'b',
        authoritative: authoritative,
      ),
      isNull,
    );
    expect(
      verifyRoutingIncrement(
        kind: 'setDefault',
        id: 'a',
        authoritative: authoritative,
      ),
      isNotNull,
      reason: 'active 提升必须以权威为准',
    );
  });

  test('SP-13:DNS 读失败判定只在无可用基线时成立', () {
    expect(
      DnsController.loadFailedFor(
        listOk: false,
        simpleOk: false,
        hasItems: false,
        hasSimple: false,
      ),
      isTrue,
    );
    // 任一读成功且有基线：可编辑，不算读失败。
    expect(
      DnsController.loadFailedFor(
        listOk: true,
        simpleOk: true,
        hasItems: false,
        hasSimple: true,
      ),
      isFalse,
    );
    // 部分失败但有旧基线：保留基线展示，不按空草稿处理。
    expect(
      DnsController.loadFailedFor(
        listOk: false,
        simpleOk: true,
        hasItems: true,
        hasSimple: true,
      ),
      isFalse,
    );
  });

  testWidgets('SP-13:增量窗确定带已提交标记，旧窗不带', (tester) async {
    final commitHost = _CommitHost();
    await _pumpRouting(tester, commitHost);
    await tester.tap(find.byKey(const ValueKey('routing-ok')));
    await tester.pumpAndSettle();
    expect(commitHost.lastDraft, isNotNull);
    expect(commitHost.lastDraft!.incrementalCommitted, isTrue);
    expect(commitHost.closeCalls, 1);

    // 卸载旧窗 State（同类型复用会残留 _busy），再建旧窗。
    await tester.pumpWidget(const MaterialApp(home: SizedBox()));
    await tester.pumpAndSettle();
    final legacyHost = _LegacyHost();
    await _pumpRouting(tester, legacyHost);
    await tester.tap(find.byKey(const ValueKey('routing-ok')));
    await tester.pumpAndSettle();
    expect(legacyHost.lastDraft, isNotNull);
    expect(legacyHost.lastDraft!.incrementalCommitted, isFalse);
  });

  testWidgets('SP-13:部分删除后重开不复活', (tester) async {
    final host = _PartialFailHost();
    await _pumpRouting(tester, host);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.delete);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('routing-delete-confirm')));
    await tester.pumpAndSettle();
    expect(host.stored, <String>{'b'});

    // 关闭重开：权威只有 b，a 不复活。
    await tester.tap(find.byKey(const ValueKey('routing-cancel')));
    await tester.pumpAndSettle();
    await _pumpRouting(tester, host);
    expect(find.byKey(const ValueKey('routing-row-a')), findsNothing);
    expect(find.byKey(const ValueKey('routing-row-b')), findsOneWidget);
  });

  testWidgets('SP-13:DNS 读失败时保存不写，取消可关', (tester) async {
    final container = _dnsContainer();
    container.read(dnsControllerProvider.notifier).reload();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final before = bridge.loadSimpleDns().item?.directDns;

    await _pumpDns(tester, container);
    await tester.tap(find.byKey(const ValueKey('sp13-open-dns')));
    await tester.pumpAndSettle();
    expect(find.byType(DnsSettingWindow), findsOneWidget);

    // 改一个可见字段，再强制读失败基线。
    await tester.enterText(find.byKey(const ValueKey('dns-direct')), '9.9.9.9');
    final state = container.read(dnsControllerProvider);
    container.read(dnsControllerProvider.notifier).state = state.copyWith(
      loadFailed: true,
      status: 'error.dns_unavailable',
    );

    await tester.tap(find.byKey(const ValueKey('dns-save')));
    await tester.pumpAndSettle();
    // 窗口仍在（未关闭），桥上基线未被空草稿覆盖。
    expect(find.byType(DnsSettingWindow), findsOneWidget);
    expect(bridge.loadSimpleDns().item?.directDns, before);

    await tester.tap(find.byKey(const ValueKey('dns-cancel')));
    await tester.pumpAndSettle();
    expect(find.byType(DnsSettingWindow), findsNothing);
  });
}
