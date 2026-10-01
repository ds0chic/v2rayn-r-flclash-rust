import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

/// Input widget kind for a form field.
enum FieldKind { text, intField, boolField, dropdown, multiline, password }

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
}

const _ssMethods = <String>[
  'aes-128-gcm',
  'aes-256-gcm',
  'chacha20-ietf-poly1305',
  '2022-blake3-aes-128-gcm',
  '2022-blake3-aes-256-gcm',
  '2022-blake3-chacha20-poly1305',
  'none',
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
List<FieldSpec> protocolFields(ConfigType t) {
  switch (t) {
    case ConfigType.vmess:
      return <FieldSpec>[
        _text(
          'password',
          '用户 ID (UUID)',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _text('alterId', 'AlterId', (d) => d.alterId, (d, v) => d.alterId = v),
        _drop(
          'vmessSecurity',
          '加密方式',
          (d) => d.vmessSecurity,
          (d, v) => d.vmessSecurity = v,
          _vmessSecurities,
        ),
      ];
    case ConfigType.vless:
      return <FieldSpec>[
        _text(
          'password',
          '用户 ID (UUID)',
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
        _drop(
          'vlessEncryption',
          '加密',
          (d) => d.vlessEncryption,
          (d, v) => d.vlessEncryption = v,
          const <String>['none', 'mlkem768x25519plus'],
        ),
      ];
    case ConfigType.shadowsocks:
      return <FieldSpec>[
        _drop(
          'ssMethod',
          '加密方式',
          (d) => d.ssMethod,
          (d, v) => d.ssMethod = v,
          _ssMethods,
          required: true,
        ),
        _text(
          'password',
          '密码',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
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
      return <FieldSpec>[
        _text(
          'password',
          'UUID',
          (d) => d.password,
          (d, v) => d.password = v ?? '',
          required: true,
        ),
        _drop(
          'congestionControl',
          '拥塞控制',
          (d) => d.congestionControl,
          (d, v) => d.congestionControl = v,
          const <String>['cubic', 'new_reno', 'bbr'],
        ),
        _bool('uot', 'UDP over TCP', (d) => d.uot, (d, v) => d.uot = v),
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
List<FieldSpec> securityFields(String? streamSecurity) {
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
      '允许不安全',
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
    _drop(
      'fingerprint',
      '指纹',
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
