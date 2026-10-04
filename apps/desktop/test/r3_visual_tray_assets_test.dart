// R3-VISUAL-DPI-TRAY: the four TrayIconStatus values must resolve to real,
// packaged, visually distinct multi-size .ico assets. Reads the asset files
// from the package working directory; no native bridge, no network.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/tray_menu_model.dart';

/// ICO entry widths (0 means 256) from the ICONDIR directory.
List<int> icoSizes(List<int> bytes) {
  final count = bytes[4] | (bytes[5] << 8);
  final sizes = <int>[];
  for (var i = 0; i < count; i++) {
    final off = 6 + 16 * i;
    final w = bytes[off] == 0 ? 256 : bytes[off];
    sizes.add(w);
  }
  return sizes;
}

String fingerprint(List<int> bytes) {
  var h = 17;
  for (final b in bytes) {
    h = (h * 31 + b) & 0x7fffffff;
  }
  return '${bytes.length}:$h';
}

void main() {
  group('R3-VISUAL-DPI-TRAY packaged tray assets', () {
    test('pubspec declares the tray asset directory', () {
      final pubspec = File('pubspec.yaml').readAsStringSync();
      expect(pubspec, contains('assets/tray/'));
    });

    test('every status maps to a packaged multi-size .ico', () {
      final names = <String>[];
      for (final status in TrayIconStatus.values) {
        final name = trayIconResourceName(status);
        names.add(name);
        final file = File('assets/tray/$name');
        expect(file.existsSync(), isTrue, reason: 'missing asset $name');
        final bytes = file.readAsBytesSync();
        expect(bytes[0] | (bytes[1] << 8), 0, reason: '$name reserved');
        expect(bytes[2] | (bytes[3] << 8), 1, reason: '$name type=icon');
        final sizes = icoSizes(bytes);
        expect(
          sizes,
          containsAll(<int>[16, 32]),
          reason: '$name sizes=$sizes (need 16 and 32)',
        );
      }
      expect(names.toSet(), hasLength(TrayIconStatus.values.length));
    });

    test('the four status icons are not byte-identical', () {
      final digests = <String>{
        for (final status in TrayIconStatus.values)
          fingerprint(
            File('assets/tray/${trayIconResourceName(status)}')
                .readAsBytesSync(),
          ),
      };
      expect(digests, hasLength(TrayIconStatus.values.length));
    });

    test('declared fallback resource stays an .ico name', () {
      expect(trayIconFallbackName, endsWith('.ico'));
    });
  });
}
