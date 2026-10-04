import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';

void main() {
  group('shortcut mapping', () {
    String? map(
      LogicalKeyboardKey key, {
      bool ctrl = false,
      bool shift = false,
    }) => actionForKey(key: key, ctrl: ctrl, shift: shift, alt: false);

    test('ctrl shortcuts', () {
      expect(map(LogicalKeyboardKey.keyA, ctrl: true), ProfileAction.selectAll);
      expect(
        map(LogicalKeyboardKey.keyC, ctrl: true),
        ProfileAction.exportShareUrl,
      );
      expect(map(LogicalKeyboardKey.keyD, ctrl: true), ProfileAction.edit);
      expect(map(LogicalKeyboardKey.keyF, ctrl: true), ProfileAction.share);
      expect(map(LogicalKeyboardKey.keyO, ctrl: true), ProfileAction.tcping);
      expect(map(LogicalKeyboardKey.keyR, ctrl: true), ProfileAction.realping);
      expect(map(LogicalKeyboardKey.keyT, ctrl: true), ProfileAction.speedtest);
      expect(map(LogicalKeyboardKey.keyE, ctrl: true), ProfileAction.mixedTest);
    });

    test('plain keys', () {
      expect(map(LogicalKeyboardKey.enter), ProfileAction.activate);
      expect(map(LogicalKeyboardKey.delete), ProfileAction.delete);
      expect(map(LogicalKeyboardKey.backspace), ProfileAction.delete);
      expect(map(LogicalKeyboardKey.keyT), ProfileAction.moveTop);
      expect(map(LogicalKeyboardKey.keyU), ProfileAction.moveUp);
      expect(map(LogicalKeyboardKey.keyD), ProfileAction.moveDown);
      expect(map(LogicalKeyboardKey.keyB), ProfileAction.moveBottom);
      expect(map(LogicalKeyboardKey.escape), ProfileAction.escape);
    });

    test('unmapped and modifier-only combos are ignored', () {
      expect(map(LogicalKeyboardKey.keyA), isNull);
      expect(map(LogicalKeyboardKey.keyZ, ctrl: true), isNull);
      expect(map(LogicalKeyboardKey.f5, shift: true), isNull);
    });
  });

  group('selection helpers', () {
    final rows = SyntheticBridgePort().generate(10);

    test('single selection replaces the set', () {
      expect(selectSingle('a'), <String>{'a'});
    });

    test('ctrl toggle adds then removes', () {
      var selection = toggleSelection(const <String>{'a'}, 'b');
      expect(selection, <String>{'a', 'b'});
      selection = toggleSelection(selection, 'a');
      expect(selection, <String>{'b'});
    });

    test('shift range is inclusive and anchored on the main row', () {
      final range = extendSelection(
        rows,
        const <String>{'syn-000002'},
        'syn-000002',
        'syn-000005',
      );
      expect(range.length, 4);
      expect(range.contains('syn-000002'), isTrue);
      expect(range.contains('syn-000005'), isTrue);
    });
  });
}
