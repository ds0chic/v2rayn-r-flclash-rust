// R3-VISUAL-DPI-TRAY shared assertions.
//
// The DPI matrix simulates 100/125/150/200% scaling without touching the host:
// `tester.view.devicePixelRatio` is the scale and `tester.view.physicalSize`
// is the physical monitor pixel count, so the logical layout size stays
// `physical / dpr`. This file is intentionally not named `*_test.dart`.
import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

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
