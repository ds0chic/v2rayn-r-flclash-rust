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
    final sampleCount = mode == 'editor' ? 4 : 32;
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
      final emptyRect = tester.getRect(find.byType(ProfilesTable));
      await _right(tester, emptyRect.center);
      record('right-click-empty-table', {'menuOpen': _menuOpen(tester)});
      await _shot(tester, evidenceDir, '00-empty-right-click');
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
      Future<void> closeByOutside() async {
        await tester.tap(find.byKey(const ValueKey('group-filter-all')));
        await _settle(tester);
      }

      if (mode == 'editor') {
        for (var index = 0; index < 2; index++) {
          await _right(tester, tester.getCenter(cell(index)));
          record('editor-right-click-$index', {
            'menuOpen': _menuOpen(tester),
            'selected': container
                .read(profilesControllerProvider)
                .selected
                .toList(),
          });
          await tester.tap(find.byKey(const ValueKey('ctx-编辑')));
          await _settle(tester);
          final actualRemarks = tester
              .widget<TextFormField>(
                find.byKey(const ValueKey('field-remarks')),
              )
              .initialValue;
          record(
            'editor-command-$index',
            {
              'menuOpen': _menuOpen(tester),
              'expectedRemarks': rows[index].remarks,
              'actualRemarks': actualRemarks,
              'remarksField': _rect(
                tester,
                find.byKey(const ValueKey('field-remarks')),
              ),
            },
            passed: !_menuOpen(tester) && actualRemarks == rows[index].remarks,
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

      await tester.tap(cell(0));
      await _settle(tester);
      final pointA = tester.getCenter(cell(0));
      final pointB = tester.getCenter(cell(1));
      final tableRect = tester.getRect(find.byType(ProfilesTable));
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
        final itemRect = tester.getRect(item);
        record('menu-position-and-density', {
          'clickX': pointA.dx,
          'clickY': pointA.dy,
          'tableX': tableRect.left,
          'tableY': tableRect.top,
          'firstItem': _rect(tester, item),
          'rootWindow': _rect(tester, find.byKey(_frame)),
          'toolbar': _rect(tester, find.byType(AdaptiveToolbar).first),
          'tableRow': _rect(tester, cell(0)),
          'itemLabelFont': tester
              .widget<Text>(
                find.descendant(of: item, matching: find.text('设为活动')),
              )
              .style
              ?.fontSize,
          'rootContextItemCount': find.byType(MenuItemButton).evaluate().length,
        }, passed: (itemRect.left - pointA.dx).abs() < 32);
        await _shot(tester, evidenceDir, '01-right-menu');

        await mouse.moveTo(const Offset(80, 190));
        await _settle(tester);
        record('pointer-leaves-menu', {'menuOpen': _menuOpen(tester)});
        await tester.tapAt(pointB, kind: PointerDeviceKind.mouse);
        await _settle(tester);
        record('left-click-another-row-dismisses-menu', {
          'menuOpen': _menuOpen(tester),
          'selected': container
              .read(profilesControllerProvider)
              .selected
              .toList(),
        }, passed: !_menuOpen(tester));
        await _shot(tester, evidenceDir, '02-click-other-row');

        final selectionBeforeEscape = Set<String>.of(
          container.read(profilesControllerProvider).selected,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _settle(tester);
        final selectionAfterEscape = container
            .read(profilesControllerProvider)
            .selected;
        record(
          'escape-closes-menu-without-clearing-selection',
          {
            'menuOpen': _menuOpen(tester),
            'selectedBefore': selectionBeforeEscape.toList(),
            'selectedAfter': selectionAfterEscape.toList(),
          },
          passed:
              !_menuOpen(tester) &&
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
        await _right(tester, tester.getCenter(cell(2)));
        record('right-click-different-row-while-menu-open', {
          'selected': container
              .read(profilesControllerProvider)
              .selected
              .toList(),
          'menuOpen': _menuOpen(tester),
          'firstItem': _rect(tester, item),
        });
        await closeByOutside();

        await _right(
          tester,
          tester.getCenter(find.byKey(ValueKey('handle-${rows[0].id}'))),
        );
        record('right-click-row-header', {
          'menuOpen': _menuOpen(tester),
        }, passed: _menuOpen(tester));
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
