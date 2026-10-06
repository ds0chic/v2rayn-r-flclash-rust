// SP-19: main-window chrome (menu/toolbar/table header/status bar) stays
// readable and operable with an 800px-high window at 100/125/150/200% DPI.
//
// Correct contract: the top menu + runtime toolbar, the table header row and
// the bottom bar never overflow and their key entries stay hittable at the
// 800px height floor. Fonts follow the original tokens (no shrink-to-fit).
// One pumpWidget per file (locked flutter_tester segfault note in
// support/profiles_harness.dart); the DPI sweep reuses the single build.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/main_shell.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';

import '../support/dpi_assertions.dart';
import '../support/profiles_harness.dart';

const _scales = <double>[1.0, 1.25, 1.5, 2.0];

/// 800px-high logical windows (upstream default height) plus the 800px-wide
/// minimum, so both the height floor and the width floor are covered.
const _logicalSizes = <Size>[Size(800, 800), Size(1200, 800), Size(1280, 800)];

/// Shell-owned key controls that must stay operable at the 800px height floor.
const _chromeKeys = <String>[
  'runtime-start',
  'runtime-stop',
  'layout-selector',
  'theme-toggle',
  'status-inbound',
  'status-proxy-speed',
  'tun-toggle',
];

void main() {
  testWidgets('main shell chrome fits 800px-high windows at 100-200% DPI', (
    tester,
  ) async {
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
    for (final scale in _scales) {
      for (final logical in _logicalSizes) {
        final tag =
            '${logical.width.toInt()}x${logical.height.toInt()} '
            '@${(scale * 100).round()}%';
        tester.view.devicePixelRatio = scale;
        tester.view.physicalSize = Size(
          logical.width * scale,
          logical.height * scale,
        );
        shell.setLayout(AppLayoutMode.vertical);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 20));
        expect(
          tester.takeException(),
          isNull,
          reason: 'layout exception at $tag',
        );
        // Chrome labels/headers never truncate at the height floor.
        expectChromeNotTruncated(tester, 'main shell $tag');
        for (final key in _chromeKeys) {
          final finder = find.byKey(ValueKey<String>(key));
          expect(finder, findsWidgets, reason: 'missing $key at $tag');
          expect(
            finder.hitTestable(),
            findsWidgets,
            reason: 'unreachable $key at $tag',
          );
        }
        // Table header row keeps its upstream height and stays on screen.
        final header = find.byKey(const ValueKey<String>('header-handle'));
        expect(header, findsWidgets, reason: 'missing table header at $tag');
        final rect = tester.getRect(header.first);
        expect(
          rect.top,
          greaterThanOrEqualTo(0),
          reason: 'table header off screen at $tag: $rect',
        );
      }
    }
  });
}
