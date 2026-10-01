import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/dns.dart' as d;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/routing/dns_controller.dart';

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
    setState(() {
      _direct.text = simple?.directDns ?? '';
      _remote.text = simple?.remoteDns ?? '';
      _bootstrap.text = simple?.bootstrapDns ?? '';
      _useSystemHosts = simple?.useSystemHosts ?? false;
      _addCommonHosts = simple?.addCommonHosts ?? false;
      _fakeIp = simple?.fakeIp ?? false;
      _fakeIpRange.text = simple?.fakeIpRange ?? '';
      _blockBinding = simple?.blockBindingQuery ?? false;
      _blockAaaa = simple?.blockAaaaQuery ?? false;
      _parallelQuery = simple?.parallelQuery ?? false;
      _serveStale = simple?.serveStale ?? false;
      _happyEyeballs = simple?.enableHappyEyeballs ?? false;
      _strategyFreedom.text = simple?.strategy4Freedom ?? '';
      _strategyProxy.text = simple?.strategy4Proxy ?? '';
      _strategyProxyDial.text = simple?.strategy4ProxyDial ?? '';
      _hosts.text = simple?.hosts ?? '';
      _expectedIps.text = simple?.directExpectedIps ?? '';
      final xray = state.forCore(CoreType.xray);
      _xrayEnabled = xray?.enabled ?? false;
      _xrayUseSystemHosts = xray?.useSystemHosts ?? false;
      _xrayStrategy.text = xray?.domainStrategy4Freedom ?? '';
      _xrayDnsAddress.text = xray?.domainDnsAddress ?? '';
      _xrayNormal.text = xray?.normalDns ?? '';
      _xrayTun.text = xray?.tunDns ?? '';
      final sbox = state.forCore(CoreType.singBox);
      _sboxEnabled = sbox?.enabled ?? false;
      _sboxStrategy.text = sbox?.domainStrategy4Freedom ?? '';
      _sboxDnsAddress.text = sbox?.domainDnsAddress ?? '';
      _sboxNormal.text = sbox?.normalDns ?? '';
      _sboxTun.text = sbox?.tunDns ?? '';
      _preset = state.preset;
    });
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
      title: const Text('DNS 设置', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(12, 8, 12, 0),
      content: SizedBox(
        width: 720,
        height: 520,
        child: Column(
          children: <Widget>[
            TabBar(
              controller: _tabs,
              labelStyle: const TextStyle(fontSize: 13),
              tabs: const [
                Tab(text: '基础 DNS'),
                Tab(text: '高级 DNS'),
                Tab(text: '自定义 DNS (Xray)'),
                Tab(text: '自定义 DNS (sing-box)'),
              ],
            ),
            Expanded(
              child: TabBarView(
                controller: _tabs,
                children: [
                  _basicTab(),
                  _advancedTab(),
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
                const Text('区域预设', style: TextStyle(fontSize: 12)),
                const SizedBox(width: 8),
                DropdownButton<String>(
                  key: const ValueKey('dns-preset'),
                  value: _preset,
                  items: const [
                    DropdownMenuItem(value: 'Default', child: Text('默认区域')),
                    DropdownMenuItem(value: 'Russia', child: Text('俄罗斯')),
                    DropdownMenuItem(value: 'Iran', child: Text('伊朗')),
                  ],
                  onChanged: (v) => setState(() => _preset = v ?? 'Default'),
                ),
                const SizedBox(width: 8),
                TextButton(
                  key: const ValueKey('dns-apply-preset'),
                  onPressed: _applyPreset,
                  child: const Text('应用预设'),
                ),
                if (state.pendingUrls.isNotEmpty)
                  Expanded(
                    child: Text(
                      '${state.pendingUrls.length} 个远程模板待下载（未联网）',
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
                  state.status!,
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
          child: const Text('取消'),
        ),
        FilledButton(
          key: const ValueKey('dns-save'),
          onPressed: _save,
          child: const Text('保存'),
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
          _field('直连 DNS', _direct, const ValueKey('dns-direct')),
          _field('远端 DNS', _remote, const ValueKey('dns-remote')),
          _field('引导 DNS', _bootstrap, const ValueKey('dns-bootstrap')),
          _field('FakeIP 段', _fakeIpRange, const ValueKey('dns-fakeip-range')),
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

  void _importDefault(CoreType core) {
    ref.read(dnsControllerProvider.notifier).importDefault(core);
    _fillFromState();
  }

  void _applyPreset() {
    ref.read(dnsControllerProvider.notifier).applyPreset(_preset);
    _fillFromState();
  }

  void _save() {
    final controller = ref.read(dnsControllerProvider.notifier);
    final simple = d.SimpleDnsDto(
      useSystemHosts: _useSystemHosts,
      addCommonHosts: _addCommonHosts,
      fakeIp: _fakeIp,
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
    if (mounted) Navigator.pop(context);
  }
}
