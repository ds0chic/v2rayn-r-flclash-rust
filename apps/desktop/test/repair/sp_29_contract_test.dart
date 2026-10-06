// SP-29 gate classification contract.
//
// The reproducible gate (`tools/gates/run_all.ps1`) runs every Flutter test
// file in its own process and must never report a silent incomplete case:
// a native tester crash (exit 79 / "did not complete") is `incomplete`, a
// completed run with assertion failures is `fail`, and only a fully
// completed green run is `pass`. This file pins that contract in pure Dart
// (no widgets, no ports, no OS side effects) so the classifier itself is
// covered by `flutter test`.
//
// The implementation below mirrors the PowerShell classifier line by line;
// any rule change must update both files together.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// Mirrors `Get-AttemptClass` in `tools/gates/run_all.ps1`.
String classifyGateAttempt({required int exitCode, required String output}) {
  const incompleteMarkers = [
    'did not complete',
    'No tests were found',
    'EXCEPTION_ACCESS_VIOLATION',
    'Segmentation fault',
    'Connection closed before test suite loaded',
    'Failed to load "',
  ];
  for (final marker in incompleteMarkers) {
    if (output.contains(marker)) {
      return 'incomplete';
    }
  }
  // Native tester crashes surface as these process exits even when the last
  // printed line looks like an ordinary failure.
  if (exitCode == 79 || exitCode == -1073741819) {
    return 'incomplete';
  }
  if (exitCode == 0) {
    // Expanded reporter trailing line is e.g. "+3 ~1: All tests passed!".
    final done = RegExp(r'\+(\d+)( ~\d+)?: All tests passed!');
    final match = done.firstMatch(output);
    if (match != null && match.group(1) == '0') {
      return 'skip';
    }
    return 'pass';
  }
  return 'fail';
}

/// Mirrors the gate inventory check: every enumerated file must have exactly
/// one result entry, otherwise the run has a silent incomplete case.
List<String> filesWithoutResult({
  required List<String> enumerated,
  required List<String> withResult,
}) {
  final covered = withResult.toSet();
  return enumerated.where((f) => !covered.contains(f)).toList();
}

void main() {
  group('SP-29 gate attempt classification', () {
    test('exit 79 with did-not-complete is incomplete, not fail', () {
      const log =
          '00:02 +0: settings/routing windows survive the 100-200% DPI matrix - did not complete [E]\n'
          '00:02 +0: Some tests failed.\n'
          'No tests were found.\n';
      expect(classifyGateAttempt(exitCode: 79, output: log), 'incomplete');
    });

    test('exit 1 with did-not-complete is incomplete, not fail', () {
      // t11_menu retry shape: exit 1 but the run never completed.
      const log =
          '00:00 +0: menu model carries the routing and DNS entries\n'
          '00:00 +1: menu opens the routing window; status bar shows the switch\n'
          '00:01 +1: menu opens the routing window; status bar shows the switch - did not complete [E]\n'
          '00:01 +1: Some tests failed.\n';
      expect(classifyGateAttempt(exitCode: 1, output: log), 'incomplete');
    });

    test('load-time tester crash with exit 1 is incomplete, not fail', () {
      // r4_06 shape: the tester dies before the suite loads; exit is 1 and
      // no test completed, so this must not count as an assertion failure.
      const log =
          '00:00 +0 -1: loading C:/repo/apps/desktop/test/r4_06_contract_test.dart [E]\n'
          '  Failed to load "C:/repo/apps/desktop/test/r4_06_contract_test.dart": Connection closed before test suite loaded.\n'
          '00:00 +0 -1: Some tests failed.\n';
      expect(classifyGateAttempt(exitCode: 1, output: log), 'incomplete');
    });

    test('completed assertion failure is fail', () {
      const log =
          '00:01 +0: context menu lifecycle and captured target [E]\n'
          '  Expected: <selected:{c}>\n'
          '    Actual: <selected:{a, b}>\n'
          '00:01 +0: Some tests failed.\n';
      expect(classifyGateAttempt(exitCode: 1, output: log), 'fail');
    });

    test('completed green run is pass', () {
      const log = '00:03 +1: All tests passed!\n';
      expect(classifyGateAttempt(exitCode: 0, output: log), 'pass');
    });

    test('all-skipped run is skip', () {
      const log = '00:01 +0 ~1: All tests passed!\n';
      expect(classifyGateAttempt(exitCode: 0, output: log), 'skip');
    });

    test('bare crash exit without output is incomplete', () {
      expect(classifyGateAttempt(exitCode: 79, output: ''), 'incomplete');
      expect(
        classifyGateAttempt(exitCode: -1073741819, output: ''),
        'incomplete',
      );
    });
  });

  group('SP-29 inventory completeness', () {
    test('missing result entry is detected, never silent', () {
      final missing = filesWithoutResult(
        enumerated: ['test/a_test.dart', 'test/repair/b_test.dart'],
        withResult: ['test/a_test.dart'],
      );
      expect(missing, ['test/repair/b_test.dart']);
    });

    test('full coverage reports nothing missing', () {
      final missing = filesWithoutResult(
        enumerated: ['test/a_test.dart'],
        withResult: ['test/a_test.dart'],
      );
      expect(missing, isEmpty);
    });

    test('recursive inventory covers nested test dirs', () {
      // The pre-SP-29 retry script enumerated only top-level test files and
      // silently skipped test/repair/*. The gate must enumerate recursively.
      final topLevel = ['test/a_test.dart'];
      final recursive = ['test/a_test.dart', 'test/repair/sp_29_test.dart'];
      expect(filesWithoutResult(enumerated: recursive, withResult: topLevel), [
        'test/repair/sp_29_test.dart',
      ]);
    });
  });

  group('SP-29 gate entry presence', () {
    test('tools/gates/run_all.ps1 exists with crash/toolchain handling', () {
      final gate = File('../../tools/gates/run_all.ps1');
      expect(gate.existsSync(), isTrue, reason: 'SP-29 gate entry missing');
      final text = gate.readAsStringSync();
      expect(text, contains('did not complete'));
      expect(text, contains('Connection closed before test suite loaded'));
      expect(text, contains('toolchain'));
      expect(text, contains('incomplete'));
    });
  });
}
