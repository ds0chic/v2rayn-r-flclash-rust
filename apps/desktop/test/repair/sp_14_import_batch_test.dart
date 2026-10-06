// SP-14 All纯预览批导入正确合同（先红后修）。
//
// 唯一流程：用户在 All 预览导入后取消或提交失败，不产生文件或半批节点。
// - 预览零写入（DB/受控文件都不动）；提交整批原子（失败回滚，无半批）；
// - `previewToken` 绑定预览内容；`expectedRevision` + `mutationId` 幂等；
// - 取消不落盘。
//
// 合成数据专用；不触 10808/宿主网络/系统代理；不读用户秘密。
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';

c.ProfileDto vlessDto(String remarks, {String indexId = ''}) => c.ProfileDto(
  indexId: indexId,
  configType: ConfigType.vless,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: '',
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
  extraJson: '{}',
);

c.ImportResult okPreview(List<c.ProfileDto> profiles) => c.ImportResult(
  ok: profiles.isNotEmpty,
  imported: profiles.length,
  profiles: profiles,
  errors: const [],
);

/// SP-14 脚本桥：parse-only 预览 + 单事务提交 + 可注入失败。
///
/// 存储语义复用 [SyntheticBridgePort] 的单事务实现；这里只加计数与可控的
/// 预览内容。`saveImportedProfile` 计数保留，用于断言“新提交路径不再逐行写”。
class Sp14Bridge extends SyntheticBridgePort {
  Sp14Bridge() : super(count: 0);

  int previewCalls = 0;
  int saveImportedCalls = 0;

  c.ImportResult? stagedPreview;

  @override
  c.ImportResult previewImportText(String text, {String? subid}) {
    previewCalls++;
    if (stagedPreview != null) return stagedPreview!;
    return super.previewImportText(text, subid: subid);
  }

  @override
  c.SaveProfileResult saveImportedProfile(
    c.ProfileDto draft,
    int expectedRevision,
  ) {
    saveImportedCalls++;
    return super.saveImportedProfile(draft, expectedRevision);
  }
}

void main() {
  group('SP-14 preview zero-write', () {
    test('preview parses only: no commit, no per-row write', () async {
      final bridge = Sp14Bridge();
      final before = bridge.queryAllProfiles().length;
      final preview = await previewImport(
        bridge,
        'vless://a@192.0.2.1:443#one\nvless://b@192.0.2.2:443#two',
      );
      expect(preview.ok, isTrue);
      expect(preview.profiles.length, 2);
      expect(bridge.previewCalls, 1);
      expect(bridge.commitImportTextCalls, 0);
      expect(bridge.saveImportedCalls, 0);
      expect(bridge.queryAllProfiles().length, before);
    });

    test('cancel after preview persists nothing', () async {
      final bridge = Sp14Bridge();
      final before = bridge.queryAllProfiles().length;
      final preview = await previewImport(
        bridge,
        'vless://a@192.0.2.1:443#one',
      );
      expect(preview.ok, isTrue);
      // 用户取消：不调用 commitImport。
      expect(bridge.commitImportTextCalls, 0);
      expect(bridge.saveImportedCalls, 0);
      expect(bridge.queryAllProfiles().length, before);
    });
  });

  group('SP-14 single-transaction commit', () {
    test(
      'All/no-group commit is one batch write, no per-row fallback',
      () async {
        final bridge = Sp14Bridge();
        final preview = await previewImport(
          bridge,
          'vless://a@192.0.2.1:443#one\nvless://b@192.0.2.2:443#two',
        );
        final before = bridge.queryAllProfiles().length;
        final persisted = await commitImport(bridge, preview);
        expect(persisted.saved, 2);
        expect(persisted.failed, 0);
        expect(bridge.commitImportTextCalls, 1);
        expect(bridge.saveImportedCalls, 0, reason: 'All 提交必须走单事务，不再逐行写');
        expect(bridge.queryAllProfiles().length, before + 2);
      },
    );

    test('grouped commit binds the frozen group in one batch', () async {
      final bridge = Sp14Bridge();
      final preview = await previewImport(
        bridge,
        'vless://a@192.0.2.1:443#one',
      );
      final persisted = await commitImport(
        bridge,
        preview,
        subid: 'sub-frozen',
      );
      expect(persisted.saved, 1);
      expect(bridge.commitImportTextCalls, 1);
      expect(bridge.lastCommitSubid, 'sub-frozen');
      expect(bridge.saveImportedCalls, 0);
    });

    test('failed commit leaves no partial batch', () async {
      final bridge = Sp14Bridge();
      final preview = await previewImport(
        bridge,
        'vless://a@192.0.2.1:443#one\nvless://b@192.0.2.2:443#two',
      );
      final before = bridge.queryAllProfiles().length;
      bridge.failNextCommit = true;
      final persisted = await commitImport(bridge, preview);
      expect(persisted.saved, 0);
      expect(persisted.failed, 2);
      expect(persisted.firstErrorCode, isNotNull);
      expect(bridge.queryAllProfiles().length, before);
    });
  });

  group('SP-14 token / revision / mutation', () {
    test('preview token matches the FNV-1a vectors shared with Rust', () async {
      // Same algorithm as `application::import_batch::preview_token`, so both
      // layers bind identical tokens for identical text.
      final bridge = Sp14Bridge();
      final empty = await previewImport(bridge, '');
      expect(empty.previewToken, 'cbf29ce484222325');
      final a = await previewImport(bridge, 'a');
      expect(a.previewToken, 'af63dc4c8601ec8c');
    });

    test(
      'commit binds the preview token: mismatched content rejected',
      () async {
        final bridge = Sp14Bridge();
        final preview = await previewImport(
          bridge,
          'vless://a@192.0.2.1:443#one',
        );
        final before = bridge.queryAllProfiles().length;
        final persisted = await commitImport(
          bridge,
          preview,
          previewToken: 'tampered-token',
        );
        expect(persisted.saved, 0);
        expect(persisted.firstErrorCode, 'E_PREVIEW_MISMATCH');
        expect(bridge.commitImportTextCalls, 0);
        expect(bridge.queryAllProfiles().length, before);
      },
    );

    test('stale expectedRevision rejected without writing', () async {
      final bridge = Sp14Bridge();
      final preview = await previewImport(
        bridge,
        'vless://a@192.0.2.1:443#one',
      );
      final before = bridge.queryAllProfiles().length;
      // 先让库前进一次，使预览时冻结的 revision 变旧。
      bridge.saveImportedProfile(vlessDto('advance'), bridge.profileRevision());
      final persisted = await commitImport(
        bridge,
        preview,
        expectedRevision: preview.expectedRevision,
      );
      expect(persisted.saved, 0);
      expect(persisted.firstErrorCode, 'E_REVISION_STALE');
      expect(bridge.commitImportTextCalls, 0);
      expect(
        bridge.queryAllProfiles().length,
        before + 1,
        reason: '只有预先的 advance 行，不加半批',
      );
    });

    test(
      'same mutationId retried returns the same receipt without rewrite',
      () async {
        final bridge = Sp14Bridge();
        final preview = await previewImport(
          bridge,
          'vless://a@192.0.2.1:443#one',
        );
        final first = await commitImport(
          bridge,
          preview,
          mutationId: 'sp14-m-1',
        );
        expect(first.saved, 1);
        final afterFirst = bridge.queryAllProfiles().length;
        final commitsAfterFirst = bridge.commitImportTextCalls;
        final second = await commitImport(
          bridge,
          preview,
          mutationId: 'sp14-m-1',
        );
        expect(second.saved, first.saved);
        expect(second.firstErrorCode, first.firstErrorCode);
        expect(bridge.commitImportTextCalls, commitsAfterFirst);
        expect(bridge.queryAllProfiles().length, afterFirst);
      },
    );
  });
}
