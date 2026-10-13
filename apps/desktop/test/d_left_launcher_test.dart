// D-LEFT D1-09: the OS opener is explorer.exe (no shell); only http(s) URLs
// and existing absolute file paths may reach it. No real launches: every
// process assertion below goes through a recording fake.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/custom_editor_dialog.dart';
import 'package:v2rayn_desktop/features/settings/settings_actions.dart';

Future<ProcessResult> _exit(
  List<List<String>> calls,
  String program,
  List<String> args, {
  int exitCode = 0,
}) async {
  calls.add(<String>[program, ...args]);
  return ProcessResult(1, exitCode, '', '');
}

void main() {
  test('isSafeExternalOpenPath allows bang, spaces, CJK, parens', () {
    expect(isSafeExternalOpenPath(r'C:\data\a!b.json'), isTrue);
    expect(isSafeExternalOpenPath(r'C:\数据\my file (1).json'), isTrue);
    expect(isSafeExternalOpenPath(r'C:\data\my-file_v2.json'), isTrue);
  });

  test('isSafeExternalOpenPath rejects UNC, device, flags, bare names', () {
    expect(isSafeExternalOpenPath(r'\\srv\share\a.json'), isFalse);
    expect(isSafeExternalOpenPath(r'\\.\C:\a.json'), isFalse);
    expect(isSafeExternalOpenPath(r'\\?\C:\a.json'), isFalse);
    expect(isSafeExternalOpenPath('/etc/a.json'), isFalse);
    expect(isSafeExternalOpenPath('-a.json'), isFalse);
    expect(isSafeExternalOpenPath('my-node.json'), isFalse);
    expect(isSafeExternalOpenPath(''), isFalse);
  });

  test('isSafeExternalOpenPath keeps the metacharacter rejections', () {
    for (final bad in <String>[
      r'C:\x" & evil',
      r'C:\x|evil',
      r'C:\x;evil',
      r'C:\x`evil`',
      r'C:\x%PATH%',
      r'C:\x^c',
      'C:\\x\$evil',
      'C:\\x\nevil',
      'C:\\x\revil',
    ]) {
      expect(isSafeExternalOpenPath(bad), isFalse, reason: bad);
    }
  });

  test(
    'openConfigFileExternally launches explorer for an existing path',
    () async {
      if (!Platform.isWindows) {
        markTestSkipped('Windows-only opener');
        return;
      }
      final dir = Directory.systemTemp.createTempSync('dleft_open_');
      addTearDown(() {
        try {
          dir.deleteSync(recursive: true);
        } catch (_) {}
      });
      final file = File('${dir.path}${Platform.pathSeparator}my file ! 数据.json')
        ..writeAsStringSync('{}');
      final calls = <List<String>>[];
      Future<ProcessResult> recording(String program, List<String> args) =>
          _exit(
            calls,
            program,
            args,
            exitCode: 1,
          ); // explorer exits 1 on success

      expect(
        await openConfigFileExternally(file.path, launcher: recording),
        isTrue,
      );
      expect(calls.single.first, 'explorer');
      expect(calls.single.sublist(1), <String>[file.path]);
    },
    skip: !Platform.isWindows,
  );

  test('openConfigFileExternally refuses missing files and UNC', () async {
    final calls = <List<String>>[];
    Future<ProcessResult> recording(String program, List<String> args) =>
        _exit(calls, program, args);

    expect(
      await openConfigFileExternally(
        r'C:\nope\dleft-missing.json',
        launcher: recording,
      ),
      isFalse,
    );
    expect(
      await openConfigFileExternally(
        r'\\srv\share\a.json',
        launcher: recording,
      ),
      isFalse,
    );
    expect(calls, isEmpty);
  });

  test('openCoreWebsite launches explorer and ignores its exit code', () async {
    if (!Platform.isWindows) {
      markTestSkipped('Windows-only opener');
      return;
    }
    final calls = <List<String>>[];
    Future<ProcessResult> recording(String program, List<String> args) =>
        _exit(calls, program, args, exitCode: 1);

    expect(
      await openCoreWebsite(coreName: 'Xray', launcher: recording),
      isTrue,
    );
    expect(calls.single, <String>[
      'explorer',
      'https://github.com/XTLS/Xray-core',
    ]);
    expect(
      await openCoreWebsite(coreName: 'Nope', launcher: recording),
      isFalse,
    );
    expect(calls, hasLength(1));
  }, skip: !Platform.isWindows);
}
