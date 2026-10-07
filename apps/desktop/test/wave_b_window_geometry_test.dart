// Wave B / FLD-CFG-156/157/158 (controller/state level, local-only).
//
// TypeName window-type locate + Width/Height geometry restore across
// save/reopen and multi-window, through the canonical `UiItem.WindowSizeItem`
// rows. Synthetic data only: `SyntheticBridgePort` + `MemoryUiStateStore`, no
// native library, window, port, or OS side effect.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/profiles/window_geometry.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

ProviderContainer _container(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(4),
  ],
);

List<Map<String, Object?>> _rows(List<WindowGeometry> geometries) =>
    encodeWindowSizeItems(geometries);

void main() {
  group('FLD-CFG-156 locate by TypeName', () {
    test('exact match locates; absent key returns null', () {
      final rows = parseWindowSizeItems(<Map<String, Object?>>[
        <String, Object?>{
          'TypeName': 'MainWindow',
          'Width': 1200,
          'Height': 800,
        },
        <String, Object?>{
          'TypeName': 'OptionSettingWindow',
          'Width': 900,
          'Height': 600,
        },
      ]);
      final main = locateWindowSize(rows, 'MainWindow');
      expect(main, isNotNull);
      expect((main!.width, main.height), (1200, 800));
      expect(locateWindowSize(rows, 'OptionSettingWindow')!.width, 900);
      expect(locateWindowSize(rows, 'UnknownWindow'), isNull);
    });

    test('match is case-sensitive; first row wins', () {
      final rows = parseWindowSizeItems(<Map<String, Object?>>[
        <String, Object?>{'TypeName': 'MainWindow', 'Width': 1, 'Height': 1},
        <String, Object?>{'TypeName': 'MainWindow', 'Width': 2, 'Height': 2},
      ]);
      expect(locateWindowSize(rows, 'mainwindow'), isNull);
      expect(locateWindowSize(rows, 'MAINWINDOW'), isNull);
      final found = locateWindowSize(rows, 'MainWindow')!;
      expect((found.width, found.height), (1, 1));
    });

    test('degenerate rows are treated as absent', () {
      final rows = parseWindowSizeItems(<Map<String, Object?>>[
        <String, Object?>{'TypeName': 'MainWindow', 'Width': 0, 'Height': 0},
        <String, Object?>{
          'TypeName': 'OptionSettingWindow',
          'Width': -4,
          'Height': 600,
        },
      ]);
      expect(locateWindowSize(rows, 'MainWindow'), isNull);
      expect(locateWindowSize(rows, 'OptionSettingWindow'), isNull);
    });

    test('unknown TypeName rows are retained, empty names refused', () {
      final rows = parseWindowSizeItems(<Object?>[
        <String, Object?>{'TypeName': 'MysteryWindow', 'Width': 7, 'Height': 8},
        <String, Object?>{'TypeName': '', 'Width': 1, 'Height': 1},
        'not-a-map',
        <String, Object?>{'TypeName': 'MainWindow', 'Width': 3, 'Height': 4},
      ]);
      expect(rows.map((r) => r.typeName), ['MysteryWindow', 'MainWindow']);
      // Retained rows stay locatable by their own key; no live window asks
      // for them, but a save must not drop them.
      expect(locateWindowSize(rows, 'MysteryWindow')!.width, 7);
      final encoded = _rows(rows);
      expect(encoded.any((row) => row['TypeName'] == 'MysteryWindow'), isTrue);
    });
  });

  group('FLD-CFG-157/158 restore + fallback', () {
    test('resolve returns stored size, fallback is never zero-size', () {
      final rows = parseWindowSizeItems(<Map<String, Object?>>[
        <String, Object?>{
          'TypeName': 'MainWindow',
          'Width': 1440,
          'Height': 900,
        },
      ]);
      final stored = resolveWindowSize(rows, 'MainWindow');
      expect((stored.width, stored.height), (1440, 900));
      for (final missing in ['OptionSettingWindow', 'UnknownWindow']) {
        final fallback = resolveWindowSize(rows, missing);
        expect(fallback.width, greaterThan(0));
        expect(fallback.height, greaterThan(0));
        expect(
          (fallback.width, fallback.height),
          (fallbackWindowWidth, fallbackWindowHeight),
        );
      }
      final degenerate = resolveWindowSize(
        parseWindowSizeItems(<Map<String, Object?>>[
          <String, Object?>{'TypeName': 'MainWindow', 'Width': 0, 'Height': 0},
        ]),
        'MainWindow',
      );
      expect((degenerate.width, degenerate.height), (1200, 800));
    });

    test('upsert replaces in place, appends unknown, keeps siblings', () {
      final rows = parseWindowSizeItems(<Map<String, Object?>>[
        <String, Object?>{
          'TypeName': 'MainWindow',
          'Width': 1200,
          'Height': 800,
        },
        <String, Object?>{
          'TypeName': 'OptionSettingWindow',
          'Width': 900,
          'Height': 600,
        },
      ]);
      final updated = upsertWindowSize(rows, 'MainWindow', 1440, 900);
      expect(updated.length, 2);
      expect((updated[0].width, updated[0].height), (1440, 900));
      // Sibling window untouched: multi-window independence.
      expect((updated[1].width, updated[1].height), (900, 600));

      final appended = upsertWindowSize(updated, 'DNSSettingWindow', 640, 480);
      expect(appended.length, 3);
      expect((appended[2].width, appended[2].height), (640, 480));
    });
  });

  group('save/reopen through the UiItem group (fake bridge)', () {
    test('resize -> save -> reopen restores Width and Height', () {
      final bridge = SyntheticBridgePort(count: 4);
      final container = _container(bridge);
      addTearDown(container.dispose);
      final controller = container.read(settingsControllerProvider.notifier);
      expect(controller.load().loaded, isTrue);

      // First open: nothing stored yet, fallback applies.
      var document = controller.draft();
      expect(
        resolveWindowSize(readWindowSizes(document), 'MainWindow').width,
        fallbackWindowWidth,
      );

      // Resize in two steps; each re-store goes through the UiItem group.
      for (final size in [(1200, 800), (1440, 900)]) {
        document = controller.draft();
        final ui = uiGroupWithWindowSize(
          document,
          'MainWindow',
          size.$1,
          size.$2,
        );
        expect(controller.saveGroup('UiItem', ui).ok, isTrue);
      }

      // Reopen against the same bridge: the stored geometry comes back.
      final reopened = _container(bridge);
      addTearDown(reopened.dispose);
      reopened.read(settingsControllerProvider.notifier).load();
      final restored = resolveWindowSize(
        readWindowSizes(reopened.read(settingsControllerProvider).document),
        'MainWindow',
      );
      expect((restored.width, restored.height), (1440, 900));
    });

    test('windows stay independent; unknown rows survive save/reopen', () {
      final bridge = SyntheticBridgePort(count: 4);
      final container = _container(bridge);
      addTearDown(container.dispose);
      final controller = container.read(settingsControllerProvider.notifier);
      expect(controller.load().loaded, isTrue);

      // Seed two windows plus an unknown row from a backup restore.
      var ui = uiGroupWithWindowSize(
        controller.draft(),
        'MainWindow',
        1200,
        800,
      );
      var rows = parseWindowSizeItems(ui['WindowSizeItem']);
      rows = upsertWindowSize(rows, 'OptionSettingWindow', 900, 600);
      rows = upsertWindowSize(rows, 'LegacyDialog', 640, 480);
      ui['WindowSizeItem'] = _rows(rows);
      expect(controller.saveGroup('UiItem', ui).ok, isTrue);

      // Resize only the main window; the option window must not move.
      final resized = uiGroupWithWindowSize(
        controller.draft(),
        'MainWindow',
        1600,
        1000,
      );
      expect(controller.saveGroup('UiItem', resized).ok, isTrue);

      final reopened = _container(bridge);
      addTearDown(reopened.dispose);
      reopened.read(settingsControllerProvider.notifier).load();
      final document = reopened.read(settingsControllerProvider).document;
      final stored = readWindowSizes(document);
      final main = resolveWindowSize(stored, 'MainWindow');
      final option = resolveWindowSize(stored, 'OptionSettingWindow');
      expect((main.width, main.height), (1600, 1000));
      expect((option.width, option.height), (900, 600));
      // Unknown row retained verbatim across the whole loop.
      expect(locateWindowSize(stored, 'LegacyDialog')!.width, 640);
      // A window that was never stored still falls back, never zero-size.
      final dns = resolveWindowSize(stored, 'DNSSettingWindow');
      expect(dns.width, greaterThan(0));
      expect(dns.height, greaterThan(0));
    });
  });
}
