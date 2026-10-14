import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/evidence_hooks.dart';

void main() {
  group('evidenceMonitorXrayPort', () {
    test('unarmed build ignores the variable entirely', () {
      expect(
        evidenceMonitorXrayPort(<String, String>{
          'V2RAYN_R_MONITOR_XRAY_PORT': '11808',
        }, armed: false),
        isNull,
      );
    });

    test('unset variable is a no-op when armed', () {
      expect(evidenceMonitorXrayPort(<String, String>{}, armed: true), isNull);
    });

    test('reserved user-proxy port 10808 is refused', () {
      expect(
        evidenceMonitorXrayPort(<String, String>{
          'V2RAYN_R_MONITOR_XRAY_PORT': '10808',
        }, armed: true),
        isNull,
      );
    });

    test('ports below the test range and garbage are refused', () {
      for (final raw in <String>[
        '0',
        '80',
        '10807',
        '11799',
        'abc',
        '',
        '99999',
      ]) {
        expect(
          evidenceMonitorXrayPort(<String, String>{
            'V2RAYN_R_MONITOR_XRAY_PORT': raw,
          }, armed: true),
          isNull,
          reason: 'raw=$raw',
        );
      }
    });

    test('valid evidence port is accepted when armed', () {
      expect(
        evidenceMonitorXrayPort(<String, String>{
          'V2RAYN_R_MONITOR_XRAY_PORT': '11808',
        }, armed: true),
        11808,
      );
    });
  });

  group('evidenceStringHook', () {
    test('unarmed build ignores layout/theme hooks', () {
      const env = <String, String>{
        'V2RAYN_R_LAYOUT': 'horizontal',
        'V2RAYN_R_THEME': 'dark',
      };
      expect(evidenceStringHook(env, 'V2RAYN_R_LAYOUT', armed: false), isNull);
      expect(evidenceStringHook(env, 'V2RAYN_R_THEME', armed: false), isNull);
    });

    test('armed build passes the value through', () {
      const env = <String, String>{
        'V2RAYN_R_LAYOUT': 'horizontal',
        'V2RAYN_R_THEME': 'dark',
      };
      expect(
        evidenceStringHook(env, 'V2RAYN_R_LAYOUT', armed: true),
        'horizontal',
      );
      expect(evidenceStringHook(env, 'V2RAYN_R_THEME', armed: true), 'dark');
      expect(
        evidenceStringHook(
          const <String, String>{},
          'V2RAYN_R_LAYOUT',
          armed: true,
        ),
        isNull,
      );
    });
  });
}
