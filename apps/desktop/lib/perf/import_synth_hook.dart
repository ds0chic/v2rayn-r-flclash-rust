import 'dart:convert';
import 'dart:io';

import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';

/// Armed-only synthetic import evidence hook (FLD-CFG-001/002).
///
/// Enabled only by `--dart-define=V2RAYN_R_SMOKE_ARMED=true` **plus**
/// `V2RAYN_R_IMPORT_SYNTHETIC=<absolute fixture path>` — the same arming
/// pattern as `V2RAYN_R_AUTO_SMOKE`/`T18Bench`. Unarmed (official) builds
/// ignore every `V2RAYN_R_IMPORT_*` variable entirely.
///
/// On startup (before `runApp`) it runs the REAL import path with a synthetic
/// fixture — no network, no real subscription, no credentials:
/// - `.zip` input: `FrbBridgePort.t16BackupImportUpstream` (the same call
///   `BackupController.importUpstream` makes for an upstream gui-configs ZIP).
/// - text input: `previewImport` + `commitImport` (the same SP-14 seam
///   `subs_actions._importPipeline` uses: parse/preview, then one commit).
///
/// Then it writes `import-result.json` (`ok`, `imported`, `indexIds`,
/// `activeId`, `groupIds`) to `V2RAYN_R_IMPORT_OUT` and exits (0 ok / 1 fail).
/// The fixture text/credentials are never logged or written anywhere.
class ImportSynthHook {
  static String _env(String key) => Platform.environment[key] ?? '';

  static bool get enabled =>
      const bool.fromEnvironment('V2RAYN_R_SMOKE_ARMED', defaultValue: false) &&
      _env('V2RAYN_R_IMPORT_SYNTHETIC').isNotEmpty;

  static String get outDir {
    final value = _env('V2RAYN_R_IMPORT_OUT');
    return value.isEmpty ? 'benchmarks/import-synth' : value;
  }

  /// Run the import and exit the process. Never returns normally.
  static Future<void> runAndExit() async {
    final input = _env('V2RAYN_R_IMPORT_SYNTHETIC');
    try {
      final bridge = const FrbBridgePort();
      final int imported;
      final String sourceKind;
      if (input.toLowerCase().endsWith('.zip')) {
        sourceKind = 'zip';
        imported = await _runZipImport(bridge, input);
      } else {
        sourceKind = 'text';
        imported = await _runTextImport(bridge, input);
      }
      final profiles = bridge.queryAllProfiles();
      final activeId = bridge.getActiveProfile();
      final groups = bridge.listSubItems().items;
      _writeResult(<String, dynamic>{
        'ok': true,
        'imported': imported,
        'sourceKind': sourceKind,
        'indexIds': <String>[for (final p in profiles) p.indexId],
        'activeId': activeId,
        'groupIds': <String>[for (final g in groups) g.id],
      });
      stderr.writeln(
        '[import-synth] ok imported=$imported profiles=${profiles.length}',
      );
      exit(0);
    } catch (error) {
      _writeResult(<String, dynamic>{
        'ok': false,
        'imported': 0,
        'indexIds': const <String>[],
        'activeId': null,
        'groupIds': const <String>[],
        'error': _short('$error'),
      });
      stderr.writeln('[import-synth] failed: ${_short('$error')}');
      exit(1);
    }
  }

  /// Real ZIP path: the same bridge call as `BackupController.importUpstream`.
  static Future<int> _runZipImport(BridgePort bridge, String path) async {
    final file = File(path);
    if (!file.isAbsolute) {
      throw StateError('V2RAYN_R_IMPORT_SYNTHETIC must be an absolute path');
    }
    if (!await file.exists()) {
      throw StateError('synthetic ZIP not found');
    }
    final summary = await bridge.t16BackupImportUpstream(path);
    if (!summary.ok) {
      throw StateError(
        'import_upstream failed: ${summary.error?.code ?? summary.status}',
      );
    }
    return summary.importedRows.toInt();
  }

  /// Real text path: the same preview/commit seam as the subs import window.
  static Future<int> _runTextImport(BridgePort bridge, String path) async {
    final file = File(path);
    if (!file.isAbsolute) {
      throw StateError('V2RAYN_R_IMPORT_SYNTHETIC must be an absolute path');
    }
    if (!await file.exists()) {
      throw StateError('synthetic text fixture not found');
    }
    final text = await file.readAsString();
    if (text.trim().isEmpty) throw StateError('synthetic fixture is empty');
    final preview = await previewImport(bridge, text);
    if (!preview.ok) {
      throw StateError(
        'preview yielded no nodes: ${describeImportFailure(preview.result)}',
      );
    }
    final persisted = await commitImport(bridge, preview);
    if (persisted.saved == 0) {
      throw StateError(
        'commit saved 0 nodes (${persisted.firstErrorCode ?? 'unknown'})',
      );
    }
    return persisted.saved;
  }

  static void _writeResult(Map<String, dynamic> payload) {
    final directory = Directory(outDir)..createSync(recursive: true);
    final file = File(
      '${directory.path}${Platform.pathSeparator}import-result.json',
    );
    file.writeAsStringSync(const JsonEncoder.withIndent('  ').convert(payload));
  }

  static String _short(String text) =>
      text.length > 300 ? '${text.substring(0, 300)}…' : text;
}
