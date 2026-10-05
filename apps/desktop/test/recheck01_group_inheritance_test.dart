import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';

/// Synthetic bridge whose node table is projected from the stored profiles, so
/// a save/delete is visible in the summary list (the base synthetic bridge
/// regenerates a fixed window instead).
class StoredBridge extends SyntheticBridgePort {
  StoredBridge({super.count});

  @override
  List<ProfileSummary> fetchSummaries(int count) {
    final stored = queryAllProfiles();
    if (stored.isEmpty) return super.fetchSummaries(count);
    return stored.map(dtoToSummary).toList();
  }

  /// R4-09: the controller now reads the store through the paged snapshot
  /// seam; keep the same "stored profiles win over generated rows" contract
  /// for saved/imported nodes.
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

ProviderContainer makeStoredContainer({BridgePort? bridge}) =>
    ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge ?? StoredBridge()),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(20),
      ],
    );

c.ProfileDto vlessDto(String remarks, {String subid = ''}) => c.ProfileDto(
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

void main() {
  test('newDraft inherits the currently selected group', () {
    final container = makeStoredContainer();
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    controller.setGroupSubId('sub-A');
    expect(controller.newDraft(ConfigType.tuic).subid, 'sub-A');

    controller.setGroupSubId(null);
    expect(controller.newDraft(ConfigType.vmess).subid, '');
  });

  test('new TUIC saved under the current group lands in that group view', () {
    final container = makeStoredContainer();
    addTearDown(container.dispose);
    final controller = container.read(profilesControllerProvider.notifier);

    controller.setGroupSubId('sub-A');
    final draft = controller.newDraft(ConfigType.tuic)
      ..remarks = 'TUIC-in-A'
      ..address = '192.0.2.9';
    final result = controller.saveDraft(draft.toDto());
    expect(result.ok, isTrue);
    final saved = result.profile!;
    expect(saved.subid, 'sub-A');

    Iterable<String> visibleIds() =>
        container.read(profilesControllerProvider).visible.map((r) => r.id);

    expect(visibleIds(), contains(saved.indexId));
    controller.setGroupSubId('sub-B');
    expect(visibleIds(), isNot(contains(saved.indexId)));
    controller.setGroupSubId(null);
    expect(visibleIds(), contains(saved.indexId));
  });

  test('persistImportedProfiles rebinds an empty subid to the snapshot', () {
    final bridge = SyntheticBridgePort();
    final parsed = <c.ProfileDto>[
      vlessDto('p1'),
      vlessDto('p2', subid: 'existing'),
    ];
    final persisted = persistImportedProfiles(bridge, parsed, subid: 'sub-A');
    expect(persisted.saved, 2);

    final saved = bridge.queryAllProfiles();
    expect(saved.firstWhere((p) => p.remarks == 'p1').subid, 'sub-A');
    expect(saved.firstWhere((p) => p.remarks == 'p2').subid, 'existing');
  });

  test('persistImportedProfiles leaves no-group imports ungrouped', () {
    final bridge = SyntheticBridgePort();
    final persisted = persistImportedProfiles(bridge, <c.ProfileDto>[
      vlessDto('lonely'),
    ]);
    expect(persisted.saved, 1);
    final saved = bridge.queryAllProfiles().where((p) => p.remarks == 'lonely');
    expect(saved.single.subid, '');
  });
}
