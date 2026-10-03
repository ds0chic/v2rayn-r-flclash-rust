// FIX-10 / PR-12: pure unit tests for the Dart port of the frozen
// `ConfigHandler.CompareProfileItem` / `DedupServerList` used by the restored
// "移除重复" menu entry.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_dedup.dart';

c.ProfileDto _vless(String id, String address, {int port = 443}) =>
    c.ProfileDto(
      indexId: id,
      configType: ConfigType.vless,
      coreType: CoreType.xray,
      configVersion: 4,
      subid: 'sub-1',
      isSub: true,
      displayLog: true,
      remarks: id,
      address: address,
      port: port,
      password: 'shared-uuid',
      username: '',
      network: 'raw',
      security: const c.SecurityDto(streamSecurity: 'tls'),
      protoExtra: const c.ProtocolExtraDto(
        vlessEncryption: 'none',
        extraJson: '{}',
      ),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );

void main() {
  test('transport-identical nodes are duplicates, keep the older', () {
    final items = [
      _vless('old', '192.0.2.10'),
      _vless('new', '192.0.2.10'),
      _vless('distinct', '192.0.2.11'),
    ];
    expect(deduplicateProfiles(items), <String>['new']);
  });

  test('keepOlder=false keeps the newer entry', () {
    final items = [_vless('old', '192.0.2.10'), _vless('new', '192.0.2.10')];
    expect(deduplicateProfiles(items, keepOlder: false), <String>['old']);
  });

  test('a differing transport field is not a duplicate', () {
    final a = _vless('a', '192.0.2.10', port: 443);
    final b = _vless('b', '192.0.2.10', port: 8443);
    expect(deduplicateProfiles([a, b]), isEmpty);
  });

  test('complex nodes (policyGroup) are always kept', () {
    final group = c.ProfileDto(
      indexId: 'grp',
      configType: ConfigType.policyGroup,
      coreType: CoreType.xray,
      configVersion: 4,
      subid: 'sub-1',
      isSub: false,
      displayLog: true,
      remarks: 'group',
      address: '',
      port: 0,
      password: '',
      username: '',
      network: 'raw',
      security: const c.SecurityDto(),
      protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );
    // Two identical groups must both survive.
    expect(deduplicateProfiles([group, group]), isEmpty);
  });
}
