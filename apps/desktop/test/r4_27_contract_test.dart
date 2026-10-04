// R4-27 contract: storage migration and failure recovery (UI-state half).
//
// Synthetic only: temp directories, no native library, no network, no user data
// directory and no port use.
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

void main() {
  late Directory temp;

  setUp(() {
    temp = Directory.systemTemp.createTempSync('r4_27_contract_');
  });

  tearDown(() {
    if (temp.existsSync()) temp.deleteSync(recursive: true);
  });

  test(
    'default UI state path lives in the data directory, not the exe dir',
    () {
      final dataPath = FileUiStateStore.resolveDefaultPath(
        dataDirOverride: temp.path,
      );
      expect(dataPath, '${temp.path}${Platform.pathSeparator}ui_state.json');
      expect(
        FileUiStateStore.resolveDefaultPath(),
        isNot(FileUiStateStore.resolveLegacyPath()),
        reason: 'layout state must persist in the writable data directory',
      );
    },
  );

  test('writes are atomic (temp file + rename, no temp left behind)', () {
    final path = '${temp.path}${Platform.pathSeparator}ui_state.json';
    final store = FileUiStateStore(overridePath: path);
    store.saveSection('window', <String, dynamic>{'width': 800});
    expect(store.loadSection('window'), <String, dynamic>{'width': 800});
    expect(File(path).existsSync(), isTrue);
    expect(File('$path.tmp').existsSync(), isFalse);
  });

  test('a write failure is visible instead of silently swallowed', () {
    final file = File('${temp.path}${Platform.pathSeparator}not_a_dir');
    file.writeAsStringSync('x');
    final failures = <Object>[];
    final store = FileUiStateStore(
      overridePath: '${file.path}${Platform.pathSeparator}ui_state.json',
      onError: (error, _) => failures.add(error),
    );
    store.saveSection('window', <String, dynamic>{'width': 800});
    expect(failures, isNotEmpty);
    expect(store.lastWriteError, isNotNull);
  });

  test(
    'legacy exe-adjacent ui_state.json is migrated into the data directory',
    () {
      final legacy = File('${temp.path}${Platform.pathSeparator}legacy.json');
      legacy.writeAsStringSync(
        jsonEncode(<String, dynamic>{
          'window': <String, dynamic>{'width': 1024},
        }),
      );
      final primaryPath =
          '${temp.path}${Platform.pathSeparator}data'
          '${Platform.pathSeparator}ui_state.json';
      final store = FileUiStateStore(
        overridePath: primaryPath,
        legacyOverridePath: legacy.path,
      );
      expect(store.loadSection('window'), <String, dynamic>{'width': 1024});
      expect(File(primaryPath).existsSync(), isTrue);
    },
  );

  test('section saves preserve unknown top-level keys', () {
    final path = '${temp.path}${Platform.pathSeparator}ui_state.json';
    final store = FileUiStateStore(overridePath: path);
    store.saveDocument(<String, dynamic>{
      'meta': <String, dynamic>{'schema_version': 2},
      'unknown_future_key': <String, dynamic>{'keep': true},
    });
    store.saveSection('window', <String, dynamic>{'width': 640});
    final reread = FileUiStateStore(overridePath: path).loadDocument();
    expect(reread['unknown_future_key'], <String, dynamic>{'keep': true});
    expect(reread['meta'], <String, dynamic>{'schema_version': 2});
  });
}
