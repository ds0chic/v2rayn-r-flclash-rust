import 'package:v2rayn_desktop/bridge/api/contract.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

/// Mutable editor draft. Kept separate from the persisted `ProfileDto` so
/// cancel never touches stored data (plan §08: 表单草稿与已保存数据分离).
class ProfileDraft {
  ProfileDraft();

  String indexId = '';
  ConfigType configType = ConfigType.vmess;
  CoreType? coreType;
  int configVersion = 4;
  String subid = '';
  bool isSub = true;
  int? preSocksPort;
  bool displayLog = true;
  String remarks = '';
  String address = '';
  int port = 443;
  String password = '';
  String username = '';
  String network = 'raw';
  bool? muxEnabled;
  String? finalmask;

  // SecurityParams (13)
  String? streamSecurity;
  String? allowInsecure;
  String? sni;
  String? alpn;
  String? fingerprint;
  String? publicKey;
  String? shortId;
  String? spiderX;
  String? mldsa65Verify;
  String? cert;
  String? certSha;
  String? echConfigList;
  String? verifyPeerCertByName;

  // ProtocolExtra (30)
  bool? uot;
  String? congestionControl;
  String? httpHeaders;
  String? alterId;
  String? vmessSecurity;
  String? flow;
  String? vlessEncryption;
  String? ssMethod;
  String? wgPublicKey;
  String? wgPresharedKey;
  String? wgInterfaceAddress;
  String? wgReserved;
  int? wgMtu;
  String? wgDns;
  String? salamanderPass;
  int? upMbps;
  int? downMbps;
  String? ports;
  String? hopInterval;
  String? hy2RealmUrl;
  String? geckoMinPacketSize;
  String? geckoMaxPacketSize;
  int? insecureConcurrency;
  bool? naiveQuic;
  String? groupType;
  String? childItems;
  String? subChildItems;
  String? filter;
  int? multipleLoad;
  bool? isSingboxEndpoint;
  String protoExtraJson = '{}';

  // TransportExtra (11)
  String? rawHeaderType;
  String? host;
  String? path;
  String? xhttpMode;
  String? xhttpExtra;
  String? grpcAuthority;
  String? grpcServiceName;
  String? grpcMode;
  String? kcpHeaderType;
  String? kcpSeed;
  int? kcpMtu;
  String transportExtraJson = '{}';

  String extraJson = '{}';

  bool get isNew => indexId.trim().isEmpty;

  static ProfileDraft fromDto(ProfileDto dto) {
    final s = dto.security;
    final p = dto.protoExtra;
    final t = dto.transportExtra;
    return ProfileDraft()
      ..indexId = dto.indexId
      ..configType = dto.configType
      ..coreType = dto.coreType
      ..configVersion = dto.configVersion
      ..subid = dto.subid
      ..isSub = dto.isSub
      ..preSocksPort = dto.preSocksPort
      ..displayLog = dto.displayLog
      ..remarks = dto.remarks
      ..address = dto.address
      ..port = dto.port
      ..password = dto.password
      ..username = dto.username
      ..network = dto.network
      ..muxEnabled = dto.muxEnabled
      ..finalmask = dto.finalmask
      ..streamSecurity = s.streamSecurity
      ..allowInsecure = s.allowInsecure
      ..sni = s.sni
      ..alpn = s.alpn
      ..fingerprint = s.fingerprint
      ..publicKey = s.publicKey
      ..shortId = s.shortId
      ..spiderX = s.spiderX
      ..mldsa65Verify = s.mldsa65Verify
      ..cert = s.cert
      ..certSha = s.certSha
      ..echConfigList = s.echConfigList
      ..verifyPeerCertByName = s.verifyPeerCertByName
      ..uot = p.uot
      ..congestionControl = p.congestionControl
      ..httpHeaders = p.httpHeaders
      ..alterId = p.alterId
      ..vmessSecurity = p.vmessSecurity
      ..flow = p.flow
      ..vlessEncryption = p.vlessEncryption
      ..ssMethod = p.ssMethod
      ..wgPublicKey = p.wgPublicKey
      ..wgPresharedKey = p.wgPresharedKey
      ..wgInterfaceAddress = p.wgInterfaceAddress
      ..wgReserved = p.wgReserved
      ..wgMtu = p.wgMtu
      ..wgDns = p.wgDns
      ..salamanderPass = p.salamanderPass
      ..upMbps = p.upMbps
      ..downMbps = p.downMbps
      ..ports = p.ports
      ..hopInterval = p.hopInterval
      ..hy2RealmUrl = p.hy2RealmUrl
      ..geckoMinPacketSize = p.geckoMinPacketSize
      ..geckoMaxPacketSize = p.geckoMaxPacketSize
      ..insecureConcurrency = p.insecureConcurrency
      ..naiveQuic = p.naiveQuic
      ..groupType = p.groupType
      ..childItems = p.childItems
      ..subChildItems = p.subChildItems
      ..filter = p.filter
      ..multipleLoad = p.multipleLoad
      ..isSingboxEndpoint = p.isSingboxEndpoint
      ..protoExtraJson = p.extraJson
      ..rawHeaderType = t.rawHeaderType
      ..host = t.host
      ..path = t.path
      ..xhttpMode = t.xhttpMode
      ..xhttpExtra = t.xhttpExtra
      ..grpcAuthority = t.grpcAuthority
      ..grpcServiceName = t.grpcServiceName
      ..grpcMode = t.grpcMode
      ..kcpHeaderType = t.kcpHeaderType
      ..kcpSeed = t.kcpSeed
      ..kcpMtu = t.kcpMtu
      ..transportExtraJson = t.extraJson
      ..extraJson = dto.extraJson;
  }

  ProfileDto toDto() => ProfileDto(
    indexId: indexId,
    configType: configType,
    coreType: coreType,
    configVersion: configVersion == 0 ? 4 : configVersion,
    subid: subid,
    isSub: isSub,
    preSocksPort: preSocksPort,
    displayLog: displayLog,
    remarks: remarks,
    address: address,
    port: port,
    password: password,
    username: username,
    network: network,
    muxEnabled: muxEnabled,
    finalmask: finalmask,
    security: SecurityDto(
      streamSecurity: emptyToNull(streamSecurity),
      allowInsecure: emptyToNull(allowInsecure),
      sni: emptyToNull(sni),
      alpn: emptyToNull(alpn),
      fingerprint: emptyToNull(fingerprint),
      publicKey: emptyToNull(publicKey),
      shortId: emptyToNull(shortId),
      spiderX: emptyToNull(spiderX),
      mldsa65Verify: emptyToNull(mldsa65Verify),
      cert: emptyToNull(cert),
      certSha: emptyToNull(certSha),
      echConfigList: emptyToNull(echConfigList),
      verifyPeerCertByName: emptyToNull(verifyPeerCertByName),
    ),
    protoExtra: ProtocolExtraDto(
      uot: uot,
      congestionControl: emptyToNull(congestionControl),
      httpHeaders: emptyToNull(httpHeaders),
      alterId: emptyToNull(alterId),
      vmessSecurity: emptyToNull(vmessSecurity),
      flow: emptyToNull(flow),
      vlessEncryption: emptyToNull(vlessEncryption),
      ssMethod: emptyToNull(ssMethod),
      wgPublicKey: emptyToNull(wgPublicKey),
      wgPresharedKey: emptyToNull(wgPresharedKey),
      wgInterfaceAddress: emptyToNull(wgInterfaceAddress),
      wgReserved: emptyToNull(wgReserved),
      wgMtu: wgMtu,
      wgDns: emptyToNull(wgDns),
      salamanderPass: emptyToNull(salamanderPass),
      upMbps: upMbps,
      downMbps: downMbps,
      ports: emptyToNull(ports),
      hopInterval: emptyToNull(hopInterval),
      hy2RealmUrl: emptyToNull(hy2RealmUrl),
      geckoMinPacketSize: emptyToNull(geckoMinPacketSize),
      geckoMaxPacketSize: emptyToNull(geckoMaxPacketSize),
      insecureConcurrency: insecureConcurrency,
      naiveQuic: naiveQuic,
      groupType: emptyToNull(groupType),
      childItems: emptyToNull(childItems),
      subChildItems: emptyToNull(subChildItems),
      filter: emptyToNull(filter),
      multipleLoad: multipleLoad,
      isSingboxEndpoint: isSingboxEndpoint,
      extraJson: protoExtraJson.isEmpty ? '{}' : protoExtraJson,
    ),
    transportExtra: TransportExtraDto(
      rawHeaderType: emptyToNull(rawHeaderType),
      host: emptyToNull(host),
      path: emptyToNull(path),
      xhttpMode: emptyToNull(xhttpMode),
      xhttpExtra: emptyToNull(xhttpExtra),
      grpcAuthority: emptyToNull(grpcAuthority),
      grpcServiceName: emptyToNull(grpcServiceName),
      grpcMode: emptyToNull(grpcMode),
      kcpHeaderType: emptyToNull(kcpHeaderType),
      kcpSeed: emptyToNull(kcpSeed),
      kcpMtu: kcpMtu,
      extraJson: transportExtraJson.isEmpty ? '{}' : transportExtraJson,
    ),
    extraJson: extraJson.isEmpty ? '{}' : extraJson,
  );
}

String? emptyToNull(String? value) {
  if (value == null) return null;
  final trimmed = value.trim();
  return trimmed.isEmpty ? null : trimmed;
}
