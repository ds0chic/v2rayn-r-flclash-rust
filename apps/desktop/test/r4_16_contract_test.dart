// R4-16/SP-14 import/export contract tests: parse/preview vs commit
// separation, single commit (no Rust+Dart duplicate write), manual source
// rules (no dedup, IsSub handled by the Rust batch path), cancel/bad-line/
// failure behaviour and import -> export -> re-import interop at the pipeline
// seam.
//
// SP-14: preview runs the pure `previewImportText` entry point and every group
// (including the no-group bucket) commits through the single-transaction
// `commitImportText` entry point; the legacy `importFromText` batch path is
// only asserted for its own backwards-compatible behaviour.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';

c.ProfileDto vlessDto(
  String remarks, {
  String subid = '',
  String extraJson = '{}',
  String indexId = '',
}) => c.ProfileDto(
  indexId: indexId,
  configType: ConfigType.vless,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: subid,
  isSub: false,
  displayLog: true,
  remarks: remarks,
  address: '192.0.2.1',
  port: 443,
  password: '',
  username: '',
  network: 'raw',
  security: const c.SecurityDto(),
  protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
  transportExtra: const c.TransportExtraDto(extraJson: '{}'),
  extraJson: extraJson,
);

c.ImportResult okResult(
  List<c.ProfileDto> profiles, {
  List<c.ParseIssueDto> errors = const [],
}) => c.ImportResult(
  ok: profiles.isNotEmpty,
  imported: profiles.length,
  profiles: profiles,
  errors: errors,
);

/// Records the import seam and can inject controlled preview/commit outcomes.
class SpyImportBridge extends SyntheticBridgePort {
  SpyImportBridge({this.previewResult, this.commitResult});

  final c.ImportResult? previewResult;
  final c.ImportResult? commitResult;
  final List<String?> importSubids = <String?>[];
  final List<bool> importDedup = <bool>[];
  final List<String> exportedKinds = <String>[];
  int importPersistCalls = 0;
  int saveImportedCalls = 0;
  int previewCalls = 0;

  @override
  c.ImportResult previewImportText(String text, {String? subid}) {
    previewCalls++;
    if (previewResult != null) return previewResult!;
    return super.previewImportText(text, subid: subid);
  }

  @override
  c.ImportResult commitImportText(
    List<c.ProfileDto> profiles, {
    String? subid,
  }) {
    importPersistCalls++;
    if (commitResult != null) return commitResult!;
    return super.commitImportText(profiles, subid: subid);
  }

  @override
  Future<c.ImportResult> importFromText(
    String text, {
    String? subid,
    bool deduplicate = true,
  }) async {
    importSubids.add(subid);
    importDedup.add(deduplicate);
    return super.importFromText(text, subid: subid, deduplicate: deduplicate);
  }

  @override
  c.SaveProfileResult saveImportedProfile(
    c.ProfileDto draft,
    int expectedRevision,
  ) {
    saveImportedCalls++;
    return super.saveImportedProfile(draft, expectedRevision);
  }

  @override
  Future<c.ShareExportResult> exportProfiles(List<String> ids, String kind) {
    exportedKinds.add(kind);
    return super.exportProfiles(ids, kind);
  }
}

void main() {
  group('parse/preview', () {
    test('previewImport is parse-only through the pure entry point', () async {
      final bridge = SpyImportBridge(
        previewResult: okResult(<c.ProfileDto>[vlessDto('p1'), vlessDto('p1')]),
      );
      final preview = await previewImport(bridge, 'irrelevant');
      expect(preview.ok, isTrue);
      expect(preview.profiles.length, 2);
      expect(preview.imported, 2);
      expect(bridge.importPersistCalls, 0, reason: 'preview is parse-only');
      expect(bridge.saveImportedCalls, 0);
      expect(bridge.previewCalls, 1);
      // Manual batch import keeps duplicates: the staged preview carries both
      // rows instead of collapsing them (upstream guards `Distinct()` with
      // `if (isSub)`).
      expect(preview.profiles.map((p) => p.remarks), <String>['p1', 'p1']);
    });

    test('bad lines keep the valid rows and locate the failure', () async {
      final bridge = SpyImportBridge(
        previewResult: okResult(
          <c.ProfileDto>[vlessDto('good')],
          errors: const <c.ParseIssueDto>[
            c.ParseIssueDto(
              code: 'E_SUB_UNSUPPORTED',
              message: 'bad line',
              itemIndex: 1,
            ),
          ],
        ),
      );
      final preview = await previewImport(bridge, 'good\nbad');
      expect(preview.profiles.single.remarks, 'good');
      expect(preview.errors.single.itemIndex, 1);
      expect(describeImportFailure(preview.result), contains('第 2 项'));
    });
  });

  group('commit', () {
    test('group commit is one batch write and no per-row write', () async {
      final bridge = SpyImportBridge(
        previewResult: okResult(<c.ProfileDto>[vlessDto('a'), vlessDto('b')]),
        commitResult: okResult(<c.ProfileDto>[vlessDto('a'), vlessDto('b')]),
      );
      clearImportMutationCache();
      final preview = await previewImport(bridge, 'text');
      final persisted = await commitImport(bridge, preview, subid: 'sub-A');
      expect(persisted.saved, 2);
      expect(persisted.failed, 0);
      expect(bridge.importPersistCalls, 1, reason: 'exactly one batch commit');
      expect(bridge.saveImportedCalls, 0, reason: 'no Dart per-row re-save');
    });

    test('no-group commit is also one batch write (SP-14)', () async {
      final bridge = SpyImportBridge(
        previewResult: okResult(<c.ProfileDto>[vlessDto('a'), vlessDto('b')]),
      );
      clearImportMutationCache();
      final preview = await previewImport(bridge, 'text');
      final persisted = await commitImport(bridge, preview);
      expect(persisted.saved, 2);
      expect(bridge.importPersistCalls, 1);
      expect(
        bridge.saveImportedCalls,
        0,
        reason: 'All commits use the single transaction, not per-row saves',
      );
    });

    test('a failed batch commit is not half-written', () async {
      final bridge = SpyImportBridge(
        previewResult: okResult(<c.ProfileDto>[vlessDto('a')]),
        commitResult: const c.ImportResult(
          ok: false,
          imported: 0,
          profiles: <c.ProfileDto>[],
          errors: <c.ParseIssueDto>[],
          error: c.ErrorDto(
            code: 'E_UNAVAILABLE',
            messageKey: 'error.import_nothing',
            retryable: true,
          ),
        ),
      );
      clearImportMutationCache();
      final preview = await previewImport(bridge, 'text');
      final persisted = await commitImport(bridge, preview, subid: 'sub-A');
      expect(persisted.saved, 0);
      expect(persisted.failed, 1);
      expect(persisted.firstErrorCode, 'E_UNAVAILABLE');
      expect(bridge.saveImportedCalls, 0, reason: 'no partial per-row write');
    });

    test('cancel before commit persists nothing', () async {
      final bridge = SpyImportBridge(
        previewResult: okResult(<c.ProfileDto>[vlessDto('a')]),
      );
      await previewImport(bridge, 'text');
      // The user cancels: no commitImport call.
      expect(bridge.importPersistCalls, 0);
      expect(bridge.saveImportedCalls, 0);
    });
  });

  group('field round-trip and export interop', () {
    test('unknown extraJson survives the preview unchanged', () async {
      final dto = vlessDto(
        'labeled',
        extraJson: '{"x-future":[1,2],"Unknown":"kept"}',
      );
      final bridge = SpyImportBridge(
        previewResult: okResult(<c.ProfileDto>[dto]),
      );
      final preview = await previewImport(bridge, 'text');
      expect(preview.profiles.single.extraJson, dto.extraJson);
      expect(preview.profiles.single, dto);
    });

    test('imported nodes export to a share list that re-imports', () async {
      final bridge = SyntheticBridgePort(count: 1);
      final saved = bridge.saveImportedProfile(vlessDto('roundtrip'), 0);
      expect(saved.ok, isTrue);
      final id = saved.profile!.indexId;

      final exported = await bridge.exportProfiles(<String>[id], 'share');
      expect(exported.ok, isTrue);
      expect(exported.text, contains('://'));

      final preview = await previewImport(bridge, exported.text);
      expect(preview.ok, isTrue);
      expect(preview.imported, greaterThan(0));
    });

    test('a failed file export is a readable structured error', () {
      final bridge = SyntheticBridgePort(count: 1);
      final result = bridge.writeExportFile('C:/tmp/r4_16.txt', 'vless://x');
      expect(result.ok, isFalse);
      expect(result.error?.code, isNotNull);
    });
  });
}
