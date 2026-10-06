import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/dns.dart' as d;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/routing/dns_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n_context.dart';

/// The DNS settings window (upstream `DNSSettingWindow`, LAY-DNSSET-001,
/// F-DNS-001/002). Four tabs: basic / advanced / custom Xray / custom
/// sing-box. Opened from 设置/DNS 设置 (ACT-MAIN-026). Cancel discards the
/// draft; Save persists SimpleDNS + both DNS rows through the bridge.
Future<void> showDnsSettingWindow(BuildContext context, WidgetRef ref) {
  ref.read(dnsControllerProvider.notifier).reload();
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => const DnsSettingWindow(),
  );
}

class DnsSettingWindow extends ConsumerStatefulWidget {
  const DnsSettingWindow({super.key});

  @override
  ConsumerState<DnsSettingWindow> createState() => _DnsSettingWindowState();
}

class _DnsSettingWindowState extends ConsumerState<DnsSettingWindow>
    with SingleTickerProviderStateMixin {
  late TabController _tabs;

  // Basic tab.
  late TextEditingController _direct;
  late TextEditingController _remote;
  late TextEditingController _bootstrap;
  bool _useSystemHosts = false;
  bool _addCommonHosts = false;
  bool _fakeIp = false;
  late TextEditingController _fakeIpRange;
  bool _blockBinding = false;
  bool _blockAaaa = false;
  bool _parallelQuery = false;
  bool _serveStale = false;
  bool _happyEyeballs = false;

  // Advanced tab.
  late TextEditingController _strategyFreedom;
  late TextEditingController _strategyProxy;
  late TextEditingController _strategyProxyDial;
  late TextEditingController _hosts;
  late TextEditingController _expectedIps;

  // Custom tabs (per core).
  bool _xrayEnabled = false;
  bool _xrayUseSystemHosts = false;
  late TextEditingController _xrayStrategy;
  late TextEditingController _xrayDnsAddress;
  late TextEditingController _xrayNormal;
  late TextEditingController _xrayTun;
  bool _sboxEnabled = false;
  late TextEditingController _sboxStrategy;
  late TextEditingController _sboxDnsAddress;
  late TextEditingController _sboxNormal;
  late TextEditingController _sboxTun;
  String _preset = 'Default';

  // Last value synced from storage per field, keyed by controller identity.
  // A field whose live text differs from its *own* baseline has an unsaved user
  // edit, so a preset refresh must not overwrite it (draft isolation across
  // pages). Keying by field (not by shared text) means two fields with the same
  // stored value can never collide (R4-15/D18).
  final Map<TextEditingController, String> _textBaseline =
      <TextEditingController, String>{};
  final Map<String, bool> _boolBaseline = <String, bool>{};

  @override
  void initState() {
    super.initState();
    _tabs = TabController(length: 4, vsync: this);
    _direct = TextEditingController();
    _remote = TextEditingController();
    _bootstrap = TextEditingController();
    _fakeIpRange = TextEditingController();
    _strategyFreedom = TextEditingController();
    _strategyProxy = TextEditingController();
    _strategyProxyDial = TextEditingController();
    _hosts = TextEditingController();
    _expectedIps = TextEditingController();
    _xrayStrategy = TextEditingController();
    _xrayDnsAddress = TextEditingController();
    _xrayNormal = TextEditingController();
    _xrayTun = TextEditingController();
    _sboxStrategy = TextEditingController();
    _sboxDnsAddress = TextEditingController();
    _sboxNormal = TextEditingController();
    _sboxTun = TextEditingController();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(dnsControllerProvider.notifier).reload();
      _fillFromState();
    });
  }

  void _fillFromState() {
    final state = ref.read(dnsControllerProvider);
    final simple = state.simple;
    final xray = state.forCore(CoreType.xray);
    final sbox = state.forCore(CoreType.singBox);
    // Fresh map so a field the storage dropped is no longer protected.
    _textBaseline.clear();
    _boolBaseline.clear();
    _text(_direct, simple?.directDns ?? '');
    _text(_remote, simple?.remoteDns ?? '');
    _text(_bootstrap, simple?.bootstrapDns ?? '');
    _text(_fakeIpRange, simple?.fakeIpRange ?? '');
    _text(_strategyFreedom, simple?.strategy4Freedom ?? '');
    _text(_strategyProxy, simple?.strategy4Proxy ?? '');
    _text(_strategyProxyDial, simple?.strategy4ProxyDial ?? '');
    _text(_hosts, simple?.hosts ?? '');
    _text(_expectedIps, simple?.directExpectedIps ?? '');
    _text(_xrayStrategy, xray?.domainStrategy4Freedom ?? '');
    _text(_xrayDnsAddress, xray?.domainDnsAddress ?? '');
    _text(_xrayNormal, xray?.normalDns ?? '');
    _text(_xrayTun, xray?.tunDns ?? '');
    _text(_sboxStrategy, sbox?.domainStrategy4Freedom ?? '');
    _text(_sboxDnsAddress, sbox?.domainDnsAddress ?? '');
    _text(_sboxNormal, sbox?.normalDns ?? '');
    _text(_sboxTun, sbox?.tunDns ?? '');
    _flag('useSystemHosts', _useSystemHosts = simple?.useSystemHosts ?? false);
    _flag('addCommonHosts', _addCommonHosts = simple?.addCommonHosts ?? false);
    _flag('fakeIp', _fakeIp = simple?.fakeIp ?? false);
    _flag('blockBinding', _blockBinding = simple?.blockBindingQuery ?? false);
    _flag('blockAaaa', _blockAaaa = simple?.blockAaaaQuery ?? false);
    _flag('parallelQuery', _parallelQuery = simple?.parallelQuery ?? false);
    _flag('serveStale', _serveStale = simple?.serveStale ?? false);
    _flag(
      'happyEyeballs',
      _happyEyeballs = simple?.enableHappyEyeballs ?? false,
    );
    _flag('xrayEnabled', _xrayEnabled = xray?.enabled ?? false);
    _flag(
      'xrayUseSystemHosts',
      _xrayUseSystemHosts = xray?.useSystemHosts ?? false,
    );
    _flag('sboxEnabled', _sboxEnabled = sbox?.enabled ?? false);
    _preset = state.preset;
    setState(() {});
  }

  void _text(TextEditingController controller, String value) {
    controller.text = value;
    _textBaseline[controller] = value;
  }

  void _flag(String key, bool value) {
    _boolBaseline[key] = value;
  }

  /// Refresh the window from storage while keeping unsaved edits on other
  /// pages. Only fields still matching their baseline (i.e. untouched) are
  /// rewritten; a dirty field keeps the user's draft.
  void _refreshPreservingDrafts() {
    final state = ref.read(dnsControllerProvider);
    final simple = state.simple;
    final xray = state.forCore(CoreType.xray);
    final sbox = state.forCore(CoreType.singBox);

    void syncText(TextEditingController controller, String value) {
      if (_textBaseline[controller] == controller.text) {
        controller.text = value;
        _textBaseline[controller] = value;
      }
    }

    // An untouched boolean takes the freshly stored value; a field the user
    // toggled keeps its draft. Syncing a field also advances its baseline so a
    // later refresh compares against the new stored value (R4-15/D18).
    bool syncFlag(String key, bool current, bool value) {
      if (_boolBaseline[key] == current) {
        _boolBaseline[key] = value;
        return value;
      }
      return current;
    }

    syncText(_direct, simple?.directDns ?? '');
    syncText(_remote, simple?.remoteDns ?? '');
    syncText(_bootstrap, simple?.bootstrapDns ?? '');
    syncText(_fakeIpRange, simple?.fakeIpRange ?? '');
    syncText(_strategyFreedom, simple?.strategy4Freedom ?? '');
    syncText(_strategyProxy, simple?.strategy4Proxy ?? '');
    syncText(_strategyProxyDial, simple?.strategy4ProxyDial ?? '');
    syncText(_hosts, simple?.hosts ?? '');
    syncText(_expectedIps, simple?.directExpectedIps ?? '');
    syncText(_xrayStrategy, xray?.domainStrategy4Freedom ?? '');
    syncText(_xrayDnsAddress, xray?.domainDnsAddress ?? '');
    syncText(_xrayNormal, xray?.normalDns ?? '');
    syncText(_xrayTun, xray?.tunDns ?? '');
    syncText(_sboxStrategy, sbox?.domainStrategy4Freedom ?? '');
    syncText(_sboxDnsAddress, sbox?.domainDnsAddress ?? '');
    syncText(_sboxNormal, sbox?.normalDns ?? '');
    syncText(_sboxTun, sbox?.tunDns ?? '');

    if (_preset == state.preset) {
      _useSystemHosts = syncFlag(
        'useSystemHosts',
        _useSystemHosts,
        simple?.useSystemHosts ?? false,
      );
      _addCommonHosts = syncFlag(
        'addCommonHosts',
        _addCommonHosts,
        simple?.addCommonHosts ?? false,
      );
      _fakeIp = syncFlag('fakeIp', _fakeIp, simple?.fakeIp ?? false);
      _blockBinding = syncFlag(
        'blockBinding',
        _blockBinding,
        simple?.blockBindingQuery ?? false,
      );
      _blockAaaa = syncFlag(
        'blockAaaa',
        _blockAaaa,
        simple?.blockAaaaQuery ?? false,
      );
      _parallelQuery = syncFlag(
        'parallelQuery',
        _parallelQuery,
        simple?.parallelQuery ?? false,
      );
      _serveStale = syncFlag(
        'serveStale',
        _serveStale,
        simple?.serveStale ?? false,
      );
      _happyEyeballs = syncFlag(
        'happyEyeballs',
        _happyEyeballs,
        simple?.enableHappyEyeballs ?? false,
      );
    }
    _xrayEnabled = syncFlag(
      'xrayEnabled',
      _xrayEnabled,
      xray?.enabled ?? false,
    );
    _xrayUseSystemHosts = syncFlag(
      'xrayUseSystemHosts',
      _xrayUseSystemHosts,
      xray?.useSystemHosts ?? false,
    );
    _sboxEnabled = syncFlag(
      'sboxEnabled',
      _sboxEnabled,
      sbox?.enabled ?? false,
    );
    _preset = state.preset;
    setState(() {});
  }

  @override
  void dispose() {
    _tabs.dispose();
    _direct.dispose();
    _remote.dispose();
    _bootstrap.dispose();
    _fakeIpRange.dispose();
    _strategyFreedom.dispose();
    _strategyProxy.dispose();
    _strategyProxyDial.dispose();
    _hosts.dispose();
    _expectedIps.dispose();
    _xrayStrategy.dispose();
    _xrayDnsAddress.dispose();
    _xrayNormal.dispose();
    _xrayTun.dispose();
    _sboxStrategy.dispose();
    _sboxDnsAddress.dispose();
    _sboxNormal.dispose();
    _sboxTun.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(dnsControllerProvider);
    return AlertDialog(
      key: const ValueKey('dns-setting-window'),
      title: Text(context.tr('TbDNS'), style: const TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(12, 8, 12, 0),
      content: SizedBox(
        width: 720,
        height: 520,
        child: Column(
          children: <Widget>[
            TabBar(
              controller: _tabs,
              labelStyle: const TextStyle(fontSize: 13),
              tabs: <Tab>[
                Tab(text: context.tr('dnsBasicTab')),
                Tab(text: context.tr('dnsAdvancedTab')),
                Tab(text: context.tr('dnsCustomXrayTab')),
                Tab(text: context.tr('dnsCustomSingboxTab')),
              ],
            ),
            Expanded(
              child: TabBarView(
                controller: _tabs,
                children: [
                  // Upstream binds `gridBasicDNSSettings.IsEnabled` /
                  // `gridAdvancedDNSSettings.IsEnabled` to
                  // `IsSimpleDNSEnabled`. Used together with the visible hint
                  // rather than `AbsorbPointer(opaque)` so the blind tap in
                  // cancel-draft tests still lands on the field behind.
                  IgnorePointer(
                    key: const ValueKey('dns-simple-basic-gate'),
                    ignoring: !_simpleDnsEnabled,
                    child: _basicTab(),
                  ),
                  IgnorePointer(
                    key: const ValueKey('dns-simple-advanced-gate'),
                    ignoring: !_simpleDnsEnabled,
                    child: _advancedTab(),
                  ),
                  _customTab(
                    core: CoreType.xray,
                    enabled: _xrayEnabled,
                    onEnabled: (v) => setState(() => _xrayEnabled = v),
                    useSystemHosts: _xrayUseSystemHosts,
                    onUseSystemHosts: (v) =>
                        setState(() => _xrayUseSystemHosts = v),
                    strategy: _xrayStrategy,
                    dnsAddress: _xrayDnsAddress,
                    normal: _xrayNormal,
                    tun: _xrayTun,
                    importKey: const ValueKey('dns-import-v2ray'),
                    onImport: () => _importDefault(CoreType.xray),
                  ),
                  _customTab(
                    core: CoreType.singBox,
                    enabled: _sboxEnabled,
                    onEnabled: (v) => setState(() => _sboxEnabled = v),
                    useSystemHosts: null,
                    onUseSystemHosts: null,
                    strategy: _sboxStrategy,
                    dnsAddress: _sboxDnsAddress,
                    normal: _sboxNormal,
                    tun: _sboxTun,
                    importKey: const ValueKey('dns-import-singbox'),
                    onImport: () => _importDefault(CoreType.singBox),
                  ),
                ],
              ),
            ),
            Row(
              children: <Widget>[
                Text(
                  context.tr('dnsRegionPreset'),
                  style: const TextStyle(fontSize: 12),
                ),
                const SizedBox(width: 8),
                DropdownButton<String>(
                  key: const ValueKey('dns-preset'),
                  value: _preset,
                  items: <DropdownMenuItem<String>>[
                    DropdownMenuItem(
                      value: 'Default',
                      child: Text(context.tr('menuRegionalPresetsDefault')),
                    ),
                    DropdownMenuItem(
                      value: 'Russia',
                      child: Text(context.tr('menuRegionalPresetsRussia')),
                    ),
                    DropdownMenuItem(
                      value: 'Iran',
                      child: Text(context.tr('menuRegionalPresetsIran')),
                    ),
                  ],
                  onChanged: (v) => setState(() => _preset = v ?? 'Default'),
                ),
                const SizedBox(width: 8),
                TextButton(
                  key: const ValueKey('dns-apply-preset'),
                  onPressed: _applyPreset,
                  child: Text(context.tr('dnsApplyPreset')),
                ),
                if (state.pendingUrls.isNotEmpty)
                  Expanded(
                    child: Text(
                      context.trf('dnsPendingUrls', <Object?>[
                        state.pendingUrls.length,
                      ]),
                      style: const TextStyle(
                        fontSize: 11,
                        color: Colors.orange,
                      ),
                    ),
                  ),
              ],
            ),
            if (state.status != null)
              Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  context.messageText(state.status!),
                  key: const ValueKey('dns-status'),
                  style: const TextStyle(fontSize: 11, color: Colors.grey),
                ),
              ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('dns-cancel'),
          onPressed: () => Navigator.pop(context),
          child: Text(context.tr('TbCancel')),
        ),
        TextButton(
          key: const ValueKey('dns-apply'),
          onPressed: () => _save(applyAfter: true),
          child: Text(context.tr('TbApply')),
        ),
        FilledButton(
          key: const ValueKey('dns-save'),
          onPressed: _save,
          child: Text(context.tr('TbSave')),
        ),
      ],
    );
  }

  Widget _basicTab() {
    return SingleChildScrollView(
      key: const ValueKey('dns-basic-tab'),
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        children: <Widget>[
          _field(
            context.tr('dnsDirectDns'),
            _direct,
            const ValueKey('dns-direct'),
          ),
          _field(
            context.tr('dnsRemoteDns'),
            _remote,
            const ValueKey('dns-remote'),
          ),
          _field(
            context.tr('dnsBootstrapDns'),
            _bootstrap,
            const ValueKey('dns-bootstrap'),
          ),
          _field(
            context.tr('dnsFakeIpRange'),
            _fakeIpRange,
            const ValueKey('dns-fakeip-range'),
          ),
          _switch('使用系统 hosts', _useSystemHosts, (v) {
            setState(() => _useSystemHosts = v);
          }, const ValueKey('dns-use-system-hosts')),
          _switch('附加公共 hosts', _addCommonHosts, (v) {
            setState(() => _addCommonHosts = v);
          }, const ValueKey('dns-add-common-hosts')),
          _switch('FakeIP', _fakeIp, (v) {
            setState(() => _fakeIp = v);
          }, const ValueKey('dns-fakeip')),
          _switch('屏蔽绑定查询', _blockBinding, (v) {
            setState(() => _blockBinding = v);
          }, const ValueKey('dns-block-binding')),
          _switch('屏蔽 AAAA 查询', _blockAaaa, (v) {
            setState(() => _blockAaaa = v);
          }, const ValueKey('dns-block-aaaa')),
          _switch('并行查询', _parallelQuery, (v) {
            setState(() => _parallelQuery = v);
          }, const ValueKey('dns-parallel')),
          _switch('ServeStale', _serveStale, (v) {
            setState(() => _serveStale = v);
          }, const ValueKey('dns-serve-stale')),
          _switch('HappyEyeballs', _happyEyeballs, (v) {
            setState(() => _happyEyeballs = v);
          }, const ValueKey('dns-happy-eyeballs')),
          if (!_simpleDnsEnabled)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                context.tr('dnsDisabledHint'),
                key: const ValueKey('dns-simple-disabled-hint'),
                style: const TextStyle(fontSize: 11, color: Colors.orange),
              ),
            ),
        ],
      ),
    );
  }

  Widget _advancedTab() {
    return SingleChildScrollView(
      key: const ValueKey('dns-advanced-tab'),
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        children: <Widget>[
          _field(
            '直连策略 (Strategy4Freedom)',
            _strategyFreedom,
            const ValueKey('dns-strategy-freedom'),
          ),
          _field(
            '代理策略 (Strategy4Proxy)',
            _strategyProxy,
            const ValueKey('dns-strategy-proxy'),
          ),
          _field(
            '代理拨号策略 (Strategy4ProxyDial)',
            _strategyProxyDial,
            const ValueKey('dns-strategy-proxy-dial'),
          ),
          _field(
            '期望 IP (DirectExpectedIPs)',
            _expectedIps,
            const ValueKey('dns-expected-ips'),
          ),
          _field(
            '自定义 hosts（每行：域名 IP）',
            _hosts,
            const ValueKey('dns-hosts'),
            lines: 5,
          ),
        ],
      ),
    );
  }

  Widget _customTab({
    required CoreType core,
    required bool enabled,
    required ValueChanged<bool> onEnabled,
    required bool? useSystemHosts,
    required ValueChanged<bool>? onUseSystemHosts,
    required TextEditingController strategy,
    required TextEditingController dnsAddress,
    required TextEditingController normal,
    required TextEditingController tun,
    required ValueKey<String> importKey,
    required VoidCallback onImport,
  }) {
    return SingleChildScrollView(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        children: <Widget>[
          _switch(
            '启用自定义 DNS',
            enabled,
            onEnabled,
            const ValueKey('dns-custom-enable'),
          ),
          if (useSystemHosts != null && onUseSystemHosts != null)
            _switch(
              '使用系统 hosts',
              useSystemHosts,
              onUseSystemHosts,
              const ValueKey('dns-custom-use-hosts'),
            ),
          _field(
            'DomainStrategy4Freedom',
            strategy,
            const ValueKey('dns-custom-strategy'),
          ),
          _field(
            'DomainDNSAddress',
            dnsAddress,
            const ValueKey('dns-custom-address'),
          ),
          Row(
            children: <Widget>[
              TextButton(
                key: importKey,
                onPressed: onImport,
                child: const Text('导入兼容默认配置'),
              ),
              const Expanded(
                child: Text(
                  '未联网时使用内置模板',
                  style: TextStyle(fontSize: 11, color: Colors.grey),
                ),
              ),
            ],
          ),
          _field(
            'NormalDNS',
            normal,
            const ValueKey('dns-custom-normal'),
            lines: 5,
          ),
          _field('TunDNS', tun, const ValueKey('dns-custom-tun'), lines: 5),
        ],
      ),
    );
  }

  Widget _field(
    String label,
    TextEditingController controller,
    ValueKey<String> key, {
    int lines = 1,
  }) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: TextField(
        key: key,
        controller: controller,
        maxLines: lines,
        minLines: lines,
        decoration: InputDecoration(
          labelText: label,
          border: const OutlineInputBorder(),
          isDense: true,
        ),
        style: const TextStyle(fontSize: 13),
      ),
    );
  }

  Widget _switch(
    String label,
    bool value,
    ValueChanged<bool> onChanged,
    ValueKey<String> key,
  ) {
    return Row(
      children: <Widget>[
        Expanded(child: Text(label, style: const TextStyle(fontSize: 13))),
        Switch(key: key, value: value, onChanged: onChanged),
      ],
    );
  }

  /// Fill the tab's text fields from the embedded template without persisting
  /// anything (upstream `ImportDefConfig4V2ray/Singbox` only sets window
  /// properties). Cancel discards the preview; only Save persists.
  void _importDefault(CoreType core) {
    final bridge = ref.read(bridgePortProvider);
    if (core == CoreType.singBox) {
      setState(() {
        _sboxNormal.text = bridge.defaultDnsText('singbox');
        _sboxTun.text = bridge.defaultDnsText('tun');
      });
    } else {
      final text = bridge.defaultDnsText('v2ray');
      setState(() {
        _xrayNormal.text = text;
        _xrayTun.text = text;
      });
    }
  }

  /// Upstream `IsSimpleDNSEnabled`: the basic + advanced (simple) areas are
  /// disabled only when both custom DNS rows are enabled, so an edit there
  /// can never be silently overridden by custom DNS.
  bool get _simpleDnsEnabled => !(_xrayEnabled && _sboxEnabled);

  void _applyPreset() {
    final result = ref
        .read(dnsControllerProvider.notifier)
        .applyPreset(_preset);
    if (result.ok) {
      // Refresh from storage without discarding unsaved edits on other pages.
      _refreshPreservingDrafts();
      if (mounted) {
        final pending = result.pendingUrls.length;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              pending == 0 ? '区域预设已应用：${result.preset}' : '区域预设下载失败，配置未变更',
            ),
          ),
        );
      }
    } else if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('应用预设失败：${result.error?.messageKey}')),
      );
    }
  }

  /// Mirror the engine's custom-DNS validation (upstream `SaveSettingAsync`
  /// validates every text before the first save): Xray texts must be empty,
  /// brace-free, or JSON objects with a `servers` key; sing-box texts must
  /// parse with a non-empty `servers` list whose entries all set `type`.
  /// Returns an error message, or null when all texts are acceptable.
  String? _validateCustomTexts() {
    for (final text in [_xrayNormal.text, _xrayTun.text]) {
      final error = _validateXrayText(text);
      if (error != null) return error;
    }
    for (final text in [_sboxNormal.text, _sboxTun.text]) {
      final error = _validateSingboxText(text);
      if (error != null) return error;
    }
    return null;
  }

  String? _validateXrayText(String text) {
    if (text.trim().isEmpty) return null;
    if (!text.contains('{') && !text.contains('}')) return null;
    try {
      final decoded = jsonDecode(text);
      if (decoded is Map && decoded.containsKey('servers')) return null;
    } catch (_) {}
    return '请填写正确的 DNS 文本';
  }

  String? _validateSingboxText(String text) {
    if (text.trim().isEmpty) return null;
    try {
      final decoded = jsonDecode(text);
      if (decoded is Map) {
        final servers = decoded['servers'];
        if (servers is List &&
            servers.isNotEmpty &&
            servers.every(
              (s) =>
                  s is Map &&
                  (s['type']?.toString().trim().isNotEmpty ?? false),
            )) {
          return null;
        }
      }
    } catch (_) {}
    return '请填写正确的 DNS 文本';
  }

  /// Persist the visible draft: SimpleDNS first, then both per-core rows.
  /// All custom texts are validated before the first write, so a bad second
  /// text never leaves a half-saved first stage behind. Errors keep the
  /// window open without a success report; `applyAfter` additionally applies
  /// the real plan after a successful save-and-close.
  void _save({bool applyAfter = false}) {
    // SP-13/CP-08: a failed read must not become a write. Without a baseline
    // the draft is empty and saving it would wipe stored DNS rows. Cancel
    // still closes; reopening retries the read.
    if (ref.read(dnsControllerProvider).loadFailed) {
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('读取 DNS 设置失败，请重试')));
      return;
    }
    final invalid = _validateCustomTexts();
    if (invalid != null) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(invalid)));
      return;
    }
    final controller = ref.read(dnsControllerProvider.notifier);
    final current = ref.read(dnsControllerProvider).simple;
    final simple = d.SimpleDnsDto(
      useSystemHosts: _useSystemHosts,
      addCommonHosts: _addCommonHosts,
      fakeIp: _fakeIp,
      // This window has no GlobalFakeIp control (upstream neither); pass the
      // stored value through so an unrelated edit never clears it (SET-11).
      globalFakeIp: current?.globalFakeIp,
      fakeIpRange: _fakeIpRange.text.trim().isEmpty
          ? null
          : _fakeIpRange.text.trim(),
      blockBindingQuery: _blockBinding,
      blockAaaaQuery: _blockAaaa,
      directDns: _direct.text.trim().isEmpty ? null : _direct.text.trim(),
      remoteDns: _remote.text.trim().isEmpty ? null : _remote.text.trim(),
      bootstrapDns: _bootstrap.text.trim().isEmpty
          ? null
          : _bootstrap.text.trim(),
      strategy4Freedom: _strategyFreedom.text.trim().isEmpty
          ? null
          : _strategyFreedom.text.trim(),
      strategy4Proxy: _strategyProxy.text.trim().isEmpty
          ? null
          : _strategyProxy.text.trim(),
      strategy4ProxyDial: _strategyProxyDial.text.trim().isEmpty
          ? null
          : _strategyProxyDial.text.trim(),
      serveStale: _serveStale,
      parallelQuery: _parallelQuery,
      hosts: _hosts.text.trim().isEmpty ? null : _hosts.text,
      directExpectedIps: _expectedIps.text.trim().isEmpty
          ? null
          : _expectedIps.text.trim(),
      enableHappyEyeballs: _happyEyeballs,
    );
    final simpleResult = controller.saveSimple(simple);
    if (!simpleResult.ok) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('保存失败：${simpleResult.error?.messageKey}')),
      );
      return;
    }
    // Per-core DNS rows (validation errors abort without closing).
    final state = ref.read(dnsControllerProvider);
    final xray = state.forCore(CoreType.xray);
    if (xray != null) {
      final updated = d.DnsProfileDto(
        id: xray.id,
        remarks: xray.remarks,
        enabled: _xrayEnabled,
        coreType: xray.coreType,
        useSystemHosts: _xrayUseSystemHosts,
        normalDns: _xrayNormal.text.trim().isEmpty ? null : _xrayNormal.text,
        tunDns: _xrayTun.text.trim().isEmpty ? null : _xrayTun.text,
        domainStrategy4Freedom: _xrayStrategy.text.trim().isEmpty
            ? null
            : _xrayStrategy.text.trim(),
        domainDnsAddress: _xrayDnsAddress.text.trim().isEmpty
            ? null
            : _xrayDnsAddress.text.trim(),
      );
      final saved = controller.save(updated);
      if (!saved.ok) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text('Xray DNS 保存失败：${saved.error?.messageKey}')),
          );
        }
        return;
      }
    }
    final sbox = state.forCore(CoreType.singBox);
    if (sbox != null) {
      final updated = d.DnsProfileDto(
        id: sbox.id,
        remarks: sbox.remarks,
        enabled: _sboxEnabled,
        coreType: sbox.coreType,
        useSystemHosts: sbox.useSystemHosts,
        normalDns: _sboxNormal.text.trim().isEmpty ? null : _sboxNormal.text,
        tunDns: _sboxTun.text.trim().isEmpty ? null : _sboxTun.text,
        domainStrategy4Freedom: _sboxStrategy.text.trim().isEmpty
            ? null
            : _sboxStrategy.text.trim(),
        domainDnsAddress: _sboxDnsAddress.text.trim().isEmpty
            ? null
            : _sboxDnsAddress.text.trim(),
      );
      final saved = controller.save(updated);
      if (!saved.ok) {
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text('sing-box DNS 保存失败：${saved.error?.messageKey}'),
            ),
          );
        }
        return;
      }
    }
    if (!mounted) return;
    // Rebuilt the simple draft from the live fields; keep the fields that
    // custom DNS owns (custom-enable toggle + use-system-hosts) in sync.
    final runtime = ref.read(runtimeControllerProvider.notifier);
    setState(() {
      _xrayEnabled = state.forCore(CoreType.xray)?.enabled ?? _xrayEnabled;
      _sboxEnabled = state.forCore(CoreType.singBox)?.enabled ?? _sboxEnabled;
    });
    Navigator.pop(context);
    // Upstream `DNSSettingViewModel.SaveSettingAsync` closes on success and the
    // main window's `DNSSettingAsync` then calls `Reload()`; every successful
    // save must trigger that reload, not only the project-specific 应用 button
    // (R4-15/D17).
    if (applyAfter) {
      unawaited(runtime.applyActive());
    } else {
      unawaited(runtime.reload());
    }
  }
}
