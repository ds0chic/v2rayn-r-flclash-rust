import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

/// Input widget kind for a form field.
///
/// [combo] is an editable dropdown: a free-text control plus a candidate list,
/// mirroring upstream's `IsEditable="True"` ComboBox (Fingerprint). A stored
/// value that is not in the candidate list is kept as-is instead of asserting.
enum FieldKind {
  text,
  intField,
  boolField,
  dropdown,
  combo,
  multiline,
  password,
}

/// One editable form field, bound to a [ProfileDraft].
class FieldSpec {
  const FieldSpec({
    required this.key,
    required this.label,
    required this.kind,
    required this.get,
    required this.set,
    this.options,
    this.required = false,
    this.hint,
  });

  final String key;
  final String label;
  final FieldKind kind;
  final String? Function(ProfileDraft) get;
  final void Function(ProfileDraft, String?) set;
  final List<FieldOption>? options;
  final bool required;
  final String? hint;
}

/// One dropdown option with a stable stored value and a display label.
class FieldOption {
  const FieldOption(this.value, this.label);

  final String? value;
  final String label;
}

class FormSection {
  const FormSection(this.title, this.fields);

  final String title;
  final List<FieldSpec> fields;
}

/// Core/transport capability constraints (compat/features.yaml `config_types`).
class ProfileCapabilities {
  static bool isSingBoxOnly(ConfigType t) =>
      t == ConfigType.tuic || t == ConfigType.anytls || t == ConfigType.naive;

  static CoreType defaultCore(ConfigType t) =>
      isSingBoxOnly(t) ? CoreType.singBox : CoreType.xray;

  static List<CoreType> allowedCores(ConfigType t) => isSingBoxOnly(t)
      ? const <CoreType>[CoreType.singBox]
      : const <CoreType>[CoreType.xray, CoreType.singBox];

  static const _rawOnly = <String>['raw'];

  static List<String> allowedNetworks(ConfigType t) {
    switch (t) {
      case ConfigType.vmess:
        return const <String>[
          'raw',
          'kcp',
          'ws',
          'httpupgrade',
          'xhttp',
          'grpc',
        ];
      case ConfigType.vless:
        return const <String>[
          'raw',
          'kcp',
          'ws',
          'httpupgrade',
          'xhttp',
          'grpc',
        ];
      case ConfigType.trojan:
        return const <String>['raw', 'ws', 'httpupgrade', 'xhttp', 'grpc'];
      case ConfigType.shadowsocks:
        return const <String>['raw', 'ws'];
      default:
        return _rawOnly;
    }
  }

  static bool supportsTls(ConfigType t) =>
      t == ConfigType.vmess ||
      t == ConfigType.vless ||
      t == ConfigType.trojan ||
      t == ConfigType.shadowsocks ||
      t == ConfigType.socks ||
      t == ConfigType.http;

  static bool supportsReality(ConfigType t) =>
      t == ConfigType.vmess || t == ConfigType.vless || t == ConfigType.trojan;

  /// Upstream `AddServerWindow` `togmuxEnabled` bindings: VMess/Shadowsocks/
  /// VLESS/Trojan only.
  static bool supportsMux(ConfigType t) =>
      t == ConfigType.vmess ||
      t == ConfigType.vless ||
      t == ConfigType.trojan ||
      t == ConfigType.shadowsocks;

  /// Upstream `togUotEnabled3`/`togUotEnabled12`: Shadowsocks and Naive.
  static bool supportsUot(ConfigType t) =>
      t == ConfigType.shadowsocks || t == ConfigType.naive;

  /// Upstream `gridFinalmask` is collapsed for TUIC/Anytls/Naive only.
  static bool supportsFinalmask(ConfigType t) =>
      t != ConfigType.tuic && t != ConfigType.anytls && t != ConfigType.naive;
}

/// Shadowsocks methods supported by Xray (`Global.SsSecuritiesInXray`).
const _ssMethodsXray = <String>[
  'aes-256-gcm',
  'aes-128-gcm',
  'chacha20-poly1305',
  'chacha20-ietf-poly1305',
  'xchacha20-poly1305',
  'xchacha20-ietf-poly1305',
  'none',
  'plain',
  '2022-blake3-aes-128-gcm',
  '2022-blake3-aes-256-gcm',
  '2022-blake3-chacha20-poly1305',
];

/// Shadowsocks methods supported by sing-box (`Global.SsSecuritiesInSingbox`).
const _ssMethodsSingBox = <String>[
  'aes-256-gcm',
  'aes-192-gcm',
  'aes-128-gcm',
  'chacha20-ietf-poly1305',
  'xchacha20-ietf-poly1305',
  'none',
  '2022-blake3-aes-128-gcm',
  '2022-blake3-aes-256-gcm',
  '2022-blake3-chacha20-poly1305',
  'aes-128-ctr',
  'aes-192-ctr',
  'aes-256-ctr',
  'aes-128-cfb',
  'aes-192-cfb',
  'aes-256-cfb',
  'rc4-md5',
  'chacha20-ietf',
  'xchacha20',
];

const _vmessSecurities = <String>[
  'auto',
  'aes-128-gcm',
  'chacha20-poly1305',
  'none',
  'zero',
];

const _fingerprints = <String>[
  'chrome',
  'firefox',
  'safari',
  'ios',
  'android',
  'edge',
  '360',
  'qq',
  'random',
  'randomized',
  '',
];

const _headerTypes = <String>[
  'none',
  'http',
  'srtp',
  'utp',
  'wechat-video',
  'dtls',
  'wireguard',
];

/// Protocol-specific fields for one of the 11 basic protocol kinds.
///
/// [coreType] selects the Shadowsocks method list (Xray vs sing-box). A stored
/// method outside the selected list is preserved as an unknown candidate.
List<FieldSpec> protocolFields(ConfigType t, {CoreType? coreType}) {
  switch (t) {
    case ConfigType.vmess:
      return <FieldSpec>[
        _text(
          'password',
          '用户 ID (id)',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _text('alterId', 'AlterId', (d) => d.alterId, (d, v) => d.alterId = v),
        _drop(
          'vmessSecurity',
          '加密方式 (security)',
          (d) => d.vmessSecurity,
          (d, v) => d.vmessSecurity = v,
          _vmessSecurities,
        ),
        _bool(
          'muxEnabled',
          '启用 Mux (muxEnabled)',
          (d) => d.muxEnabled,
          (d, v) => d.muxEnabled = v,
        ),
      ];
    case ConfigType.vless:
      return <FieldSpec>[
        _text(
          'password',
          '用户 ID (id)',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _drop(
          'flow',
          '流控 Flow',
          (d) => d.flow,
          (d, v) => d.flow = v,
          const <String>['', 'xtls-rprx-vision', 'xtls-rprx-vision-udp443'],
        ),
        // Upstream `txtSecurity5` is a plain TextBox so real encryption
        // parameters (e.g. mlkem) can be typed, not only the two presets.
        _text(
          'vlessEncryption',
          '加密方式 (encryption)',
          (d) => d.vlessEncryption,
          (d, v) => d.vlessEncryption = v,
        ),
        _bool(
          'muxEnabled',
          '启用 Mux (muxEnabled)',
          (d) => d.muxEnabled,
          (d, v) => d.muxEnabled = v,
        ),
      ];
    case ConfigType.shadowsocks:
      final methods = coreType == CoreType.singBox
          ? _ssMethodsSingBox
          : _ssMethodsXray;
      return <FieldSpec>[
        _drop(
          'ssMethod',
          '加密方式 (encryption)',
          (d) => d.ssMethod,
          (d, v) => d.ssMethod = v,
          methods,
          required: true,
        ),
        _text(
          'password',
          '密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _bool('uot', 'UDP over TCP', (d) => d.uot, (d, v) => d.uot = v),
        _bool(
          'muxEnabled',
          '启用 Mux (muxEnabled)',
          (d) => d.muxEnabled,
          (d, v) => d.muxEnabled = v,
        ),
      ];
    case ConfigType.socks:
      return <FieldSpec>[
        _text(
          'username',
          '用户名',
          (d) => d.username,
          (d, v) => d.username = v ?? '',
        ),
        _text(
          'password',
          '密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
        ),
      ];
    case ConfigType.http:
      return <FieldSpec>[
        _text(
          'username',
          '用户名',
          (d) => d.username,
          (d, v) => d.username = v ?? '',
        ),
        _text(
          'password',
          '密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
        ),
        _multiline(
          'httpHeaders',
          '自定义 HTTP 头',
          (d) => d.httpHeaders,
          (d, v) => d.httpHeaders = v,
        ),
      ];
    case ConfigType.trojan:
      return <FieldSpec>[
        _text(
          'password',
          '密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _bool(
          'muxEnabled',
          '启用 Mux (muxEnabled)',
          (d) => d.muxEnabled,
          (d, v) => d.muxEnabled = v,
        ),
      ];
    case ConfigType.hysteria2:
      return <FieldSpec>[
        _text(
          'password',
          '认证密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _text(
          'salamanderPass',
          'Salamander 混淆密码',
          (d) => d.salamanderPass,
          (d, v) => d.salamanderPass = v,
        ),
        _text(
          'ports',
          '端口范围 (Ports)',
          (d) => d.ports,
          (d, v) => d.ports = v,
          hint: '如 443-8443',
        ),
        _int('upMbps', '上行 Mbps', (d) => d.upMbps, (d, v) => d.upMbps = v),
        _int(
          'downMbps',
          '下行 Mbps',
          (d) => d.downMbps,
          (d, v) => d.downMbps = v,
        ),
        _text(
          'hopInterval',
          '端口跳跃间隔',
          (d) => d.hopInterval,
          (d, v) => d.hopInterval = v,
          hint: '如 30s',
        ),
        _text(
          'hy2RealmUrl',
          'Realm URL',
          (d) => d.hy2RealmUrl,
          (d, v) => d.hy2RealmUrl = v,
        ),
        _text(
          'geckoMinPacketSize',
          'Gecko 最小包',
          (d) => d.geckoMinPacketSize,
          (d, v) => d.geckoMinPacketSize = v,
        ),
        _text(
          'geckoMaxPacketSize',
          'Gecko 最大包',
          (d) => d.geckoMaxPacketSize,
          (d, v) => d.geckoMaxPacketSize = v,
        ),
      ];
    case ConfigType.tuic:
      // Upstream `gridTuic`: Username (UUID) and Password are two separate
      // TextBoxes (txtId8 / txtSecurity8). TUIC always runs on sing-box.
      return <FieldSpec>[
        _text(
          'username',
          '用户 ID (id)',
          (d) => d.username,
          (d, v) => d.username = v ?? '',
          required: true,
        ),
        _text(
          'password',
          '密码 (password)',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _drop(
          'congestionControl',
          '拥塞控制算法',
          (d) => d.congestionControl,
          (d, v) => d.congestionControl = v,
          const <String>['cubic', 'new_reno', 'bbr'],
        ),
      ];
    case ConfigType.wireGuard:
      return <FieldSpec>[
        _text(
          'password',
          '私钥 (Private Key)',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _text(
          'wgPublicKey',
          '公钥 (Public Key)',
          (d) => d.wgPublicKey,
          (d, v) => d.wgPublicKey = v,
        ),
        _text(
          'wgPresharedKey',
          '预共享密钥',
          (d) => d.wgPresharedKey,
          (d, v) => d.wgPresharedKey = v,
        ),
        _text(
          'wgInterfaceAddress',
          '本机地址',
          (d) => d.wgInterfaceAddress,
          (d, v) => d.wgInterfaceAddress = v,
          required: true,
          hint: '如 10.0.0.2/32',
        ),
        _text(
          'wgReserved',
          'Reserved',
          (d) => d.wgReserved,
          (d, v) => d.wgReserved = v,
        ),
        _int('wgMtu', 'MTU', (d) => d.wgMtu, (d, v) => d.wgMtu = v),
        _text('wgDns', 'DNS', (d) => d.wgDns, (d, v) => d.wgDns = v),
      ];
    case ConfigType.anytls:
      return <FieldSpec>[
        _text(
          'password',
          '密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
      ];
    case ConfigType.naive:
      return <FieldSpec>[
        _text(
          'username',
          '用户名',
          (d) => d.username,
          (d, v) => d.username = v ?? '',
        ),
        _text(
          'password',
          '密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _int(
          'insecureConcurrency',
          '并发数',
          (d) => d.insecureConcurrency,
          (d, v) => d.insecureConcurrency = v,
        ),
        _bool(
          'naiveQuic',
          '启用 QUIC',
          (d) => d.naiveQuic,
          (d, v) => d.naiveQuic = v,
        ),
        _bool('uot', 'UDP over TCP', (d) => d.uot, (d, v) => d.uot = v),
      ];
    default:
      return const <FieldSpec>[];
  }
}

/// Transport fields for the selected network.
List<FieldSpec> transportFields(String network) {
  switch (network) {
    case 'raw':
      return <FieldSpec>[
        _drop(
          'rawHeaderType',
          '伪装类型',
          (d) => d.rawHeaderType,
          (d, v) => d.rawHeaderType = v,
          _headerTypes,
        ),
        _text('host', 'Host', (d) => d.host, (d, v) => d.host = v),
        _text('path', 'Path', (d) => d.path, (d, v) => d.path = v),
      ];
    case 'kcp':
      return <FieldSpec>[
        _drop(
          'kcpHeaderType',
          '伪装类型',
          (d) => d.kcpHeaderType,
          (d, v) => d.kcpHeaderType = v,
          _headerTypes,
        ),
        _text(
          'kcpSeed',
          '种子 (Seed)',
          (d) => d.kcpSeed,
          (d, v) => d.kcpSeed = v,
        ),
        _int('kcpMtu', 'MTU', (d) => d.kcpMtu, (d, v) => d.kcpMtu = v),
      ];
    case 'ws':
    case 'httpupgrade':
      return <FieldSpec>[
        _text('host', 'Host', (d) => d.host, (d, v) => d.host = v),
        _text('path', 'Path', (d) => d.path, (d, v) => d.path = v),
      ];
    case 'xhttp':
      return <FieldSpec>[
        _text('host', 'Host', (d) => d.host, (d, v) => d.host = v),
        _text('path', 'Path', (d) => d.path, (d, v) => d.path = v),
        _drop(
          'xhttpMode',
          '模式',
          (d) => d.xhttpMode,
          (d, v) => d.xhttpMode = v,
          const <String>['auto', 'packet-up', 'stream-up', 'stream-one'],
        ),
        _multiline(
          'xhttpExtra',
          'Extra',
          (d) => d.xhttpExtra,
          (d, v) => d.xhttpExtra = v,
        ),
      ];
    case 'grpc':
      return <FieldSpec>[
        _text(
          'grpcAuthority',
          'Authority',
          (d) => d.grpcAuthority,
          (d, v) => d.grpcAuthority = v,
        ),
        _text(
          'grpcServiceName',
          'Service Name',
          (d) => d.grpcServiceName,
          (d, v) => d.grpcServiceName = v,
        ),
        _drop(
          'grpcMode',
          '模式',
          (d) => d.grpcMode,
          (d, v) => d.grpcMode = v,
          const <String>['gun', 'multi'],
        ),
      ];
    default:
      return const <FieldSpec>[];
  }
}

/// TLS/Reality fields. Reality-only fields are included only when selected.
List<FieldSpec> securityFields(
  String? streamSecurity, {
  bool finalmask = false,
}) {
  final isReality = streamSecurity == 'reality';
  return <FieldSpec>[
    _drop(
      'streamSecurity',
      '传输安全',
      (d) => d.streamSecurity,
      (d, v) => d.streamSecurity = v,
      const <String>['', 'tls', 'reality'],
    ),
    _drop(
      'allowInsecure',
      '跳过证书验证 (allowInsecure)',
      (d) => d.allowInsecure,
      (d, v) => d.allowInsecure = v,
      const <String>['', 'true', 'false'],
    ),
    _text('sni', 'SNI', (d) => d.sni, (d, v) => d.sni = v),
    _text(
      'alpn',
      'ALPN',
      (d) => d.alpn,
      (d, v) => d.alpn = v,
      hint: '逗号分隔，如 h2,http/1.1',
    ),
    _combo(
      'fingerprint',
      'Fingerprint',
      (d) => d.fingerprint,
      (d, v) => d.fingerprint = v,
      _fingerprints,
    ),
    if (isReality) ...<FieldSpec>[
      _text(
        'publicKey',
        'Reality 公钥',
        (d) => d.publicKey,
        (d, v) => d.publicKey = v,
        required: true,
      ),
      _text(
        'shortId',
        'Reality ShortId',
        (d) => d.shortId,
        (d, v) => d.shortId = v,
      ),
      _text('spiderX', 'SpiderX', (d) => d.spiderX, (d, v) => d.spiderX = v),
      _text(
        'mldsa65Verify',
        'ML-DSA-65 Verify',
        (d) => d.mldsa65Verify,
        (d, v) => d.mldsa65Verify = v,
      ),
    ],
    _text('cert', '证书 (Cert)', (d) => d.cert, (d, v) => d.cert = v),
    _text('certSha', '证书 SHA', (d) => d.certSha, (d, v) => d.certSha = v),
    _text(
      'echConfigList',
      'ECH Config List',
      (d) => d.echConfigList,
      (d, v) => d.echConfigList = v,
    ),
    _text(
      'verifyPeerCertByName',
      '按名验证证书',
      (d) => d.verifyPeerCertByName,
      (d, v) => d.verifyPeerCertByName = v,
    ),
    if (finalmask)
      _multiline(
        'finalmask',
        'Finalmask',
        (d) => d.finalmask,
        (d, v) => d.finalmask = v,
      ),
  ];
}

FieldSpec _text(
  String key,
  String label,
  String? Function(ProfileDraft) get,
  void Function(ProfileDraft, String?) set, {
  bool required = false,
  String? hint,
}) => FieldSpec(
  key: key,
  label: label,
  kind: FieldKind.text,
  get: get,
  set: set,
  required: required,
  hint: hint,
);

FieldSpec _multiline(
  String key,
  String label,
  String? Function(ProfileDraft) get,
  void Function(ProfileDraft, String?) set,
) => FieldSpec(
  key: key,
  label: label,
  kind: FieldKind.multiline,
  get: get,
  set: set,
);

FieldSpec _int(
  String key,
  String label,
  int? Function(ProfileDraft) get,
  void Function(ProfileDraft, int?) set,
) => FieldSpec(
  key: key,
  label: label,
  kind: FieldKind.intField,
  get: (d) => get(d)?.toString(),
  set: (d, v) => set(d, v == null || v.isEmpty ? null : int.tryParse(v)),
);

FieldSpec _bool(
  String key,
  String label,
  bool? Function(ProfileDraft) get,
  void Function(ProfileDraft, bool?) set,
) => FieldSpec(
  key: key,
  label: label,
  kind: FieldKind.boolField,
  get: (d) => switch (get(d)) {
    true => 'true',
    false => 'false',
    null => null,
  },
  set: (d, v) => set(d, v == null || v.isEmpty ? null : v == 'true'),
);

FieldSpec _drop(
  String key,
  String label,
  String? Function(ProfileDraft) get,
  void Function(ProfileDraft, String?) set,
  List<String> options, {
  bool required = false,
}) => FieldSpec(
  key: key,
  label: label,
  kind: FieldKind.dropdown,
  get: get,
  set: set,
  required: required,
  options: <FieldOption>[
    for (final o in options)
      FieldOption(o.isEmpty ? null : o, o.isEmpty ? '(无)' : o),
  ],
);

/// The write path of the UUID-bearing field for [t].
///
/// Upstream stores the TUIC UUID in `username` (txtId8) and the VMess/VLESS
/// UUID in `password`. Returns `null` when the protocol has no UUID concept.
void Function(ProfileDraft, String)? uuidSetter(ConfigType t) {
  switch (t) {
    case ConfigType.vmess:
    case ConfigType.vless:
      return (d, v) => d.password = v;
    case ConfigType.tuic:
      return (d, v) => d.username = v;
    default:
      return null;
  }
}

/// The field key that carries the UUID for [t], or `null`.
String? uuidFieldKey(ConfigType t) {
  switch (t) {
    case ConfigType.vmess:
    case ConfigType.vless:
      return 'password';
    case ConfigType.tuic:
      return 'username';
    default:
      return null;
  }
}

/// Generate a random v4 UUID (`Utils.GetGuid`).
String generateUuidV4([Random? random]) {
  final rng = random ?? Random.secure();
  final bytes = List<int>.generate(16, (_) => rng.nextInt(256));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  String hex(int b) => b.toRadixString(16).padLeft(2, '0');
  final s = bytes.map(hex).join();
  return '${s.substring(0, 8)}-${s.substring(8, 12)}-${s.substring(12, 16)}'
      '-${s.substring(16, 20)}-${s.substring(20)}';
}

/// Split a PEM bundle into individual `-----BEGIN CERTIFICATE-----` blocks
/// (`CertPemManager.ParsePemChain`).
List<String> parsePemChain(String pem) {
  final blocks = <String>[];
  const begin = '-----BEGIN CERTIFICATE-----';
  const end = '-----END CERTIFICATE-----';
  var cursor = 0;
  while (true) {
    final start = pem.indexOf(begin, cursor);
    if (start < 0) break;
    final stop = pem.indexOf(end, start);
    if (stop < 0) break;
    blocks.add(pem.substring(start, stop + end.length));
    cursor = stop + end.length;
  }
  return blocks;
}

/// Compute the upper-case SHA-256 thumbprint of one PEM certificate, mirroring
/// `CertPemManager.GetCertSha256Thumbprint` (hash of the DER body).
String? certSha256Thumbprint(String pemBlock) {
  final begin = pemBlock.indexOf('-----BEGIN');
  final endMarker = pemBlock.indexOf('-----END');
  if (begin < 0 || endMarker < 0 || endMarker <= begin) return null;
  final header = pemBlock.indexOf('-----', begin + 5);
  if (header < 0 || header >= endMarker) return null;
  final body = pemBlock
      .substring(header + 5, endMarker)
      .replaceAll(RegExp(r'\s'), '');
  if (body.isEmpty) return null;
  // A malformed body cannot be decoded; upstream returns empty and the caller
  // leaves the SHA untouched.
  final bytes = base64.decode(body);
  final digest = _sha256(bytes);
  return digest.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
}

/// Derive a comma-joined CertSha list from a full PEM chain. Returns `null`
/// when no certificate block parses (upstream leaves CertSha unchanged).
String? certShaFromChain(String pem) {
  final blocks = parsePemChain(pem);
  if (blocks.isEmpty) return null;
  final shas = <String>[];
  for (final block in blocks) {
    final sha = certSha256Thumbprint(block);
    if (sha == null) return null;
    shas.add(sha);
  }
  return shas.join(',');
}

/// Fetch the peer's leaf certificate as a PEM block (`CertPemManager.GetCertPemAsync`).
///
/// `dart:io` exposes only the leaf certificate; the chain variant below reuses
/// it and callers keep the leaf-only limitation. Never bypasses verification on
/// purpose for the host, but accepts the handshake so an expired/self-signed
/// pinning target can still be inspected (upstream behavior).
Future<String?> fetchPeerCertPem({
  required String host,
  required int port,
  String? serverName,
  Duration timeout = const Duration(seconds: 8),
}) async {
  SecureSocket? socket;
  try {
    socket = await SecureSocket.connect(
      host,
      port,
      onBadCertificate: (_) => true,
      timeout: timeout,
    );
    final der = socket.peerCertificate?.der;
    if (der == null) return null;
    return _derToPem(der);
  } finally {
    await socket?.close();
  }
}

/// Fetch the certificate chain as concatenated PEM. `dart:io` only returns the
/// leaf, so this matches [fetchPeerCertPem] on this platform.
Future<String?> fetchPeerCertChainPem({
  required String host,
  required int port,
  String? serverName,
  Duration timeout = const Duration(seconds: 8),
}) => fetchPeerCertPem(
  host: host,
  port: port,
  serverName: serverName,
  timeout: timeout,
);

String _derToPem(List<int> der) {
  const lineLength = 64;
  final base64Body = base64.encode(der);
  final lines = <String>[];
  for (var i = 0; i < base64Body.length; i += lineLength) {
    final end = (i + lineLength < base64Body.length)
        ? i + lineLength
        : base64Body.length;
    lines.add(base64Body.substring(i, end));
  }
  return '-----BEGIN CERTIFICATE-----\n'
      '${lines.join('\n')}\n'
      '-----END CERTIFICATE-----';
}

const _sha256K = <int>[
  0x428a2f98,
  0x71374491,
  0xb5c0fbcf,
  0xe9b5dba5,
  0x3956c25b,
  0x59f111f1,
  0x923f82a4,
  0xab1c5ed5,
  0xd807aa98,
  0x12835b01,
  0x243185be,
  0x550c7dc3,
  0x72be5d74,
  0x80deb1fe,
  0x9bdc06a7,
  0xc19bf174,
  0xe49b69c1,
  0xefbe4786,
  0x0fc19dc6,
  0x240ca1cc,
  0x2de92c6f,
  0x4a7484aa,
  0x5cb0a9dc,
  0x76f988da,
  0x983e5152,
  0xa831c66d,
  0xb00327c8,
  0xbf597fc7,
  0xc6e00bf3,
  0xd5a79147,
  0x06ca6351,
  0x14292967,
  0x27b70a85,
  0x2e1b2138,
  0x4d2c6dfc,
  0x53380d13,
  0x650a7354,
  0x766a0abb,
  0x81c2c92e,
  0x92722c85,
  0xa2bfe8a1,
  0xa81a664b,
  0xc24b8b70,
  0xc76c51a3,
  0xd192e819,
  0xd6990624,
  0xf40e3585,
  0x106aa070,
  0x19a4c116,
  0x1e376c08,
  0x2748774c,
  0x34b0bcb5,
  0x391c0cb3,
  0x4ed8aa4a,
  0x5b9cca4f,
  0x682e6ff3,
  0x748f82ee,
  0x78a5636f,
  0x84c87814,
  0x8cc70208,
  0x90befffa,
  0xa4506ceb,
  0xbef9a3f7,
  0xc67178f2,
];

List<int> _sha256(List<int> message) {
  var h0 = 0x6a09e667, h1 = 0xbb67ae85, h2 = 0x3c6ef372, h3 = 0xa54ff53a;
  var h4 = 0x510e527f, h5 = 0x9b05688c, h6 = 0x1f83d9ab, h7 = 0x5be0cd19;

  final bitLen = message.length * 8;
  final withOne = <int>[...message, 0x80];
  while (withOne.length % 64 != 56) {
    withOne.add(0);
  }
  for (var i = 7; i >= 0; i--) {
    withOne.add((bitLen >> (i * 8)) & 0xff);
  }

  final w = List<int>.filled(64, 0);
  for (var offset = 0; offset < withOne.length; offset += 64) {
    for (var i = 0; i < 16; i++) {
      final j = offset + i * 4;
      w[i] =
          (withOne[j] << 24) |
          (withOne[j + 1] << 16) |
          (withOne[j + 2] << 8) |
          withOne[j + 3];
    }
    for (var i = 16; i < 64; i++) {
      final s0 = _rotr(w[i - 15], 7) ^ _rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
      final s1 = _rotr(w[i - 2], 17) ^ _rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
      w[i] = (_u32(w[i - 16] + s0 + w[i - 7] + s1));
    }
    var a = h0, b = h1, c = h2, d = h3, e = h4, f = h5, g = h6, h = h7;
    for (var i = 0; i < 64; i++) {
      final s1 = _rotr(e, 6) ^ _rotr(e, 11) ^ _rotr(e, 25);
      final ch = (e & f) ^ (~e & g);
      final t1 = _u32(h + s1 + ch + _sha256K[i] + w[i]);
      final s0 = _rotr(a, 2) ^ _rotr(a, 13) ^ _rotr(a, 22);
      final maj = (a & b) ^ (a & c) ^ (b & c);
      final t2 = _u32(s0 + maj);
      h = g;
      g = f;
      f = e;
      e = _u32(d + t1);
      d = c;
      c = b;
      b = a;
      a = _u32(t1 + t2);
    }
    h0 = _u32(h0 + a);
    h1 = _u32(h1 + b);
    h2 = _u32(h2 + c);
    h3 = _u32(h3 + d);
    h4 = _u32(h4 + e);
    h5 = _u32(h5 + f);
    h6 = _u32(h6 + g);
    h7 = _u32(h7 + h);
  }

  final out = <int>[];
  for (final value in <int>[h0, h1, h2, h3, h4, h5, h6, h7]) {
    for (var i = 3; i >= 0; i--) {
      out.add((value >> (i * 8)) & 0xff);
    }
  }
  return out;
}

int _rotr(int x, int n) => ((x >> n) | (x << (32 - n))) & 0xffffffff;

int _u32(int x) => x & 0xffffffff;
FieldSpec _combo(
  String key,
  String label,
  String? Function(ProfileDraft) get,
  void Function(ProfileDraft, String?) set,
  List<String> options, {
  bool required = false,
  String? hint,
}) => FieldSpec(
  key: key,
  label: label,
  kind: FieldKind.combo,
  get: get,
  set: set,
  required: required,
  hint: hint,
  options: <FieldOption>[
    for (final o in options)
      FieldOption(o.isEmpty ? null : o, o.isEmpty ? '(无)' : o),
  ],
);
