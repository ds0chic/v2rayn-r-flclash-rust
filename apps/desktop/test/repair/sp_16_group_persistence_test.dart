// SP-16 接线轮：组创建/编辑/删除经真实控制器路径 + 重开持久化（合成数据专用）。
//
// 唯一流程：选 G 后退出重开仍选 G；编辑当前订阅直达 G；删除当前组回 All；
// 取消/失败不写库、不换组。不读用户秘密，不触网络/10808。
//
// 原版依据（冻结 7d6a967）：ProfilesViewModel.EditSubAsync（true=空白新建，
// false=GetSubItem(SubIndexId)，null 直接 return）；RefreshSubscriptions
//（SubIndexId 命中恢复，否则 All 首项，从不持久化修复值）；
// SubSelectedChangedAsync（_config.SubIndexId = SelectedSub?.Id）；
// DeleteSubAsync（删当前 G 后 RefreshSubscriptions + SubSelectedChangedAsync）。
// SubSettingViewModel.Add/Edit/Delete 作用自身选中行（无 G 预选），本轮经
// subs_controller.save/delete 统一刷新 + profiles 回落覆盖。
//
// 技术路径（已有桥接缝，未新增 FRB/Rust API）：
// 组切换 = saveSettingsGroup('SubIndexId', …)（engine 视显式 SubIndexId 组写
// 即组切换；整树保存会 pin 回该值，故不用 saveSettingsJson）；
// 重开恢复 = getSettings 读 SubIndexId 对现存订阅解析。
// 缺口登记：engine.set_current_group/resolve_current_group 无 FRB 暴露
// （SP-00 整合），本轮经 settings 组缝达到同等持久化效果；
// engine settings 组写不校验订阅存在性，存在性门控在 Dart 读侧
// （resync/build/edit 入口门控），见证据 README。
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/sub_entry.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';

/// Stored-projection synthetic bridge:节点表投射已存 profiles，组过滤可断言。
class Sp16StoredBridge extends SyntheticBridgePort {
  Sp16StoredBridge({super.count});

  @override
  ProfileSnapshot fetchProfileSnapshot(
    int count, {
    String? text,
    String? subid,
  }) {
    final stored = queryAllProfiles();
    if (stored.isEmpty) return super.fetchProfileSnapshot(count);
    return ProfileSnapshot(
      summaries: stored.map(dtoToSummary).toList(),
      profiles: stored,
    );
  }
}

/// settings 组写恒失败的桥（persist 失败路径）。
class FailingGroupBridge extends SyntheticBridgePort {
  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
    int expectedRevision,
  ) {
    return settings.SaveSettingsResult(
      ok: false,
      changes: const [],
      restartCoreFields: const [],
      restartAppFields: const [],
      nextLaunchFields: const [],
      error: const c.ErrorDto(
        code: 'E_REVISION_STALE',
        messageKey: 'error.revision_stale',
        retryable: false,
      ),
    );
  }
}

c.SubItemDto synSub(String remarks, {String id = ''}) => c.SubItemDto(
  id: id,
  remarks: remarks,
  url: 'https://example.com/${remarks.hashCode & 0xffff}',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 0,
  autoUpdateInterval: 0,
  updateTime: 0,
);

c.ProfileDto synNode(String remarks, {String subid = ''}) => c.ProfileDto(
  indexId: '',
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
  extraJson: '{}',
);

ProviderContainer sp16Container(BridgePort bridge) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
    profileRowCountProvider.overrideWithValue(50),
  ],
);

String? persistedSubIndexId(BridgePort bridge) {
  final load = bridge.getSettings();
  if (!load.ok || load.settingsJson.isEmpty) return null;
  return readPersistedSubIndexId(load.settingsJson);
}

void main() {
  group('SP-16 group persistence pure contract (synthetic)', () {
    test('subIndexIdPatch encodes group scalar, All as null', () {
      expect(subIndexIdPatch('g-syn-1'), '"g-syn-1"');
      expect(subIndexIdPatch(null), 'null');
      expect(subIndexIdPatch(''), 'null');
      expect(subIndexIdPatch('  '), 'null');
    });

    test('readPersistedSubIndexId normalizes blank/malformed to All', () {
      expect(readPersistedSubIndexId('{"SubIndexId":"g-syn-1"}'), 'g-syn-1');
      expect(readPersistedSubIndexId('{"SubIndexId":null}'), isNull);
      expect(readPersistedSubIndexId('{"SubIndexId":""}'), isNull);
      expect(readPersistedSubIndexId('{"SubIndexId":"  "}'), isNull);
      expect(readPersistedSubIndexId('{}'), isNull);
      expect(readPersistedSubIndexId('not-json'), isNull);
      expect(readPersistedSubIndexId('{"SubIndexId":42}'), isNull);
    });

    test('decodeGroupRevisions tolerates malformed input', () {
      expect(decodeGroupRevisions('{"SubIndexId":3}')['SubIndexId'], 3);
      expect(decodeGroupRevisions('{}'), isEmpty);
      expect(decodeGroupRevisions('not-json'), isEmpty);
      expect(decodeGroupRevisions('{"SubIndexId":"x"}'), isEmpty);
    });

    test('resolveReopenGroup hits live G, never first group', () {
      expect(
        resolveReopenGroup(
          persisted: 'g-syn-1',
          existingIds: const ['g-syn-1', 'g-syn-2'],
        ),
        'g-syn-1',
      );
      expect(
        resolveReopenGroup(persisted: 'g-gone', existingIds: const ['g-syn-1']),
        isNull,
      );
      expect(
        resolveReopenGroup(persisted: null, existingIds: const ['g-syn-1']),
        isNull,
      );
      expect(
        resolveReopenGroup(persisted: '  ', existingIds: const ['g-syn-1']),
        isNull,
      );
    });
  });

  group('SP-16 group switch persists canonical SubIndexId (synthetic)', () {
    test('switch writes SubIndexId, All writes null', () {
      final bridge = Sp16StoredBridge();
      final container = sp16Container(bridge);
      addTearDown(container.dispose);
      final profiles = container.read(profilesControllerProvider.notifier);

      final a = container
          .read(subsControllerProvider.notifier)
          .save(synSub('A'))
          .item!;
      expect(profiles.setGroupSubId(a.id), isTrue);
      expect(container.read(profilesControllerProvider).groupSubId, a.id);
      expect(persistedSubIndexId(bridge), a.id);

      expect(profiles.setGroupSubId(null), isTrue);
      expect(container.read(profilesControllerProvider).groupSubId, isNull);
      expect(persistedSubIndexId(bridge), isNull);
    });

    test('persist failure leaves the previous group untouched', () {
      final bridge = FailingGroupBridge();
      final container = sp16Container(bridge);
      addTearDown(container.dispose);
      final profiles = container.read(profilesControllerProvider.notifier);

      expect(profiles.setGroupSubId('g-syn-1'), isFalse);
      expect(container.read(profilesControllerProvider).groupSubId, isNull);
    });

    test('blank switch normalizes to All', () {
      final bridge = Sp16StoredBridge();
      final container = sp16Container(bridge);
      addTearDown(container.dispose);
      final profiles = container.read(profilesControllerProvider.notifier);

      expect(profiles.setGroupSubId('  '), isTrue);
      expect(container.read(profilesControllerProvider).groupSubId, isNull);
      expect(persistedSubIndexId(bridge), isNull);
    });
  });

  group('SP-16 reopen restores G (synthetic reopen)', () {
    test('reopen restores the persisted group view', () {
      final bridge = Sp16StoredBridge();
      final first = sp16Container(bridge);
      final a = first
          .read(subsControllerProvider.notifier)
          .save(synSub('A'))
          .item!;
      // A node stored under G so the restored view is observable.
      final node = first
          .read(profilesControllerProvider.notifier)
          .saveDraft(synNode('n-in-A', subid: a.id));
      expect(node.ok, isTrue);
      expect(
        first.read(profilesControllerProvider.notifier).setGroupSubId(a.id),
        isTrue,
      );
      expect(
        first.read(profilesControllerProvider).visible.map((r) => r.id),
        contains(node.profile!.indexId),
      );
      first.dispose();

      final second = sp16Container(bridge);
      addTearDown(second.dispose);
      expect(second.read(profilesControllerProvider).groupSubId, a.id);
      expect(
        second.read(profilesControllerProvider).visible.map((r) => r.id),
        contains(node.profile!.indexId),
      );
    });

    test('reopen with a deleted group falls back to All', () {
      final bridge = Sp16StoredBridge();
      final first = sp16Container(bridge);
      final a = first
          .read(subsControllerProvider.notifier)
          .save(synSub('A'))
          .item!;
      expect(
        first.read(profilesControllerProvider.notifier).setGroupSubId(a.id),
        isTrue,
      );
      first.dispose();

      // The group is deleted out-of-band; reopen must not invent a row.
      bridge.deleteSubItems([a.id]);
      final second = sp16Container(bridge);
      addTearDown(second.dispose);
      expect(second.read(profilesControllerProvider).groupSubId, isNull);
    });

    test('reopen never persists the dangling repair', () {
      final bridge = Sp16StoredBridge();
      bridge.saveSettingsGroup('SubIndexId', jsonEncode('g-gone'), 0);
      final container = sp16Container(bridge);
      addTearDown(container.dispose);
      expect(container.read(profilesControllerProvider).groupSubId, isNull);
      // The stored value is left for the next explicit switch (upstream
      // RefreshSubscriptions never persists a repair either).
      expect(persistedSubIndexId(bridge), 'g-gone');
    });
  });

  group('SP-16 create/edit/delete through the real path (synthetic)', () {
    test('edit keeps G and survives reopen', () {
      final bridge = Sp16StoredBridge();
      final first = sp16Container(bridge);
      final subs = first.read(subsControllerProvider.notifier);
      final profiles = first.read(profilesControllerProvider.notifier);
      final a = subs.save(synSub('A')).item!;
      expect(profiles.setGroupSubId(a.id), isTrue);

      final edited = c.SubItemDto(
        id: a.id,
        remarks: 'A-renamed',
        url: a.url,
        moreUrl: a.moreUrl,
        enabled: a.enabled,
        userAgent: a.userAgent,
        sort: a.sort,
        autoUpdateInterval: a.autoUpdateInterval,
        updateTime: a.updateTime,
      );
      final result = subs.save(edited);
      expect(result.ok, isTrue);
      expect(first.read(profilesControllerProvider).groupSubId, a.id);
      expect(
        first.read(subsControllerProvider).items.map((s) => s.remarks),
        contains('A-renamed'),
      );
      // Unified refresh: the profiles chips see the edited list.
      expect(profiles.subItems().map((s) => s.remarks), contains('A-renamed'));
      first.dispose();

      final second = sp16Container(bridge);
      addTearDown(second.dispose);
      expect(second.read(profilesControllerProvider).groupSubId, a.id);
    });

    test('create keeps the current G (no auto-switch to new)', () {
      final bridge = Sp16StoredBridge();
      final first = sp16Container(bridge);
      final subs = first.read(subsControllerProvider.notifier);
      final profiles = first.read(profilesControllerProvider.notifier);
      final a = subs.save(synSub('A')).item!;
      expect(profiles.setGroupSubId(a.id), isTrue);

      final created = subs.save(synSub('C')).item!;
      expect(created.id.isNotEmpty, isTrue);
      // Upstream keeps the SubIndexId hit after RefreshSubscriptions.
      expect(first.read(profilesControllerProvider).groupSubId, a.id);
      expect(profiles.subItems().map((s) => s.id), contains(created.id));
      first.dispose();

      final second = sp16Container(bridge);
      addTearDown(second.dispose);
      expect(second.read(profilesControllerProvider).groupSubId, a.id);
    });

    test(
      'delete of the current group falls back to All without repair write',
      () {
        final bridge = Sp16StoredBridge();
        final first = sp16Container(bridge);
        final subs = first.read(subsControllerProvider.notifier);
        final profiles = first.read(profilesControllerProvider.notifier);
        final a = subs.save(synSub('A')).item!;
        final b = subs.save(synSub('B')).item!;
        expect(profiles.setGroupSubId(a.id), isTrue);

        final deleted = subs.delete([a.id]);
        expect(deleted.ok, isTrue);
        expect(first.read(profilesControllerProvider).groupSubId, isNull);
        final chipIds = profiles.subItems().map((s) => s.id);
        expect(chipIds, contains(b.id));
        expect(chipIds, isNot(contains(a.id)));
        // No repair persisted: the stored id stays until an explicit switch.
        expect(persistedSubIndexId(bridge), a.id);
        first.dispose();

        final second = sp16Container(bridge);
        addTearDown(second.dispose);
        expect(second.read(profilesControllerProvider).groupSubId, isNull);
      },
    );

    test('delete of another group keeps the current G', () {
      final bridge = Sp16StoredBridge();
      final container = sp16Container(bridge);
      addTearDown(container.dispose);
      final subs = container.read(subsControllerProvider.notifier);
      final profiles = container.read(profilesControllerProvider.notifier);
      final a = subs.save(synSub('A')).item!;
      final b = subs.save(synSub('B')).item!;
      expect(profiles.setGroupSubId(a.id), isTrue);

      expect(subs.delete([b.id]).ok, isTrue);
      expect(container.read(profilesControllerProvider).groupSubId, a.id);
    });

    test('rejected save changes neither the list nor the group', () {
      final bridge = Sp16StoredBridge();
      final container = sp16Container(bridge);
      addTearDown(container.dispose);
      final subs = container.read(subsControllerProvider.notifier);
      final profiles = container.read(profilesControllerProvider.notifier);
      final a = subs.save(synSub('A')).item!;
      expect(profiles.setGroupSubId(a.id), isTrue);

      final before = container.read(subsControllerProvider).items.length;
      final rejected = subs.save(synSub(''));
      expect(rejected.ok, isFalse);
      expect(container.read(subsControllerProvider).items.length, before);
      expect(container.read(profilesControllerProvider).groupSubId, a.id);
    });
  });
}
