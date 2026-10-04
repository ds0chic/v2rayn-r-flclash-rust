import 'dart:io';

import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

/// Client-facing proxy protocol of a loopback inbound, mirroring the Rust
/// `runtime::ProxyProtocol` scheme (do not invent an HTTP inbound where the
/// applied plan has a SOCKS one).
enum ProxyProtocolKind { http, socks, mixed }

/// A resolved local proxy endpoint: the applied port plus the protocol that
/// determines whether the system-proxy / PAC rule must speak SOCKS or HTTP.
class LocalProxyInbound {
  const LocalProxyInbound({required this.port, required this.protocol});

  final int port;
  final ProxyProtocolKind protocol;
}

/// The loopback proxy inbound the system proxy should point at, mirroring
/// upstream `AppManager.GetLocalPort(EInboundProtocol.socks)`: prefer the
/// persisted SOCKS inbound (`Protocol` 0/1/2), otherwise the first inbound.
/// Returns `null` when the document carries no usable inbound (the applied
/// session port is then used with the default HTTP scheme).
LocalProxyInbound? primaryLocalProxyInbound(Map<String, dynamic> document) {
  final list = document['Inbound'];
  if (list is! List) return null;
  LocalProxyInbound? fallback;
  for (final entry in list) {
    if (entry is! Map) continue;
    final port = (entry['LocalPort'] as num?)?.toInt();
    if (port == null || port <= 0) continue;
    final protocol = _proxyProtocolKind((entry['Protocol'] as num?)?.toInt());
    if (protocol == ProxyProtocolKind.socks) {
      return LocalProxyInbound(port: port, protocol: protocol);
    }
    fallback ??= LocalProxyInbound(port: port, protocol: protocol);
  }
  return fallback;
}

/// `EInboundProtocol` numeric -> protocol kind. Socks/Socks2/Socks3 (0/1/2)
/// are SOCKS; Mixed (6) accepts HTTP; PAC/API/speedtest are internal and are
/// never a client proxy (mapped to HTTP so they are not mistaken for SOCKS).
ProxyProtocolKind _proxyProtocolKind(int? value) {
  switch (value) {
    case 0:
    case 1:
    case 2:
      return ProxyProtocolKind.socks;
    case 6:
      return ProxyProtocolKind.mixed;
    default:
      return ProxyProtocolKind.http;
  }
}

/// Pure helpers that translate the persisted `SystemProxyItem` settings fields
/// into the arguments the platform bridge needs. Kept free of any I/O so it is
/// trivially unit-testable and so the wiring never guesses a value.
class ProxySettingsView {
  const ProxySettingsView({
    required this.mode,
    this.exceptions,
    this.notProxyLocalAddress = true,
    this.advancedProtocol,
    this.customPacPath,
    this.customScriptPath,
  });

  final SysProxyMode mode;
  final String? exceptions;
  final bool notProxyLocalAddress;
  final String? advancedProtocol;
  final String? customPacPath;
  final String? customScriptPath;

  /// Parse from the settings document (`SystemProxyItem` group).
  static ProxySettingsView fromDocument(Map<String, dynamic> document) {
    final item = document['SystemProxyItem'];
    final map = item is Map<String, dynamic> ? item : const <String, dynamic>{};
    return ProxySettingsView(
      mode: SysProxyMode.fromValue((map['SysProxyType'] as num?)?.toInt() ?? 2),
      exceptions: _nonEmpty(map['SystemProxyExceptions'] as String?),
      notProxyLocalAddress: map['NotProxyLocalAddress'] != false,
      advancedProtocol: _nonEmpty(
        map['SystemProxyAdvancedProtocol'] as String?,
      ),
      customPacPath: _nonEmpty(map['CustomSystemProxyPacPath'] as String?),
      customScriptPath: _nonEmpty(
        map['CustomSystemProxyScriptPath'] as String?,
      ),
    );
  }

  static String? _nonEmpty(String? value) =>
      (value == null || value.isEmpty) ? null : value;
}

/// Build the named-proxy server string for `ForcedChange`, mirroring upstream
/// `GetWindowsProxyString`: the advanced protocol template replaces
/// `{ip}`/`{http_port}`/`{socks_port}`, otherwise `<loopback>:<port>`.
String buildProxyServer({
  required int port,
  String? advancedProtocol,
  String loopback = '127.0.0.1',
}) {
  final template = advancedProtocol;
  if (template == null || template.isEmpty) {
    return '$loopback:$port';
  }
  return template
      .replaceAll('{ip}', loopback)
      .replaceAll('{http_port}', '$port')
      .replaceAll('{socks_port}', '$port');
}

/// Build the `ForcedChange` named-proxy server string, protocol-aware: the
/// advanced template (upstream `GetWindowsProxyString`) wins when set;
/// otherwise a SOCKS endpoint uses the WinINET `socks=` form so the client is
/// not told to speak HTTP to a SOCKS-only listener, and every other endpoint
/// keeps upstream's bare `<loopback>:<port>`.
String buildSystemProxyServer({
  required int port,
  required ProxyProtocolKind protocol,
  String? advancedProtocol,
  String loopback = '127.0.0.1',
}) {
  final template = advancedProtocol;
  if (template != null && template.isNotEmpty) {
    return buildProxyServer(
      port: port,
      advancedProtocol: template,
      loopback: loopback,
    );
  }
  if (protocol == ProxyProtocolKind.socks) {
    return 'socks=$loopback:$port';
  }
  return '$loopback:$port';
}

/// Build the PAC directive substituted for `__PROXY__`, mirroring upstream
/// `PacManager.cs:49` (`PROXY 127.0.0.1:{port};DIRECT;`) but honoring the
/// applied protocol: a SOCKS endpoint gets the `SOCKS5` directive instead of
/// pretending an HTTP proxy. This is deliberately separate from
/// [buildProxyServer]: a WinINET named-proxy template is not a valid PAC rule.
String buildPacProxyRule({
  required int port,
  ProxyProtocolKind protocol = ProxyProtocolKind.http,
  String loopback = '127.0.0.1',
}) {
  final directive = protocol == ProxyProtocolKind.socks ? 'SOCKS5' : 'PROXY';
  return '$directive $loopback:$port;DIRECT;';
}

/// Build the bypass/exception list, adding `<local>` when
/// `NotProxyLocalAddress` is enabled (upstream `GetWindowsProxyString`).
String buildProxyBypass({
  required String? exceptions,
  required bool notProxyLocalAddress,
}) {
  final compact = (exceptions ?? '')
      .split(';')
      .map((s) => s.replaceAll(' ', ''))
      .where((s) => s.isNotEmpty)
      .join(';');
  if (notProxyLocalAddress) {
    return compact.isEmpty ? '<local>' : '<local>;$compact';
  }
  return compact;
}

/// Build the PAC URL from a running PAC server handle.
String? pacUrlFromHandle({required String? url}) =>
    (url == null || url.isEmpty) ? null : url;

/// Return the `SystemProxyItem` map with `SysProxyType` set to [mode], keeping
/// every other field so a mode toggle persists without dropping the user's
/// exceptions / advanced protocol / custom PAC path (RT-14: a new mode must
/// survive a reopen instead of reverting).
Map<String, dynamic> systemProxyItemWithMode(
  Map<String, dynamic> document,
  SysProxyMode mode,
) {
  final item = document['SystemProxyItem'];
  final map = item is Map<String, dynamic>
      ? Map<String, dynamic>.from(item)
      : <String, dynamic>{};
  map['SysProxyType'] = mode.value;
  return map;
}

/// Bundled fallback PAC script. Only written when neither a custom PAC path nor
/// an existing `<configDir>/pac.txt` is present (upstream seeds the embedded
/// `pac` sample on first use; the Rust backend carries the full sample).
const String defaultPacScriptTemplate =
    'function FindProxyForURL(url, host) { return "__PROXY__"; }';

/// The PAC file the `Pac` mode should serve and whether it was user-configured.
class PacFileSelection {
  const PacFileSelection({required this.path, required this.isCustom});

  final String path;
  final bool isCustom;
}

/// Choose the PAC file to serve, mirroring upstream `PacManager.InitText`: the
/// configured custom path when set, otherwise `<configDir>/pac.txt`.
PacFileSelection selectPacFile({
  String? customPacPath,
  required String configDir,
}) {
  final custom = (customPacPath ?? '').trim();
  if (custom.isNotEmpty) {
    return PacFileSelection(path: custom, isCustom: true);
  }
  return PacFileSelection(
    path: _joinPath(configDir, 'pac.txt'),
    isCustom: false,
  );
}

String _joinPath(String dir, String name) {
  if (dir.isEmpty) return name;
  final last = dir[dir.length - 1];
  if (last == '/' || last == r'\') return '$dir$name';
  return '$dir${Platform.pathSeparator}$name';
}
