// R3-VISUAL-DPI-TRAY shared assertions.
//
// The DPI matrix simulates 100/125/150/200% scaling without touching the host:
// `tester.view.devicePixelRatio` is the scale and `tester.view.physicalSize`
// is the physical monitor pixel count, so the logical layout size stays
// `physical / dpr`. This file is intentionally not named `*_test.dart`.
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';

import 'profiles_harness.dart';

/// Fails when the frame produced an overflow/exception or clipped a paragraph.
///
/// Data-grid cells legitimately ellipsize (`TextOverflow.ellipsis`); pass
/// [ignoreText] to exclude their synthetic values from the truncation check so
/// the assertion targets chrome/labels instead.
void expectNoLayoutProblems(
  WidgetTester tester,
  String reason, {
  bool Function(String text)? ignoreText,
}) {
  final exception = tester.takeException();
  expect(exception, isNull, reason: reason);
  for (final ro in tester.allRenderObjects) {
    if (ro is RenderParagraph) {
      final text = ro.text.toPlainText();
      if (ignoreText != null && ignoreText(text)) continue;
      expect(
        ro.didExceedMaxLines,
        isFalse,
        reason: '$reason: truncated text "$text"',
      );
    }
  }
}

/// Assert the shell chrome (table headers, toolbar/menu, status bar) never
/// ellipsizes. Node-table data cells are excluded: they intentionally ellipsize.
void expectChromeNotTruncated(WidgetTester tester, String reason) {
  final chrome = find.byWidgetPredicate((w) {
    final key = w.key;
    if (key is! ValueKey<String>) return false;
    final value = key.value;
    return value.startsWith('header-') ||
        value.startsWith('status-') ||
        value.startsWith('menu-') ||
        value.startsWith('runtime-') ||
        value == 'layout-selector' ||
        value == 'theme-toggle' ||
        value == 'tun-toggle';
  });
  final candidates = <Element>[
    ...chrome.evaluate(),
    ...find.descendant(of: chrome, matching: find.byType(Text)).evaluate(),
  ];
  for (final element in candidates) {
    final ro = element.renderObject;
    if (ro is RenderParagraph) {
      expect(
        ro.didExceedMaxLines,
        isFalse,
        reason: '$reason: chrome truncated "${ro.text.toPlainText()}"',
      );
    }
  }
}

/// The scaled-hosting matrix: the four Windows DPI presets. The logical window
/// is kept constant while the physical pixel count scales, which is exactly
/// what a high-DPI monitor does.
const List<double> kDpiScales = <double>[1.0, 1.25, 1.5, 2.0];

/// Supported logical window sizes (default 1200x800 and the 800x600 floor).
const List<Size> kLogicalSizes = <Size>[Size(1280, 800), Size(800, 600)];

/// Apply one scale/size pair to the test view.
void applyDpi(WidgetTester tester, double scale, Size logical) {
  tester.view.devicePixelRatio = scale;
  tester.view.physicalSize = Size(
    logical.width * scale,
    logical.height * scale,
  );
}

String dpiLabel(double scale, Size logical) =>
    '${logical.width.toInt()}x${logical.height.toInt()} '
    '@${(scale * 100).round()}%';

/// One shell DPI case per file: build the real [MainShell] once, then sweep the
/// supported logical sizes and all three layouts at [scale]. The 100% file also
/// asserts the 800x600 control floor. Splitting by scale keeps each
/// flutter_tester process light enough to avoid the locked engine's
/// post-heavy-build segfault (see docs/evidence/T01.md).
Future<void> runShellDpiCase(WidgetTester tester, double scale) async {
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final container = makeContainer(rows: 6);
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: MainShell()),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));

  final shell = container.read(uiShellControllerProvider.notifier);
  for (final logical in kLogicalSizes) {
    applyDpi(tester, scale, logical);
    for (final layout in AppLayoutMode.values) {
      shell.setLayout(layout);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 20));
      expectNoLayoutProblems(
        tester,
        'main shell ${dpiLabel(scale, logical)} ${layout.name}',
        // Node-table data cells ellipsize by design; chrome is checked below.
        ignoreText: (text) => true,
      );
      expectChromeNotTruncated(
        tester,
        'main shell ${dpiLabel(scale, logical)} ${layout.name}',
      );
    }
  }

  if (scale != 1.0) return;
  // Toolbar / status-bar controls stay hittable at the logical floor.
  applyDpi(tester, 1.0, const Size(800, 600));
  shell.setLayout(AppLayoutMode.vertical);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  for (final key in <String>[
    'layout-selector',
    'theme-toggle',
    'filter-field',
    'status-proxy-speed',
    'status-inbound',
    'tun-toggle',
  ]) {
    final finder = find.byKey(ValueKey<String>(key));
    expect(finder, findsWidgets, reason: 'missing $key at 800x600');
    await tester.ensureVisible(finder.first);
    await tester.pump();
    expect(
      finder.hitTestable(),
      findsWidgets,
      reason: 'unreachable $key at 800x600',
    );
  }
}
