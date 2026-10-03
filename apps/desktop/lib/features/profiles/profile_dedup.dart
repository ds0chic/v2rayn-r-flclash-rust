import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';

/// Faithful Dart port of the frozen upstream
/// `ConfigHandler.CompareProfileItem(o, n, remarks)` used by
/// `DedupServerList` (v2rayN 7.25.4 `7d6a967`).
///
/// Empty and absent optional strings compare equal (`eq_opt`), matching the
/// existing Rust `subscriptions::compare_profile`. Only the transport identity
/// fields participate; `remarks` is ignored for dedup.
bool compareProfileForDedup(
  c.ProfileDto o,
  c.ProfileDto n, {
  bool remarks = false,
}) {
  final oProto = o.protoExtra;
  final nProto = n.protoExtra;
  final oT = o.transportExtra;
  final nT = n.transportExtra;

  bool eq(String? a, String? b) => (a ?? '') == (b ?? '');

  return o.configType == n.configType &&
      eq(o.address, n.address) &&
      o.port == n.port &&
      eq(o.password, n.password) &&
      eq(o.username, n.username) &&
      eq(oProto.vlessEncryption, nProto.vlessEncryption) &&
      eq(oProto.ssMethod, nProto.ssMethod) &&
      eq(oProto.vmessSecurity, nProto.vmessSecurity) &&
      eq(o.network, n.network) &&
      eq(oT.rawHeaderType, nT.rawHeaderType) &&
      eq(oT.host, nT.host) &&
      eq(oT.path, nT.path) &&
      eq(oT.xhttpMode, nT.xhttpMode) &&
      eq(oT.xhttpExtra, nT.xhttpExtra) &&
      eq(oT.grpcAuthority, nT.grpcAuthority) &&
      eq(oT.grpcServiceName, nT.grpcServiceName) &&
      eq(oT.grpcMode, nT.grpcMode) &&
      eq(oT.kcpHeaderType, nT.kcpHeaderType) &&
      eq(oT.kcpSeed, nT.kcpSeed) &&
      (o.configType == ConfigType.trojan ||
          eq(o.security.streamSecurity, n.security.streamSecurity)) &&
      eq(oProto.flow, nProto.flow) &&
      eq(oProto.salamanderPass, nProto.salamanderPass) &&
      eq(o.security.sni, n.security.sni) &&
      eq(o.security.alpn, n.security.alpn) &&
      eq(o.security.fingerprint, n.security.fingerprint) &&
      eq(o.security.publicKey, n.security.publicKey) &&
      eq(o.security.shortId, n.security.shortId) &&
      eq(o.finalmask, n.finalmask) &&
      (!remarks || o.remarks == n.remarks);
}

/// Whether a profile is a complex node that dedup must always preserve
/// (upstream `ProfileItem.IsComplex()`).
bool isComplexProfile(ConfigType type) {
  switch (type) {
    case ConfigType.custom:
    case ConfigType.outbound:
    case ConfigType.policyGroup:
    case ConfigType.proxyChain:
      return true;
    default:
      return false;
  }
}

/// `ConfigHandler.DedupServerList`: return the ids that must be removed from
/// [items] to collapse duplicates.
///
/// When [keepOlder] is false the list is reversed first, so the *newer* entry
/// wins, exactly like upstream `!KeepOlderDedupl` handling. Complex nodes are
/// always kept. The input order is the caller's visible order (already sorted).
List<String> deduplicateProfiles(
  List<c.ProfileDto> items, {
  bool keepOlder = true,
}) {
  final ordered = keepOlder
      ? List<c.ProfileDto>.of(items)
      : List<c.ProfileDto>.of(items.reversed);
  final kept = <c.ProfileDto>[];
  final removedIds = <String>[];
  for (final item in ordered) {
    if (isComplexProfile(item.configType)) {
      kept.add(item);
      continue;
    }
    if (kept.any((existing) => compareProfileForDedup(existing, item))) {
      removedIds.add(item.indexId);
    } else {
      kept.add(item);
    }
  }
  return removedIds;
}
