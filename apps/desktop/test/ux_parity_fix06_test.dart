import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/features/subs/sub_edit_window.dart';

import 'support/subs_harness.dart';

/// A subscription bridge that encodes the FIX-06 rule (empty URL = plain
/// group) without touching the shared synthetic bridge: a remarks-only group
/// validates and persists; a non-empty URL still must be http(s).
class Fix06SubsBridge extends SyntheticBridgePort {
  final List<c.SubItemDto> _groups = <c.SubItemDto>[];
  int _seq = 0;

  c.ErrorDto? _check(c.SubItemDto item) {
    if (item.remarks.trim().isEmpty) {
      return const c.ErrorDto(
        code: 'E_FIELD_REQUIRED',
        messageKey: 'error.remarks_required',
        fieldPath: 'remarks',
        retryable: false,
      );
    }
    final url = item.url.trim();
    if (url.isNotEmpty &&
        !url.startsWith('http://') &&
        !url.startsWith('https://')) {
      return const c.ErrorDto(
        code: 'E_FIELD_FORMAT',
        messageKey: 'error.url_invalid',
        fieldPath: 'url',
        retryable: false,
      );
    }
    return null;
  }

  @override
  c.SubsPageDto listSubItems() {
    final items = List<c.SubItemDto>.of(_groups)
      ..sort((a, b) => a.sort.compareTo(b.sort));
    return c.SubsPageDto(items: items);
  }

  @override
  c.SubItemDto? getSubItem(String id) {
    for (final s in _groups) {
      if (s.id == id) return s;
    }
    return null;
  }

  @override
  c.SubItemDtoResult saveSubItem(c.SubItemDto item) {
    final invalid = _check(item);
    if (invalid != null) {
      return c.SubItemDtoResult(ok: false, error: invalid);
    }
    final saved = item.id.trim().isEmpty
        ? _withId(item, 'fix06-sub-${_seq++}')
        : item;
    final index = _groups.indexWhere((s) => s.id == saved.id);
    if (index >= 0) {
      _groups[index] = saved;
    } else {
      _groups.add(saved);
    }
    return c.SubItemDtoResult(ok: true, item: saved);
  }

  @override
  c.SimpleResult validateSubItem(c.SubItemDto item) {
    final invalid = _check(item);
    return invalid == null
        ? const c.SimpleResult(ok: true)
        : c.SimpleResult(ok: false, error: invalid);
  }

  @override
  c.DeleteSubsResult deleteSubItems(List<String> ids) {
    final before = _groups.length;
    _groups.removeWhere((s) => ids.contains(s.id));
    return c.DeleteSubsResult(
      ok: true,
      removed: BigInt.from(before - _groups.length),
    );
  }

  c.SubItemDto _withId(c.SubItemDto item, String id) => c.SubItemDto(
    id: id,
    remarks: item.remarks,
    url: item.url,
    moreUrl: item.moreUrl,
    enabled: item.enabled,
    userAgent: item.userAgent,
    requestHeaders: item.requestHeaders,
    sort: item.sort,
    filter: item.filter,
    autoUpdateInterval: item.autoUpdateInterval,
    updateTime: item.updateTime,
    convertTarget: item.convertTarget,
    prevProfile: item.prevProfile,
    nextProfile: item.nextProfile,
    preSocksPort: item.preSocksPort,
    memo: item.memo,
    customCoreType: item.customCoreType,
  );
}

/// Opens the editor through the real `showSubEditWindow` and saves the popped
/// draft through the controller, while rendering the current group list the
/// same way the top toolbar does (watch profiles state + read `subItems()`).
class _EditorHost extends ConsumerWidget {
  const _EditorHost();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Watching profiles state is what the real toolbar does; it is the
    // refresh signal for the group chips.
    ref.watch(profilesControllerProvider);
    final groups = ref.read(profilesControllerProvider.notifier).subItems();
    return Column(
      children: <Widget>[
        FilledButton(
          key: const ValueKey('open-editor'),
          onPressed: () async {
            final controller = ref.read(subsControllerProvider.notifier);
            final saved = await showSubEditWindow(
              context,
              controller.newDraft(),
            );
            if (saved != null) controller.save(saved);
          },
          child: const Text('新增'),
        ),
        for (final group in groups)
          Text(
            group.remarks.isEmpty ? group.id : group.remarks,
            key: ValueKey('chip-${group.id}'),
          ),
      ],
    );
  }
}

Future<void> _pumpHost(WidgetTester tester, ProviderContainer container) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: _EditorHost())),
    ),
  );
  await tester.pump();
}

void main() {
  testWidgets('remarks-only plain group saves and reaches the group list', (
    tester,
  ) async {
    final bridge = Fix06SubsBridge();
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    await _pumpHost(tester, container);

    await tester.tap(find.byKey(const ValueKey('open-editor')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('sub-field-remarks')),
      '仅备注普通分组',
    );
    await tester.tap(find.byKey(const ValueKey('sub-edit-save')));
    await tester.pumpAndSettle();

    // The save went through the real controller and persistence.
    final items = bridge.listSubItems().items;
    expect(items, hasLength(1));
    expect(items.single.remarks, '仅备注普通分组');
    expect(items.single.url, isEmpty);
    expect(container.read(subsControllerProvider).items, hasLength(1));

    // The top group chips rebuild off the profiles controller (FIX-06).
    expect(find.text('仅备注普通分组'), findsOneWidget);
    expect(find.byKey(ValueKey('chip-${items.single.id}')), findsOneWidget);
  });

  testWidgets('non-empty invalid URL is rejected with the Rust error', (
    tester,
  ) async {
    final bridge = Fix06SubsBridge();
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    await _pumpHost(tester, container);

    await tester.tap(find.byKey(const ValueKey('open-editor')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('sub-field-remarks')),
      '非法订阅',
    );
    await tester.enterText(
      find.byKey(const ValueKey('sub-field-url')),
      'ftp://example.com',
    );
    await tester.tap(find.byKey(const ValueKey('sub-edit-save')));
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('sub-edit-error')), findsOneWidget);
    expect(find.text('error.url_invalid'), findsOneWidget);
    expect(find.byKey(const ValueKey('sub-edit-window')), findsOneWidget);
    expect(bridge.listSubItems().items, isEmpty);
  });

  testWidgets('cancelling the editor does not persist', (tester) async {
    final bridge = Fix06SubsBridge();
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    await _pumpHost(tester, container);

    await tester.tap(find.byKey(const ValueKey('open-editor')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('sub-field-remarks')),
      '未保存',
    );
    await tester.tap(find.byKey(const ValueKey('sub-edit-cancel')));
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('sub-edit-window')), findsNothing);
    expect(bridge.listSubItems().items, isEmpty);
  });
}
