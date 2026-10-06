import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/subs/import_persistence.dart';

c.ProfileDto profile(String id) => c.ProfileDto(indexId: id, configType: ConfigType.vless, coreType: CoreType.xray, configVersion: 4, subid: '', isSub: false, displayLog: true, remarks: 'Synthetic $id', address: '192.0.2.1', port: 443, password: '', username: '', network: 'raw', security: const c.SecurityDto(), protoExtra: const c.ProtocolExtraDto(extraJson: '{}'), transportExtra: const c.TransportExtraDto(extraJson: '{}'), extraJson: '{}');

class SecondWriteFailsBridge extends SyntheticBridgePort {
  SecondWriteFailsBridge() : super(count: 0);
  int writes = 0;

  @override
  c.SaveProfileResult saveImportedProfile(c.ProfileDto draft, int expectedRevision) {
    writes++;
    if (writes == 2) {
      return const c.SaveProfileResult(ok: false, error: c.ErrorDto(code: 'E_STORAGE', messageKey: 'error.storage', retryable: true));
    }
    return super.saveImportedProfile(draft, expectedRevision);
  }
}

void main() {
  test('ungrouped import uses one transaction and does not leave a partial batch', () async {
    final bridge = SecondWriteFailsBridge();
    final before = bridge.queryAllProfiles().length;
    final preview = ImportPreview(c.ImportResult(ok: true, imported: 2, profiles: <c.ProfileDto>[profile('a'), profile('b')], errors: const []));
    final result = await commitImport(bridge, 'synthetic preview already decoded', preview);
    final after = bridge.queryAllProfiles().length;
    print('AUDIT no-group import: saved=${result.saved}, failed=${result.failed}, per-row writes=${bridge.writes}, stored before=$before after=$after');
    expect(after, before, reason: 'an I/O failure in the batch must not leave just the first imported row; wire the generated commitImportText transaction');
  });
}
