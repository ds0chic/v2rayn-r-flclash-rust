// R4-16 per-format matrix on the REAL bridge: synthetic fixture ->
// `importFromText` single commit -> read back from the SQLite data directory
// (reopen) -> export -> re-import, asserting field/unknown-key consistency for
// every upstream `compat/features.yaml` `fmt_formats` entry
// (FMT-001..FMT-017).
//
// The real `bridge_api.dll` is loaded like `t21e_import_real_bridge_test.dart`;
// when it is not built the test skips with the build instruction instead of
// faking a pass. The pure parse/emit half of the matrix lives in
// `crates/subscriptions/tests/r4_16_matrix.rs`.
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/api/subs.dart' as subs;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';

String? _findLibrary() {
  final candidates = <String>[
    '../../target/debug/bridge_api.dll',
    '../../target/release/bridge_api.dll',
    'bridge_api.dll',
  ];
  for (final path in candidates) {
    final file = File(path);
    if (file.existsSync()) return file.absolute.path;
  }
  final env = Platform.environment['V2RAYN_R_BRIDGE_DLL'];
  if (env != null && File(env).existsSync()) return env;
  return null;
}

/// Export seam for one row: `share` = share URI list, `inner` = `v2rayn://`,
/// `full` = full kernel config (Custom nodes), `none` = detection-only.
enum _Export { share, inner, full, none }

class _FmtCase {
  const _FmtCase(
    this.id,
    this.name,
    this.fixture,
    this.export,
    this.expectType, {
    this.expectCore,
    this.unknownKey,
  });

  final String id;
  final String name;
  final String fixture;
  final _Export export;
  final ConfigType? expectType;
  final CoreType? expectCore;
  final String? unknownKey;
}

String _b64UrlNoPad(String text) =>
    base64UrlEncode(utf8.encode(text)).replaceAll('=', '');

final List<_FmtCase> _cases = <_FmtCase>[
  const _FmtCase(
    'FMT-001',
    'VmessFmt',
    'vmess://22222222-2222-2222-2222-222222222222@vmess.example:443'
        '?type=ws&host=ws.example&path=/ws&security=tls#vmess-node',
    _Export.share,
    ConfigType.vmess,
  ),
  const _FmtCase(
    'FMT-002',
    'VLESSFmt',
    'vless://11111111-1111-1111-1111-111111111111@vless.example:8443'
        '?encryption=none&type=raw&security=reality&sni=s.example&pbk=KEY'
        '&sid=ab12&flow=xtls-rprx-vision#vless-node',
    _Export.share,
    ConfigType.vless,
  ),
  const _FmtCase(
    'FMT-003',
    'ShadowsocksFmt',
    'ss://YWVzLTI1Ni1nY206c2VjcmV0@ss.example:8388#ss-node',
    _Export.share,
    ConfigType.shadowsocks,
  ),
  const _FmtCase(
    'FMT-004',
    'SocksFmt',
    'socks://dXNlcjpwYXNzQDEyNy4wLjAuMToxMDgw#socks-node',
    _Export.share,
    ConfigType.socks,
  ),
  const _FmtCase(
    'FMT-005',
    'TrojanFmt',
    'trojan://tpass@trojan.example:443?security=tls&sni=trojan.example#trojan-node',
    _Export.share,
    ConfigType.trojan,
  ),
  const _FmtCase(
    'FMT-006',
    'Hysteria2Fmt',
    'hy2://hypass@hy2.example:8443?sni=hy2.example&obfs=salamander'
        '&obfs-password=obfs#hy2-node',
    _Export.share,
    ConfigType.hysteria2,
  ),
  const _FmtCase(
    'FMT-007',
    'TuicFmt',
    'tuic://tuser:tuicpass@tuic.example:8443?congestion_control=bbr#tuic-node',
    _Export.share,
    ConfigType.tuic,
  ),
  const _FmtCase(
    'FMT-008',
    'WireguardFmt',
    'wireguard://privkeypass@wireguard.example:51820?publickey=pubkey'
        '&address=10.0.0.2/32&mtu=1420#wg-node',
    _Export.share,
    ConfigType.wireGuard,
  ),
  const _FmtCase(
    'FMT-009',
    'AnytlsFmt',
    'anytls://anytlspass@anytls.example:8443?security=tls&alpn=h2,http/1.1'
        '#anytls-node',
    _Export.share,
    ConfigType.anytls,
  ),
  const _FmtCase(
    'FMT-010',
    'NaiveFmt',
    'naive+https://naive-user:naivepass@naive.example:443#naive-node',
    _Export.share,
    ConfigType.naive,
  ),
  _FmtCase(
    'FMT-011',
    'InnerFmt',
    'v2rayn://vless/${_b64UrlNoPad(jsonEncode(<String, Object?>{
      'IndexId': 'r416-inner',
      'ConfigType': 5,
      'ConfigVersion': 4,
      'Remarks': 'inner-node',
      'Address': 'node.example.invalid',
      'Port': 11984,
      'Password': '11111111-2222-3333-4444-555555555555',
      'Network': 'tcp',
      'ProtoExtraObj': <String, Object?>{'VlessEncryption': 'none'},
    }))}',
    _Export.inner,
    ConfigType.vless,
  ),
  const _FmtCase(
    'FMT-012',
    'V2rayFmt',
    '{"inbounds":[{"port":11888,"protocol":"socks"}],'
        '"outbounds":[{"protocol":"vmess","tag":"r416-v2ray",'
        '"settings":{"vnext":[]},"streamSettings":{"network":"tcp"}}],'
        '"x-future":[1,2,3]}',
    _Export.full,
    ConfigType.custom,
    expectCore: CoreType.xray,
    unknownKey: 'x-future',
  ),
  const _FmtCase(
    'FMT-013',
    'SingboxFmt',
    '{"inbounds":[],"outbounds":[{"type":"vless","tag":"r416-sbox",'
        '"server":"node.example.invalid","server_port":11981}],'
        '"x-future":"kept"}',
    _Export.full,
    ConfigType.custom,
    expectCore: CoreType.singBox,
    unknownKey: 'x-future',
  ),
  const _FmtCase(
    'FMT-014',
    'ClashFmt',
    'proxies:\n  - name: r416-clash\n    type: ss\n'
        '    server: node.example.invalid\n    port: 11982\n'
        'rules:\n  - MATCH,DIRECT\nmixed-port: 7890\nx-future: kept\n',
    _Export.full,
    ConfigType.custom,
    expectCore: CoreType.mihomo,
    unknownKey: 'x-future',
  ),
  const _FmtCase(
    'FMT-015',
    'HtmlPageFmt',
    '<!doctype html><html><head><title>x</title></head>'
        '<body>no nodes</body></html>',
    _Export.none,
    null,
  ),
  const _FmtCase('FMT-016', 'BaseFmt', '', _Export.none, null),
  const _FmtCase('FMT-017', 'FmtHandler', '', _Export.none, null),
];

Map<String, Object?> _primitives(c.ProfileDto p) => <String, Object?>{
  'configType': p.configType,
  'coreType': p.coreType,
  'remarks': p.remarks,
  'address': p.address,
  'port': p.port,
  'password': p.password,
  'username': p.username,
  'network': p.network,
  'isSub': p.isSub,
};

Map<String, Object?> _full(c.ProfileDto p) => <String, Object?>{
  ..._primitives(p),
  'configVersion': p.configVersion,
  'displayLog': p.displayLog,
  'preSocksPort': p.preSocksPort,
  'muxEnabled': p.muxEnabled,
  'finalmask': p.finalmask,
  'security': _securityFields(p.security),
  'protoExtra': _protoFields(p.protoExtra),
  'transportExtra': _transportFields(p.transportExtra),
  'extraJson': p.extraJson,
};

Map<String, Object?> _transportFields(c.TransportExtraDto t) =>
    <String, Object?>{
      'rawHeaderType': _norm(t.rawHeaderType),
      'host': _norm(t.host),
      'path': _norm(t.path),
      'xhttpMode': _norm(t.xhttpMode),
      'xhttpExtra': _norm(t.xhttpExtra),
      'grpcAuthority': _norm(t.grpcAuthority),
      'grpcServiceName': _norm(t.grpcServiceName),
      'grpcMode': _norm(t.grpcMode),
      'kcpHeaderType': _norm(t.kcpHeaderType),
      'kcpSeed': _norm(t.kcpSeed),
      'kcpMtu': t.kcpMtu,
      'extraJson': _norm(t.extraJson) ?? '{}',
    };

Map<String, Object?> _protoFields(c.ProtocolExtraDto p) => <String, Object?>{
  'uot': p.uot,
  'congestionControl': _norm(p.congestionControl),
  'httpHeaders': _norm(p.httpHeaders),
  // VMess `AlterId` defaults to 0; an omitted value and the explicit default
  // are the same node, so both normalize to null.
  'alterId': _norm(p.alterId) == '0' ? null : _norm(p.alterId),
  'vmessSecurity': _norm(p.vmessSecurity),
  'flow': _norm(p.flow),
  'vlessEncryption': _norm(p.vlessEncryption),
  'ssMethod': _norm(p.ssMethod),
  'wgPublicKey': _norm(p.wgPublicKey),
  'wgPresharedKey': _norm(p.wgPresharedKey),
  'wgInterfaceAddress': _norm(p.wgInterfaceAddress),
  'wgReserved': _norm(p.wgReserved),
  'wgMtu': p.wgMtu,
  'wgDns': _norm(p.wgDns),
  'salamanderPass': _norm(p.salamanderPass),
  'upMbps': p.upMbps,
  'downMbps': p.downMbps,
  'ports': _norm(p.ports),
  'hopInterval': _norm(p.hopInterval),
  'hy2RealmUrl': _norm(p.hy2RealmUrl),
  'geckoMinPacketSize': _norm(p.geckoMinPacketSize),
  'geckoMaxPacketSize': _norm(p.geckoMaxPacketSize),
  'insecureConcurrency': p.insecureConcurrency,
  'naiveQuic': p.naiveQuic,
  'groupType': _norm(p.groupType),
  'childItems': _norm(p.childItems),
  'subChildItems': _norm(p.subChildItems),
  'filter': _norm(p.filter),
  'multipleLoad': p.multipleLoad,
  'isSingboxEndpoint': p.isSingboxEndpoint,
  'extraJson': _norm(p.extraJson) ?? '{}',
};

/// Null and the empty string are the same "unset" for an optional share
/// parameter, and an explicit `security=none` token is the canonical spelling
/// of "no TLS" that a re-emit always writes; both normalize to null so the
/// import -> export -> re-import comparison reflects data, not spelling.
String? _norm(String? v) {
  if (v == null || v.isEmpty || v == 'none') return null;
  return v;
}

Map<String, Object?> _securityFields(c.SecurityDto s) => <String, Object?>{
  'streamSecurity': _norm(s.streamSecurity),
  'allowInsecure': _norm(s.allowInsecure),
  'sni': _norm(s.sni),
  'alpn': _norm(s.alpn),
  'fingerprint': _norm(s.fingerprint),
  'publicKey': _norm(s.publicKey),
  'shortId': _norm(s.shortId),
  'spiderX': _norm(s.spiderX),
  'mldsa65Verify': _norm(s.mldsa65Verify),
  'cert': _norm(s.cert),
  'certSha': _norm(s.certSha),
  'echConfigList': _norm(s.echConfigList),
  'verifyPeerCertByName': _norm(s.verifyPeerCertByName),
};

c.ProfilePageDto _pageFor(String subid) => engine.queryProfiles(
  filter: c.ProfileFilterDto(
    text: null,
    configTypes: const <ConfigType>[],
    subid: subid,
  ),
  sort: c.ProfileSortDto.indexId,
  cursor: BigInt.zero,
  pageSize: 10000,
);

Future<void> main() async {
  test(
    'R4-16 17-format import/reopen/export/reimport matrix (real bridge)',
    () async {
      final libraryPath = _findLibrary();
      if (libraryPath == null) {
        markTestSkipped(
          'bridge_api.dll not built; run `cargo build -p bridge_api`',
        );
        return;
      }
      await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
      final dir = Directory.systemTemp.createTempSync('r4_16_matrix_');
      addTearDown(() {
        try {
          dir.deleteSync(recursive: true);
        } catch (_) {}
      });
      expect(engine.initEngine(dataDir: dir.path).ok, isTrue);

      final rows = <String>[];
      final failures = <String>[];

      for (final cs in _cases) {
        try {
          final status = await _runCase(cs, dir);
          rows.add('${cs.id} ${cs.name}: $status');
        } catch (e) {
          rows.add('${cs.id} ${cs.name}: FAIL $e');
          failures.add('${cs.id} ${cs.name}: $e');
        }
      }

      // Contract guards on the real bridge (not the matrix rows).
      failures.addAll(await _contractGuards(dir));

      expect(
        failures,
        isEmpty,
        reason: <String>[
          'R4-16 matrix failures:',
          ...failures,
          '--- per-format rows ---',
          ...rows,
        ].join('\n'),
      );
    },
  );
}

Future<String> _runCase(_FmtCase cs, Directory dir) async {
  // Detection-only rows (BaseFmt base class / FmtHandler dispatch / HtmlPage)
  // have no persisted profile and therefore no round-trip.
  if (cs.export == _Export.none) {
    if (cs.id == 'FMT-015') {
      final html = await subs.importFromText(
        text: cs.fixture,
        deduplicate: false,
      );
      expect(html.ok, isFalse, reason: 'an HTML page must not import as nodes');
      expect(
        html.errors.isNotEmpty || html.error != null,
        isTrue,
        reason: 'the HTML page must surface a readable failure',
      );
      return 'verified (detection-only: readable failure, no persisted node)';
    }
    return 'not_applicable (base class / dispatch entry, covered transitively)';
  }

  final group = 'r4-16-${cs.id}';
  final importRes = await subs.importFromText(
    text: cs.fixture,
    subid: group,
    deduplicate: false,
  );
  expect(
    importRes.ok,
    isTrue,
    reason: '${cs.id} import: ${importRes.error?.code}',
  );
  expect(importRes.profiles.length, 1, reason: '${cs.id} one profile');
  final stage1 = importRes.profiles.single;
  if (cs.expectType != null) {
    expect(stage1.configType, cs.expectType, reason: '${cs.id} config type');
  }
  if (cs.expectCore != null) {
    expect(stage1.coreType, cs.expectCore, reason: '${cs.id} core type');
  }
  expect(
    stage1.isSub,
    isFalse,
    reason: '${cs.id} manual import is IsSub=false',
  );

  // Reopen: read the row back from the SQLite data directory.
  final reopened = _pageFor(group);
  expect(reopened.items.length, 1, reason: '${cs.id} persisted exactly once');
  expect(
    _primitives(reopened.items.single),
    _primitives(stage1),
    reason: '${cs.id} import -> reopen field set',
  );

  final row = reopened.items.single;
  expect(row.subid, group, reason: '${cs.id} reopened row keeps its group');

  // Unknown-key preservation for structured configs (materialized raw file).
  if (cs.unknownKey != null) {
    final file = File(
      '${dir.path}${Platform.pathSeparator}config'
      '${Platform.pathSeparator}${row.address}',
    );
    expect(
      file.existsSync(),
      isTrue,
      reason: '${cs.id} materialized config ${row.address}',
    );
    expect(file.readAsStringSync(), contains(cs.unknownKey!));
  }

  // Export then re-import.
  final String exported;
  switch (cs.export) {
    case _Export.share:
      final r = await subs.exportProfiles(
        ids: <String>[row.indexId],
        kind: 'share',
      );
      expect(r.ok, isTrue, reason: '${cs.id} share export: ${r.error?.code}');
      exported = r.text;
    case _Export.inner:
      final r = await subs.exportProfiles(
        ids: <String>[row.indexId],
        kind: 'inner',
      );
      expect(r.ok, isTrue, reason: '${cs.id} inner export: ${r.error?.code}');
      exported = r.text;
    case _Export.full:
      final r = const FrbBridgePort().exportClientConfigText(row.indexId);
      expect(
        r.ok,
        isTrue,
        reason: '${cs.id} full config export: ${r.error?.code}',
      );
      exported = r.text;
    case _Export.none:
      throw StateError('unreachable');
  }
  expect(exported.trim(), isNotEmpty, reason: '${cs.id} export is non-empty');
  if (cs.unknownKey != null) {
    expect(
      exported,
      contains(cs.unknownKey!),
      reason: '${cs.id} unknown key survives export',
    );
  }

  final stage2Group = 'r4-16-${cs.id}-reimport';
  final reimport = await subs.importFromText(
    text: exported,
    subid: stage2Group,
    deduplicate: false,
  );
  expect(
    reimport.ok,
    isTrue,
    reason: '${cs.id} re-import: ${reimport.error?.code}',
  );
  expect(reimport.profiles.length, 1, reason: '${cs.id} re-import one profile');
  final stage2 = reimport.profiles.single;
  expect(
    stage2.configType,
    stage1.configType,
    reason: '${cs.id} re-import field set',
  );
  expect(
    stage2.coreType,
    stage1.coreType,
    reason: '${cs.id} re-import core type',
  );
  expect(
    _full(stage2),
    _full(stage1),
    reason: '${cs.id} export -> re-import field set',
  );

  return 'verified (import=${stage1.configType.name}, reopen ok, '
      'export=${cs.export.name}, re-import consistent)';
}

/// Real-bridge contract guards for the R4-16 completion scenarios.
Future<List<String>> _contractGuards(Directory dir) async {
  final failures = <String>[];

  // parse-only preview never persists.
  const previewText =
      'vless://11111111-2222-3333-4444-555555555555@preview.example:443?encryption=none#preview';
  const previewGroup = 'r4-16-preview';
  final preview = subs.previewImportText(
    text: previewText,
    subid: previewGroup,
  );
  if (!preview.ok || preview.imported != 1) {
    failures.add('preview: expected 1 parsed profile, got ${preview.imported}');
  }
  if (_pageFor(previewGroup).items.isNotEmpty) {
    failures.add('preview: persisted a row (must be parse-only)');
  }

  // bad line keeps the good rows and locates the failure.
  const badText =
      'vless://11111111-2222-3333-4444-555555555555@good.example:443?encryption=none#good\n'
      'not-a-share-uri';
  final bad = await subs.importFromText(
    text: badText,
    subid: 'r4-16-badline',
    deduplicate: false,
  );
  if (!bad.ok || bad.imported != 1 || bad.errors.isEmpty) {
    failures.add(
      'bad-line: ok=${bad.ok} imported=${bad.imported} '
      'errors=${bad.errors.length} (expected 1 good + 1 located error)',
    );
  }
  if (_pageFor('r4-16-badline').items.length != 1) {
    failures.add('bad-line: the good row was not persisted');
  }

  // manual import keeps duplicates; explicit dedup collapses them.
  const dup =
      'vless://11111111-2222-3333-4444-555555555555@dup.example:443?encryption=none#dup\n'
      'vless://11111111-2222-3333-4444-555555555555@dup.example:443?encryption=none#dup';
  final kept = await subs.importFromText(
    text: dup,
    subid: 'r4-16-dup-keep',
    deduplicate: false,
  );
  if (kept.imported != 2) {
    failures.add('manual dedup=false: expected 2, got ${kept.imported}');
  }
  final collapsed = await subs.importFromText(
    text: dup,
    subid: 'r4-16-dup-drop',
    deduplicate: true,
  );
  if (collapsed.imported != 1) {
    failures.add('dedup=true: expected 1, got ${collapsed.imported}');
  }

  // an unwritable export file is a readable structured error.
  final unwritable = subs.writeExportFile(
    path:
        '${dir.path}${Platform.pathSeparator}missing'
        '${Platform.pathSeparator}sub${Platform.pathSeparator}out.txt',
    text: 'vless://x',
  );
  if (unwritable.ok || unwritable.error == null) {
    failures.add(
      'file export: unwritable path must fail with a structured error',
    );
  }

  return failures;
}
