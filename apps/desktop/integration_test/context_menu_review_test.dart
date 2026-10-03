import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:two_dimensional_scrollables/two_dimensional_scrollables.dart';
import 'package:v2rayn_desktop/app/app.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/shared/widgets/adaptive_toolbar.dart';

const _frame = ValueKey('context-review-frame');

Future<void> _settle(WidgetTester tester) =>
    tester.pump(const Duration(milliseconds: 450));

Future<void> _shot(WidgetTester tester, String dir, String name) async {
  await tester.pump(const Duration(milliseconds: 200));
  if (Platform.environment['V2RAYN_R_CONTEXT_SKIP_IMAGES'] == '1') return;
  final boundary = tester.renderObject<RenderRepaintBoundary>(
    find.byKey(_frame),
  );
  final image = await boundary.toImage(pixelRatio: 1);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
  if (bytes == null) throw StateError('Screenshot encoding failed');
  await File('$dir/$name.png').writeAsBytes(bytes.buffer.asUint8List());
  image.dispose();
}

Map<String, double> _rect(WidgetTester tester, Finder finder) {
  final rect = tester.getRect(finder);
  return {
    'x': rect.left,
    'y': rect.top,
    'width': rect.width,
    'height': rect.height,
  };
}

bool _menuOpen(WidgetTester tester) => tester
    .widget<MenuAnchor>(
      find
          .descendant(
            of: find.byType(ProfilesTable),
            matching: find.byType(MenuAnchor),
          )
          .first,
    )
    .controller!
    .isOpen;

Future<void> _right(WidgetTester tester, Offset point) async {
  await tester.tapAt(
    point,
    kind: PointerDeviceKind.mouse,
    buttons: kSecondaryButton,
  );
  await _settle(tester);
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('context-menu lifetime and desktop spacing in the real window', (
    tester,
  ) async {
    final dataDir = Platform.environment['V2RAYN_R_DATA_DIR'];
    final evidenceDir = Platform.environment['V2RAYN_R_CONTEXT_EVIDENCE_DIR'];
    final mode = Platform.environment['V2RAYN_R_CONTEXT_MODE'] ?? 'full';
    final sampleCount = (mode == 'editor' || mode == 'screens')
        ? 4
        : (mode == 'interaction' ? 8 : 32);
    expect(dataDir, isNotNull);
    expect(evidenceDir, isNotNull);
    await Directory(evidenceDir!).create(recursive: true);
    final records = <Map<String, Object?>>[];
    final failures = <String>[];
    void record(String name, Map<String, Object?> facts, {bool? passed}) {
      final entry = <String, Object?>{'step': name, 'facts': facts};
      if (passed != null) entry['passed'] = passed;
      records.add(entry);
      if (passed == false) failures.add(name);
      File('$evidenceDir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ')
            .convert({'failures': failures, 'records': records}),
      );
    }

    void complete() {
      File('$evidenceDir/observations.json').writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert({
          'recordingComplete': true,
          'mode': mode,
          'imagesSkipped':
              Platform.environment['V2RAYN_R_CONTEXT_SKIP_IMAGES'] == '1',
          'failures': failures,
          'records': records,
        }),
      );
      expect(
        failures,
        isEmpty,
        reason: 'Context-menu flow gaps: ${failures.join(', ')}',
      );
    }

    RustBridgeInit.configure(RustLib.init);
    await RustBridgeInit.init();
    record('bridge-initialized', {});
    runApp(
      ProviderScope(
        overrides: [
          uiStateStoreProvider.overrideWithValue(
            FileUiStateStore(overridePath: '$dataDir/ui_state.json'),
          ),
        ],
        child: const RepaintBoundary(key: _frame, child: V2rayNRApp()),
      ),
    );
    for (
      var i = 0;
      i < 100 && find.byKey(const ValueKey('menu-配置项')).evaluate().isEmpty;
      i++
    ) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    final container = ProviderScope.containerOf(
      tester.element(find.byType(V2rayNRApp)),
    );
    record('app-started', {});

    try {
      Future<void> closeByOutside() async {
        await tester.tap(find.byKey(const ValueKey('group-filter-all')));
        await _settle(tester);
        // Clear the event log so the next menu command's restore can be seen.
        container.read(profilesControllerProvider.notifier).resetEvents();
      }

      final emptyRect = tester.getRect(find.byType(ProfilesTable));
      await _right(tester, emptyRect.center);
      final emptyEdit = tester.widget<MenuItemButton>(
        find.byKey(const ValueKey('ctx-编辑')),
      );
      record(
        'right-click-empty-table',
        {
          'menuOpen': _menuOpen(tester),
          'editEnabled': emptyEdit.onPressed != null,
          'selected': container
              .read(profilesControllerProvider)
              .selected
              .toList(),
        },
        // Upstream DataGrid.ContextMenu covers the whole grid, so the empty
        // table must open it; with no target the selection-dependent entries
        // stay disabled instead of inventing a row.
        passed: _menuOpen(tester) && emptyEdit.onPressed == null,
      );
      await _shot(tester, evidenceDir, '00-empty-right-click');
      await closeByOutside();
      final links = List.generate(
        sampleCount,
        (i) =>
            'vless://${(i + 1).toString().padLeft(8, '0')}-1111-1111-1111-111111111111@127.0.0.1:11980?encryption=none#${Uri.encodeComponent('节点${(i + 1).toString().padLeft(2, '0')}')}',
      ).join('\n');
      await Clipboard.setData(ClipboardData(text: links));
      await tester.tap(find.byKey(const ValueKey('menu-配置项')));
      await _settle(tester);
      await tester.tap(find.byKey(const ValueKey('menu-item-从剪贴板导入分享链接')));
      for (
        var i = 0;
        i < 100 &&
            container.read(profilesControllerProvider).totalCount < sampleCount;
        i++
      ) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      expect(
        container.read(profilesControllerProvider).totalCount,
        sampleCount,
      );
      record('synthetic-nodes-imported', {'count': sampleCount});
      final rows = container.read(profilesControllerProvider).visible;
      Finder cell(int index) =>
          find.byKey(ValueKey('cell-${rows[index].id}-Remarks'));

      if (mode == 'screens') {
        // Light screenshot-only pass: open the menu on a row, then capture the
        // structure panel, the export submenu, the move-to-group submenu and a
        // bottom-edge open. Kept separate so the heavier interaction flow's
        // intermittent native crash cannot prevent the required PNGs.
        final shootPoint = tester.getCenter(cell(1));
        await _right(tester, shootPoint);
        await _shot(tester, evidenceDir, 'menu-overview');
        final exportFinder = find.byKey(const ValueKey('ctx-导出'));
        final hover = TestGesture(
          dispatcher: tester.sendEventToBinding,
          kind: PointerDeviceKind.mouse,
          device: 71,
          pointer: 571,
        );
        await hover.addPointer(location: tester.getCenter(exportFinder));
        await hover.moveTo(tester.getCenter(exportFinder));
        await tester.pump(const Duration(milliseconds: 700));
        await _shot(tester, evidenceDir, 'submenu-export');
        final moveFinder = find.byKey(const ValueKey('ctx-移至订阅分组'));
        await hover.moveTo(tester.getCenter(moveFinder));
        await tester.pump(const Duration(milliseconds: 700));
        await _shot(tester, evidenceDir, 'submenu-move-to-group');
        await hover.removePointer();
        await closeByOutside();

        // Bottom-right corner: the menu must flip/reposition to stay visible.
        final tableRect = tester.getRect(find.byType(ProfilesTable));
        await _right(tester, Offset(tableRect.right - 8, tableRect.bottom - 8));
        await _shot(tester, evidenceDir, 'menu-edge-bottom-right');
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pump(const Duration(milliseconds: 200));
        await _shot(tester, evidenceDir, 'menu-keyboard-highlight');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _settle(tester);
        complete();
        return;
      }

      if (mode == 'editor') {
        for (var index = 0; index < 2; index++) {
          final clickPoint = tester.getCenter(cell(index));
          await _right(tester, clickPoint);
          final editorItem = _rect(
            tester,
            find.byKey(const ValueKey('ctx-设为活动')),
          );
          final editButton = tester.widget<MenuItemButton>(
            find.byKey(const ValueKey('ctx-编辑')),
          );
          record(
            'editor-right-click-$index',
            {
              'menuOpen': _menuOpen(tester),
              'editEnabled': editButton.onPressed != null,
              'selected': container
                  .read(profilesControllerProvider)
                  .selected
                  .toList(),
              'firstItem': editorItem,
              'leftDelta': (editorItem['x']! - clickPoint.dx).abs(),
            },
            passed:
                _menuOpen(tester) &&
                editButton.onPressed != null &&
                (editorItem['x']! - clickPoint.dx).abs() <= 8,
          );
          await tester.tap(find.byKey(const ValueKey('ctx-编辑')));
          await _settle(tester);
          final restores = container
              .read(profilesControllerProvider)
              .events
              .where((e) => e.detail.startsWith('restore='))
              .map((e) => e.detail)
              .toList();
          final editorField = find.byKey(const ValueKey('field-remarks'));
          final actualRemarks = editorField.evaluate().isEmpty
              ? null
              : tester.widget<TextFormField>(editorField).initialValue;
          record(
            'editor-command-$index',
            {
              'menuOpen': _menuOpen(tester),
              'expectedRemarks': rows[index].remarks,
              'actualRemarks': actualRemarks,
              'restoreEvents': restores,
              'editorVisible': editorField.evaluate().isNotEmpty,
              'selectedAfter': container
                  .read(profilesControllerProvider)
                  .selected
                  .toList(),
            },
            // The editor opens the node the menu was opened on, and the menu is
            // closed. `restoreEvents` is empty when the right-click already set
            // the selection to the target; it is non-empty only when the
            // selection had drifted, and is recorded for evidence.
            passed:
                !_menuOpen(tester) &&
                editorField.evaluate().isNotEmpty &&
                actualRemarks == rows[index].remarks,
          );
          await _shot(tester, evidenceDir, 'editor-$index');
          await tester.tap(find.byKey(const ValueKey('editor-cancel')));
          await _settle(tester);
          record(
            'editor-cancel-$index',
            {
              'menuOpen': _menuOpen(tester),
              'count': container.read(profilesControllerProvider).totalCount,
              'selected': container
                  .read(profilesControllerProvider)
                  .selected
                  .toList(),
            },
            passed:
                !_menuOpen(tester) &&
                container.read(profilesControllerProvider).totalCount ==
                    sampleCount &&
                container
                    .read(profilesControllerProvider)
                    .selected
                    .contains(rows[index].id),
          );
        }
        complete();
        return;
      }

      if (mode == 'interaction') {
        final pointA = tester.getCenter(cell(0));
        final item = find.byKey(const ValueKey('ctx-设为活动'));
        final windowRect = _rect(tester, find.byKey(_frame));
        final tableRect = tester.getRect(find.byType(ProfilesTable));
        // The corrected menu now opens at the click point and covers the
        // nearby cells, so "click another row" must target a row region the
        // panel does not overlap: the far-right edge of the table.
        Offset rowRight(int index) =>
            Offset(tableRect.right - 20, tester.getCenter(cell(index)).dy);
        final pointB = rowRight(1);

        // 1. Open on row 0 and verify anchor-local positioning.
        await _right(tester, pointA);
        final itemRect = _rect(tester, item);
        record('menu-position-and-density', {
          'firstItem': itemRect,
          'clickX': pointA.dx,
          'rootWindow': windowRect,
        }, passed: (itemRect['x']! - pointA.dx).abs() <= 8);
        await _shot(tester, evidenceDir, '01-right-menu');

        // 2. Left-click row 1: close + select B in one click.
        await tester.tapAt(pointB, kind: PointerDeviceKind.mouse);
        await _settle(tester);
        record(
          'left-click-another-row-dismisses-menu',
          {
            'menuOpen': _menuOpen(tester),
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          passed:
              !_menuOpen(tester) &&
              setEquals(container.read(profilesControllerProvider).selected, {
                rows[1].id,
              }),
        );
        await _shot(tester, evidenceDir, '02-click-other-row');

        // 3. Esc closes the menu without clearing the selection.
        await _right(tester, pointA);
        final selBefore = Set<String>.of(
          container.read(profilesControllerProvider).selected,
        );
        final openBefore = _menuOpen(tester);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _settle(tester);
        record(
          'escape-closes-menu-without-clearing-selection',
          {
            'menuOpenBeforeEscape': openBefore,
            'menuOpen': _menuOpen(tester),
            'selectedBefore': selBefore.toList(),
            'selectedAfter': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          passed:
              openBefore &&
              !_menuOpen(tester) &&
              setEquals(
                selBefore,
                container.read(profilesControllerProvider).selected,
              ),
        );
        await _shot(tester, evidenceDir, '03-escape');

        // 4. Multi-select, then right-click one of the selected rows.
        await tester.tap(cell(0));
        await _settle(tester);
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.tap(cell(1));
        await _settle(tester);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        final multi = Set<String>.of(
          container.read(profilesControllerProvider).selected,
        );
        await _right(tester, pointB);
        record(
          'right-click-selected-multi-row',
          {
            'selectionBefore': multi.toList(),
            'selectionAfter': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          passed: setEquals(
            multi,
            container.read(profilesControllerProvider).selected,
          ),
        );

        // 5. Re-invoke on another row while the menu is open. The corrected
        // menu sits at the click point and its tall panel covers the data
        // cells, so the second target is that row's row-handle (left gutter,
        // outside the panel) - still a row target.
        final pointC = tester.getCenter(
          find.byKey(ValueKey('handle-${rows[2].id}')),
        );
        await _right(tester, pointC);
        final cItem = _rect(tester, item);
        record(
          'right-click-different-row-while-menu-open',
          {
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
            'menuOpen': _menuOpen(tester),
            'menuCount': find
                .byType(MenuAnchor)
                .evaluate()
                .where((e) => (e.widget as MenuAnchor).controller!.isOpen)
                .length,
            'leftDelta': (cItem['x']! - pointC.dx).abs(),
          },
          passed:
              _menuOpen(tester) &&
              setEquals(container.read(profilesControllerProvider).selected, {
                rows[2].id,
              }) &&
              (cItem['x']! - pointC.dx).abs() <= 8,
        );
        await closeByOutside();

        // 6. Row-header right-click targets that row.
        final handle = tester.getCenter(
          find.byKey(ValueKey('handle-${rows[4].id}')),
        );
        await _right(tester, handle);
        record(
          'right-click-row-header',
          {
            'menuOpen': _menuOpen(tester),
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          passed:
              _menuOpen(tester) &&
              setEquals(container.read(profilesControllerProvider).selected, {
                rows[4].id,
              }),
        );
        await closeByOutside();

        // 7. Empty area right-click opens the grid menu with disabled
        // target-dependent entries and keeps the selection.
        await tester.tap(find.byKey(const ValueKey('group-filter-all')));
        await _settle(tester);
        container.read(profilesControllerProvider.notifier).clearSelection();
        await _settle(tester);
        final emptyPoint = Offset(tableRect.right - 20, tableRect.bottom - 12);
        await _right(tester, emptyPoint);
        final emptyEdit = tester.widget<MenuItemButton>(
          find.byKey(const ValueKey('ctx-编辑')),
        );
        record('right-click-empty-table', {
          'menuOpen': _menuOpen(tester),
          'editEnabled': emptyEdit.onPressed != null,
          'selected': container
              .read(profilesControllerProvider)
              .selected
              .toList(),
        }, passed: _menuOpen(tester) && emptyEdit.onPressed == null);
        await _shot(tester, evidenceDir, '04-empty-right-click');
        await closeByOutside();

        // 8. Structure parity: 17 root entries in upstream order, 4 separators,
        // 32-px rows. Labels come from ResUI.zh-Hans; the two extra roots
        // (快速真延迟 / 混合测试) must be gone.
        await tester.tap(cell(0));
        await _settle(tester);
        await _right(tester, pointA);
        final rootOrder = <String>[
          '设为活动',
          '编辑',
          '克隆所选',
          '移除所选 (多选)',
          '移除重复',
          '按测试结果移除无效',
          '测试延迟 Tcping (多选)',
          '测试真连接延迟 (多选)',
          '测试 UDP 延迟 (多选)',
          '测试速度 (多选)',
          '按测试结果排序',
          '移至订阅分组',
          '移至上下',
          '全选',
          '分享',
          '导出',
          '一键生成策略组',
        ];
        final missingRoots = <String>[
          for (final label in rootOrder)
            if (find.byKey(ValueKey('ctx-$label')).evaluate().isEmpty) label,
        ];
        final extraRoots = <String>[
          for (final label in <String>['快速真延迟', '混合测试 (真连接+测速)'])
            if (find.byKey(ValueKey('ctx-$label')).evaluate().isNotEmpty) label,
        ];
        bool isContextKey(Element e) {
          final key = e.widget.key;
          return key is ValueKey<String> && key.value.startsWith('ctx-');
        }

        // Scope to the menu overlay: only widgets carrying a `ctx-` key belong
        // to the node-table context menu (the shell MenuBar has its own).
        final rootItems = <Element>[
          ...find.byType(MenuItemButton).evaluate(),
          ...find.byType(SubmenuButton).evaluate(),
        ].where(isContextKey).length;
        final separators = find
            .byType(Divider)
            .evaluate()
            .where(
              (e) =>
                  e.widget.key is ValueKey<String> &&
                  (e.widget.key! as ValueKey<String>).value.startsWith(
                    'ctx-sep-',
                  ),
            )
            .length;
        final firstRowHeight = tester
            .getSize(find.byKey(const ValueKey('ctx-设为活动')))
            .height;
        record(
          'menu-structure-parity',
          {
            'rootOrder': rootOrder,
            'missingRoots': missingRoots,
            'extraRoots': extraRoots,
            'rootItemCount': rootItems,
            'separatorCount': separators,
            'firstRowHeight': firstRowHeight,
          },
          passed:
              missingRoots.isEmpty &&
              extraRoots.isEmpty &&
              rootItems == 17 &&
              separators == 4 &&
              firstRowHeight == 32,
        );
        await _shot(tester, evidenceDir, '08-menu-structure');

        // 9. Keyboard navigation: ↓ moves the highlight, Enter activates the
        // highlighted row, Esc closes without clearing the selection.
        final selectedBeforeKeys = Set<String>.of(
          container.read(profilesControllerProvider).selected,
        );
        final beforeDown = _menuOpen(tester);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await _settle(tester);
        // Two ↓ from the "no highlight" state land on the second root entry
        // (编辑); the highlight is a coloured decoration on that row.
        final highlightedAfter = tester
            .widget<Container>(
              find
                  .descendant(
                    of: find.byKey(const ValueKey('ctx-编辑')),
                    matching: find.byType(Container),
                  )
                  .first,
            )
            .decoration;
        record(
          'keyboard-arrow-navigates-menu',
          {
            'menuOpenBefore': beforeDown,
            'menuOpen': _menuOpen(tester),
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          // Two ↓ from the "no highlight" state land on the second root entry;
          // the table selection must be untouched (keys stay in the menu).
          passed:
              beforeDown &&
              _menuOpen(tester) &&
              setEquals(
                selectedBeforeKeys,
                container.read(profilesControllerProvider).selected,
              ) &&
              highlightedAfter != null,
        );
        await _shot(tester, evidenceDir, '09-keyboard-highlight');
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _settle(tester);
        record(
          'keyboard-escape-closes-menu',
          {
            'menuOpen': _menuOpen(tester),
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          passed:
              !_menuOpen(tester) &&
              setEquals(
                selectedBeforeKeys,
                container.read(profilesControllerProvider).selected,
              ),
        );

        // 10. Submenu hover: entering 导出 keeps the root open and renders the
        // share/export entries; leaving the root menu does not close the chain.
        await _right(tester, pointA);
        final exportFinder = find.byKey(const ValueKey('ctx-导出'));
        final hover = TestGesture(
          dispatcher: tester.sendEventToBinding,
          kind: PointerDeviceKind.mouse,
          device: 61,
          pointer: 561,
        );
        await hover.addPointer(location: tester.getCenter(exportFinder));
        await hover.moveTo(tester.getCenter(exportFinder));
        await tester.pump(const Duration(milliseconds: 700));
        final exportEntry = find.byKey(const ValueKey('ctx-导出分享链接至剪贴板 (多选)'));
        record('submenu-hover-keeps-chain', {
          'rootOpen': _menuOpen(tester),
          'exportItems': exportEntry.evaluate().length,
        }, passed: _menuOpen(tester) && exportEntry.evaluate().isNotEmpty);
        await _shot(tester, evidenceDir, '10-submenu');
        await hover.moveTo(
          tester.getCenter(find.byKey(const ValueKey('ctx-编辑'))),
        );
        await _settle(tester);
        await hover.removePointer();
        record('pointer-crosses-gap-stays-open', {
          'menuOpen': _menuOpen(tester),
        });

        // 11. Move-to-group submenu is a real cascade listing the live groups
        // (the synthetic import attaches nodes to no subscription, so the
        // honest list shows 无分组 plus any stored groups).
        await closeByOutside();
        await _right(tester, pointA);
        final moveFinder = find.byKey(const ValueKey('ctx-移至订阅分组'));
        final moveHover = TestGesture(
          dispatcher: tester.sendEventToBinding,
          kind: PointerDeviceKind.mouse,
          device: 62,
          pointer: 562,
        );
        await moveHover.addPointer(location: tester.getCenter(moveFinder));
        await moveHover.moveTo(tester.getCenter(moveFinder));
        await tester.pump(const Duration(milliseconds: 700));
        final noGroup = find.byKey(const ValueKey('ctx-无分组'));
        record('move-to-group-submenu', {
          'rootOpen': _menuOpen(tester),
          'noGroupVisible': noGroup.evaluate().isNotEmpty,
        }, passed: _menuOpen(tester) && noGroup.evaluate().isNotEmpty);
        await _shot(tester, evidenceDir, '11-move-to-group');
        // The 「无分组」 entry must be enabled (real save path exists). Its
        // execution is covered by the controller unit test
        // `profiles move-to-group rewrites subid through the save path`; the
        // cascade tap in the integration overlay is not a stable hit target.
        final noGroupButton = tester.widget<MenuItemButton>(noGroup);
        record('move-to-group-entry-enabled', {
          'noGroupEnabled': noGroupButton.onPressed != null,
        }, passed: noGroupButton.onPressed != null);
        await moveHover.removePointer();

        complete();
        return;
      }

      await tester.tap(cell(0));
      await _settle(tester);
      final pointA = tester.getCenter(cell(0));
      final tableRect = tester.getRect(find.byType(ProfilesTable));
      // The corrected menu sits at the click point and its tall panel covers
      // the nearby data cells, so other-row targets use the far-right edge of
      // the table (outside the panel).
      Offset rowRight(int index) =>
          Offset(tableRect.right - 20, tester.getCenter(cell(index)).dy);
      final pointB = rowRight(1);
      final item = find.byKey(const ValueKey('ctx-设为活动'));
      if (mode != 'late') {
        final mouse = await tester.startGesture(
          pointA,
          kind: PointerDeviceKind.mouse,
          buttons: kSecondaryButton,
        );
        await tester.pump(const Duration(milliseconds: 150));
        record('right-button-down', {
          'menuOpenBeforeRelease': _menuOpen(tester),
        });
        await mouse.up();
        await _settle(tester);
        final itemRect = _rect(tester, item);
        final windowRect = _rect(tester, find.byKey(_frame));
        record(
          'menu-position-and-density',
          {
            'clickX': pointA.dx,
            'clickY': pointA.dy,
            'tableX': tableRect.left,
            'tableY': tableRect.top,
            'firstItem': itemRect,
            'rootWindow': windowRect,
            'toolbar': _rect(tester, find.byType(AdaptiveToolbar).first),
            'tableRow': _rect(tester, cell(0)),
            'leftDelta': (itemRect['x']! - pointA.dx).abs(),
            'itemLabelFont': tester
                .widget<Text>(
                  find.descendant(of: item, matching: find.text('设为活动')),
                )
                .style
                ?.fontSize,
            'rootContextItemCount': find
                .byType(MenuItemButton)
                .evaluate()
                .length,
          },
          // MenuAnchor.open(position:) takes anchor-local coordinates; the menu
          // left edge must track the click point (<=8 lp tolerance for panel
          // padding/density) and stay horizontally inside the window. Vertical
          // overflow is deferred to UX-SPACE-02 (19 items * 40 lp > window) and
          // is only required not to be clipped off the top.
          passed:
              (itemRect['x']! - pointA.dx).abs() <= 8 &&
              itemRect['x']! >= windowRect['x']! - 0.5 &&
              itemRect['x']! + itemRect['width']! <=
                  windowRect['x']! + windowRect['width']! + 0.5 &&
              itemRect['y']! >= windowRect['y']! - 0.5,
        );
        await _shot(tester, evidenceDir, '01-right-menu');

        await mouse.moveTo(const Offset(80, 190));
        await _settle(tester);
        record('pointer-leaves-menu', {'menuOpen': _menuOpen(tester)});
        await tester.tapAt(pointB, kind: PointerDeviceKind.mouse);
        await _settle(tester);
        record(
          'left-click-another-row-dismisses-menu',
          {
            'menuOpen': _menuOpen(tester),
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          // Clicking a table row while the menu is open closes the menu and the
          // same click selects that row (close + normal handling).
          passed:
              !_menuOpen(tester) &&
              setEquals(container.read(profilesControllerProvider).selected, {
                rows[1].id,
              }),
        );
        await _shot(tester, evidenceDir, '02-click-other-row');

        await closeByOutside();
        // Re-open the menu, then verify Esc closes it without leaking the
        // table's Esc semantics (which would clear the selection).
        await _right(tester, pointA);
        final selectionBeforeEscape = Set<String>.of(
          container.read(profilesControllerProvider).selected,
        );
        final menuOpenBeforeEscape = _menuOpen(tester);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _settle(tester);
        final selectionAfterEscape = container
            .read(profilesControllerProvider)
            .selected;
        record(
          'escape-closes-menu-without-clearing-selection',
          {
            'menuOpenBeforeEscape': menuOpenBeforeEscape,
            'menuOpen': _menuOpen(tester),
            'selectedBefore': selectionBeforeEscape.toList(),
            'selectedAfter': selectionAfterEscape.toList(),
          },
          passed:
              menuOpenBeforeEscape &&
              !_menuOpen(tester) &&
              selectionBeforeEscape.isNotEmpty &&
              setEquals(selectionBeforeEscape, selectionAfterEscape),
        );
        await _shot(tester, evidenceDir, '03-escape');
        await closeByOutside();

        await tester.tap(cell(0));
        await _settle(tester);
        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.tap(cell(1));
        await _settle(tester);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        final multi = Set<String>.of(
          container.read(profilesControllerProvider).selected,
        );
        await _right(tester, pointB);
        record(
          'right-click-selected-multi-row',
          {
            'selectionBefore': multi.toList(),
            'selectionAfter': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
            'menuOpen': _menuOpen(tester),
          },
          passed: setEquals(
            multi,
            container.read(profilesControllerProvider).selected,
          ),
        );
        final pointC = tester.getCenter(
          find.byKey(ValueKey('handle-${rows[2].id}')),
        );
        await _right(tester, pointC);
        final cMenuItem = _rect(tester, item);
        record(
          'right-click-different-row-while-menu-open',
          {
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
            'menuOpen': _menuOpen(tester),
            // Re-invoking on another row rebuilds exactly one menu, retargets
            // the selection to that row and repositions under the new click.
            'menuCount': find.byType(MenuAnchor).evaluate().where((e) {
              final w = e.widget as MenuAnchor;
              return w.controller?.isOpen ?? false;
            }).length,
            'firstItem': cMenuItem,
            'leftDelta': (cMenuItem['x']! - pointC.dx).abs(),
          },
          passed:
              _menuOpen(tester) &&
              setEquals(container.read(profilesControllerProvider).selected, {
                rows[2].id,
              }) &&
              (cMenuItem['x']! - pointC.dx).abs() <= 8,
        );
        await closeByOutside();
        container.read(profilesControllerProvider.notifier).clearSelection();
        await _settle(tester);

        await _right(
          tester,
          tester.getCenter(find.byKey(ValueKey('handle-${rows[0].id}'))),
        );
        record(
          'right-click-row-header',
          {
            'menuOpen': _menuOpen(tester),
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          },
          // Upstream DataGrid.ContextMenu triggers on the row header too, and
          // the header targets that row.
          passed:
              _menuOpen(tester) &&
              setEquals(container.read(profilesControllerProvider).selected, {
                rows[0].id,
              }),
        );
        await closeByOutside();
      }

      await _right(tester, tester.getCenter(cell(0)));
      final tableView = tester.widget<TableView>(find.byType(TableView));
      final scrollBefore = tableView.verticalDetails.controller!.offset;
      await tester.sendEventToBinding(
        PointerScrollEvent(position: pointB, scrollDelta: const Offset(0, 120)),
      );
      await _settle(tester);
      record('wheel-over-table-while-menu-open', {
        'menuOpen': _menuOpen(tester),
        'scrollBefore': scrollBefore,
        'scrollAfter': tableView.verticalDetails.controller!.offset,
      });
      await _shot(tester, evidenceDir, '05-wheel');
      await closeByOutside();
      await tester.sendEventToBinding(
        PointerScrollEvent(
          position: pointB,
          scrollDelta: const Offset(0, -120),
        ),
      );
      await _settle(tester);

      await _right(tester, tester.getCenter(cell(5)));
      final export = find.byKey(const ValueKey('ctx-导出'));
      await tester.ensureVisible(export);
      final hover = TestGesture(
        dispatcher: tester.sendEventToBinding,
        kind: PointerDeviceKind.mouse,
        device: 45,
        pointer: 445,
      );
      await hover.addPointer(location: tester.getCenter(export));
      await hover.moveTo(tester.getCenter(export));
      await tester.pump(const Duration(milliseconds: 600));
      record('submenu-hover', {
        'rootOpen': _menuOpen(tester),
        'shareExportItems': find
            .byKey(const ValueKey('ctx-导出分享链接至剪贴板 (多选)'))
            .evaluate()
            .length,
      });
      await _shot(tester, evidenceDir, '06-submenu');
      await hover.moveTo(
        tester.getCenter(find.byKey(const ValueKey('ctx-编辑'))),
      );
      await _settle(tester);
      await hover.removePointer();
      await tester.tap(find.byKey(const ValueKey('ctx-编辑')));
      await _settle(tester);
      final actualRemarks = tester
          .widget<TextFormField>(find.byKey(const ValueKey('field-remarks')))
          .initialValue;
      record(
        'edit-command-closes-menu-and-opens-dialog',
        {
          'menuOpen': _menuOpen(tester),
          'editorVisible': find
              .byKey(const ValueKey('profile-editor'))
              .evaluate()
              .isNotEmpty,
          'remarksField': _rect(
            tester,
            find.byKey(const ValueKey('field-remarks')),
          ),
          'expectedRemarks': rows[5].remarks,
          'actualRemarks': actualRemarks,
        },
        passed:
            !_menuOpen(tester) &&
            find
                .byKey(const ValueKey('profile-editor'))
                .evaluate()
                .isNotEmpty &&
            actualRemarks == rows[5].remarks,
      );
      await _shot(tester, evidenceDir, '07-editor-spacing');
      await tester.tap(find.byKey(const ValueKey('editor-cancel')));
      await _settle(tester);
      record('cancel-editor-returns-to-table', {'menuOpen': _menuOpen(tester)});
      complete();
    } finally {
      await _shot(tester, evidenceDir, 'final-state');
      await tester.pumpWidget(const SizedBox.shrink());
      await _settle(tester);
    }
  });
}
