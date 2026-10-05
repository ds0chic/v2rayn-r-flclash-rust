// R4-20 repro. Expected to FAIL pre-fix:
//  - A subscription-backed policy group preview (`resolveGroupPreview`) must use
//    the same child-validity rule as the persisted generation consumer
//    (`application::groups::resolve_sub_children` -> `domain::Profile::is_valid`).
//    Rust drops a Shadowsocks subscription child whose `ssMethod` is missing or
//    outside `Global.SsSecuritiesInSingbox`; the Dart preview currently keeps it,
//    so "预览与生成一致" is violated and the generated group would silently omit
//    a child the editor showed.
//
// Synthetic data only: no native library, kernel, network, user data or ports.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';

c.ProfileDto _node(
  String id,
  String remarks, {
  ConfigType type = ConfigType.vless,
  String address = '192.0.2.1',
  String password = '11111111-1111-1111-1111-111111111111',
  String? ssMethod,
}) => c.ProfileDto(
  indexId: id,
  configType: type,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: 'sub-1',
  isSub: true,
  displayLog: true,
  remarks: remarks,
  address: address,
  port: 443,
  password: password,
  username: '',
  network: 'raw',
  security: const c.SecurityDto(),
  protoExtra: c.ProtocolExtraDto(extraJson: '{}', ssMethod: ssMethod),
  transportExtra: const c.TransportExtraDto(extraJson: '{}'),
  extraJson: '{}',
);

void main() {
  test(
    'subscription preview drops a Shadowsocks child with an unsupported method',
    () {
      final all = <c.ProfileDto>[
        _node('ok', 'HK-vless'),
        _node(
          'ss-null',
          'HK-ss-null',
          type: ConfigType.shadowsocks,
          password: 'secret',
          ssMethod: null,
        ),
        _node(
          'ss-plain',
          'HK-ss-plain',
          type: ConfigType.shadowsocks,
          password: 'secret',
          ssMethod: 'plain',
        ),
        _node(
          'ss-aes',
          'HK-ss-aes',
          type: ConfigType.shadowsocks,
          password: 'secret',
          ssMethod: 'aes-256-gcm',
        ),
      ];
      final out = resolveGroupPreview(
        all: all,
        childIds: const <String>[],
        subChildItems: 'sub-1',
        filter: '^HK',
      );
      // Mirrors Rust `is_valid`: only a supported method keeps the SS child.
      expect(out.map((p) => p.indexId), <String>[
        'ok',
        'ss-aes',
      ], reason: 'preview must match the generation child-validity rule');
    },
  );
}
