import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';

/// Option settings window (LAY-OPTSET-001/002).
///
/// Tab structure mirrors the frozen upstream `OptionSettingWindow.xaml`
/// (`TbSettingsCore` / `TbSettingsN` / `TbSettingsSystemproxy` /
/// `TbSettingsTunMode` / `TbSettingsCoreType`). Grouping and field placement
/// follow that file; the audit matrix is
/// `docs/evidence/UX-PARITY-FIX-16/field-matrix.md`.
///
/// Fields the frozen Window does not expose (KCP page is commented out
/// upstream; FakeIP/HappyEyeballs live in the DNS window; ClashUIItem has no
/// Window control) are kept in clearly labelled "历史保留" sections so the
/// underlying config stays editable without pretending upstream had a control.
/// Every control edits the canonical settings document; Save persists it
/// through the Rust engine. Platform-layer actions (kernel restart, system
/// proxy, TUN) are labelled 未接线 here and are never reported as having run.
class OptionSettingWindow extends ConsumerStatefulWidget {
  const OptionSettingWindow({super.key});

  static Future<void> show(BuildContext context) => showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => const OptionSettingWindow(),
  );

  @override
  ConsumerState<OptionSettingWindow> createState() =>
      _OptionSettingWindowState();
}

class _OptionSettingWindowState extends ConsumerState<OptionSettingWindow>
    with SingleTickerProviderStateMixin {
  late final TabController _tabs = TabController(length: 5, vsync: this);
  Map<String, dynamic> _draft = <String, dynamic>{};
  bool _draftInit = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    // Evidence runs can request a specific initial tab (screenshots).
    final requested = int.tryParse(
      Platform.environment['V2RAYN_R_SETTINGS_TAB'] ?? '',
    );
    if (requested != null && requested >= 0 && requested < _tabs.length) {
      _tabs.index = requested;
    }
    // Deferred so the provider is not mutated during the widget build; the
    // draft is derived in build() once the document is loaded.
    Future<void>.microtask(() {
      if (!mounted) return;
      ref.read(settingsControllerProvider.notifier).load();
      if (mounted) setState(() {});
    });
  }

  @override
  void dispose() {
    _tabs.dispose();
    super.dispose();
  }

  Map<String, dynamic> _group(String key) {
    final value = _draft[key];
    if (value is Map<String, dynamic>) return value;
    final created = <String, dynamic>{};
    _draft[key] = created;
    return created;
  }

  void _set(String group, String key, Object? value) {
    setState(() {
      if (group == 'Inbound') {
        _inboundListener()[key] = value;
      } else {
        _group(group)[key] = value;
      }
      _error = null;
    });
  }

  /// Persist `GuiItem.AutoRun` via the platform bridge and write/clear the Run
  /// key. Returns true only when the write succeeded.
  bool _applyAutostartWrite(bool enabled) {
    try {
      final bridge = ref.read(platformBridgeProvider);
      final exe = Platform.resolvedExecutable;
      final name = bridge.autostartValueName(exe);
      return bridge.setAutostart(
        name: name,
        enabled: enabled,
        exe: exe,
        args: '',
      );
    } on Object {
      return false;
    }
  }

  Map<String, dynamic> _inboundListener() {
    final value = _draft['Inbound'];
    if (value is List &&
        value.isNotEmpty &&
        value.first is Map<String, dynamic>) {
      return value.first as Map<String, dynamic>;
    }
    final created = <String, dynamic>{
      'LocalPort': 10808,
      'Protocol': 0,
      'UdpEnabled': true,
      'SniffingEnabled': true,
      'DestOverride': <dynamic>['http', 'tls'],
      'RouteOnly': false,
      'AllowLANConn': false,
      'NewPort4LAN': false,
      'User': '',
      'Pass': '',
      'SecondLocalPortEnabled': false,
    };
    _draft['Inbound'] = <dynamic>[created];
    return created;
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(settingsControllerProvider);
    // Seed the editable draft exactly once, from the persisted document, after
    // the first successful load. The old `_draft.isEmpty` guard failed whenever
    // a tab built (and created a group map) before load finished, which silently
    // replaced the stored values with defaults on reopen.
    if (!_draftInit && state.loaded) {
      _draft = ref.read(settingsControllerProvider.notifier).draft();
      _draftInit = true;
    }
    return AlertDialog(
      title: const Text('参数设置', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
      content: SizedBox(
        width: 720,
        height: 520,
        child: Column(
          children: <Widget>[
            TabBar(
              controller: _tabs,
              isScrollable: true,
              tabAlignment: TabAlignment.start,
              labelStyle: const TextStyle(fontSize: 12),
              tabs: const <Tab>[
                Tab(text: '核心基础'),
                Tab(text: '显示'),
                Tab(text: '系统代理'),
                Tab(text: 'Tun 模式'),
                Tab(text: '内核类型'),
              ],
            ),
            if (_error != null)
              Padding(
                padding: const EdgeInsets.all(4),
                child: Text(
                  _error!,
                  style: TextStyle(
                    fontSize: 11,
                    color: Theme.of(context).colorScheme.error,
                  ),
                ),
              ),
            if (state.status != null)
              Padding(
                padding: const EdgeInsets.all(4),
                child: Text(
                  state.status!,
                  style: const TextStyle(fontSize: 11),
                ),
              ),
            Expanded(
              child: TabBarView(
                controller: _tabs,
                children: <Widget>[
                  _coreTab(),
                  _displayTab(),
                  _systemProxyTab(),
                  _tunTab(),
                  _coreTypeTab(),
                ],
              ),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        // FIX-08: 应用 saves the visible draft first, then applies the real
        // plan. A save error keeps the window open with the error shown and
        // never reports success or applies a stale document.
        FilledButton.tonal(
          key: const ValueKey('settings-apply'),
          onPressed: () => _save(applyAfter: true),
          child: const Text('应用'),
        ),
        FilledButton(
          key: const ValueKey('settings-save'),
          onPressed: () => _save(),
          child: const Text('保存'),
        ),
      ],
    );
  }

  /// Upstream `OptionSettingViewModel.SaveSettingAsync` rejects a non-numeric
  /// or out-of-range local port and malformed fragment ranges before touching
  /// storage.
  String? _validateDraft() {
    final inbound = _inboundListener();
    final port = (inbound['LocalPort'] as num?)?.toInt();
    if (port == null || port <= 0 || port >= 65536) {
      return '请填写本地监听端口';
    }
    final fragment = _group('Fragment4RayItem');
    for (final key in <String>['Lengths', 'Delays']) {
      for (final range in _list(fragment, key)) {
        if (!_isValidRange(range)) return '请填写正确的分片参数';
      }
    }
    final maxSplit = _str(fragment, 'MaxSplit');
    if (maxSplit != null && maxSplit.isNotEmpty) {
      final value = int.tryParse(maxSplit);
      if (value == null || value < 0 || value > 10000) {
        return '请填写正确的分片参数';
      }
    }
    return null;
  }

  /// Upstream `Utils.TryParseRange`: a single positive integer or `a-b`.
  static bool _isValidRange(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return false;
    final parts = text.split('-');
    if (parts.length == 1) {
      final value = int.tryParse(parts[0]);
      return value != null && value >= 0;
    }
    if (parts.length == 2) {
      final start = int.tryParse(parts[0]);
      final end = int.tryParse(parts[1]);
      return start != null &&
          end != null &&
          start >= 0 &&
          end >= 0 &&
          start <= end;
    }
    return false;
  }

  void _save({bool applyAfter = false}) {
    final localError = _validateDraft();
    if (localError != null) {
      setState(() => _error = localError);
      return;
    }
    final previousAutoRun = _loadedAutoRun();
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveDocument(_draft);
    if (result.ok) {
      // Upstream writes autostart only after the config save succeeded, and
      // only the saved value takes effect. Skip the host write entirely when
      // the value did not change (evidence runs must not touch the Run key).
      final savedAutoRun = _bool(_group('GuiItem'), 'AutoRun');
      if (savedAutoRun != previousAutoRun) {
        _syncAutostart(savedAutoRun);
      }
      // Core-affecting fields stay dormant until the plan is re-applied:
      // keep the dialog open so the 未应用 hint + 应用 entry are visible,
      // unless this *is* the apply path.
      if (result.restartCoreFields.isNotEmpty && !applyAfter) {
        setState(() {});
        return;
      }
      if (!mounted) return;
      Navigator.of(context).pop();
      if (applyAfter) {
        ref.read(runtimeControllerProvider.notifier).applyActive();
      }
    } else {
      setState(() => _error = result.error?.messageKey ?? 'error.save_failed');
    }
  }

  bool _loadedAutoRun() {
    final document = ref.read(settingsControllerProvider).document;
    final gui = document['GuiItem'];
    if (gui is Map<String, dynamic>) return gui['AutoRun'] == true;
    return false;
  }

  /// Write/clear the Run key for the saved value and report honestly.
  /// Only runs after a successful settings save, never on toggle or cancel.
  void _syncAutostart(bool enabled) {
    final ok = _applyAutostartWrite(enabled);
    if (!mounted) return;
    ScaffoldMessenger.maybeOf(context)?.showSnackBar(
      SnackBar(
        content: Text(ok ? '开机自启已更新' : '开机自启写入失败'),
        duration: const Duration(seconds: 2),
      ),
    );
  }

  // -- helpers -----------------------------------------------------------

  static String? _str(Map<String, dynamic> m, String key) {
    final value = m[key];
    return value is String ? value : null;
  }

  static int? _int(Map<String, dynamic> m, String key) =>
      (m[key] as num?)?.toInt();
  static bool _bool(Map<String, dynamic> m, String key) => m[key] == true;
  static List<String> _list(Map<String, dynamic> m, String key) {
    final value = m[key];
    if (value is List) return value.map((e) => e.toString()).toList();
    return <String>[];
  }

  List<DropdownMenuItem<T>> _items<T>(List<(String, T)> entries) => entries
      .map((e) => DropdownMenuItem<T>(value: e.$2, child: Text(e.$1)))
      .toList();

  // -- tabs --------------------------------------------------------------

  Widget _coreTab() {
    final core = _group('CoreBasicItem');
    final inbound = _inboundListener();
    return _scroll(<Widget>[
      SettingsSection(
        title: '本地监听',
        child: <Widget>[
          SettingsNumberField(
            key: const ValueKey('settings-local-port'),
            label: '本地端口 (LocalPort)',
            value: _int(inbound, 'LocalPort'),
            onChanged: (v) => _set('Inbound', 'LocalPort', v),
          ),
          SettingsCheckbox(
            label: '第二本地端口',
            value: _bool(inbound, 'SecondLocalPortEnabled'),
            onChanged: (v) => _set('Inbound', 'SecondLocalPortEnabled', v),
          ),
          SettingsCheckbox(
            label: 'UDP 转发',
            value: _bool(inbound, 'UdpEnabled'),
            onChanged: (v) => _set('Inbound', 'UdpEnabled', v),
          ),
          SettingsCheckbox(
            label: '嗅探 (SniffingEnabled)',
            value: _bool(inbound, 'SniffingEnabled'),
            onChanged: (v) => _set('Inbound', 'SniffingEnabled', v),
          ),
          if (_bool(inbound, 'SniffingEnabled')) ...<Widget>[
            Wrap(
              spacing: 8,
              children: <Widget>[
                for (final option in <String>['http', 'tls', 'quic'])
                  FilterChip(
                    label: Text(option, style: const TextStyle(fontSize: 11)),
                    selected: _list(inbound, 'DestOverride').contains(option),
                    onSelected: (on) {
                      final next = _list(inbound, 'DestOverride');
                      if (on) {
                        next.add(option);
                      } else {
                        next.remove(option);
                      }
                      _set('Inbound', 'DestOverride', next);
                    },
                  ),
              ],
            ),
            SettingsCheckbox(
              label: '仅路由 (RouteOnly)',
              value: _bool(inbound, 'RouteOnly'),
              onChanged: (v) => _set('Inbound', 'RouteOnly', v),
            ),
          ],
          SettingsCheckbox(
            label: '允许来自局域网的连接',
            value: _bool(inbound, 'AllowLANConn'),
            onChanged: (v) => _set('Inbound', 'AllowLANConn', v),
          ),
          if (_bool(inbound, 'AllowLANConn'))
            SettingsCheckbox(
              label: '为局域网使用新端口',
              value: _bool(inbound, 'NewPort4LAN'),
              onChanged: (v) => _set('Inbound', 'NewPort4LAN', v),
            ),
          if (_bool(inbound, 'AllowLANConn') &&
              _bool(inbound, 'NewPort4LAN')) ...[
            SettingsTextField(
              label: '用户名 (User)',
              value: _str(inbound, 'User'),
              onChanged: (v) => _set('Inbound', 'User', v),
            ),
            SettingsTextField(
              label: '密码 (Pass)',
              value: _str(inbound, 'Pass'),
              onChanged: (v) => _set('Inbound', 'Pass', v),
            ),
          ],
        ],
      ),
      SettingsSection(
        title: '日志与指纹',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用日志 (LogEnabled)',
            value: _bool(core, 'LogEnabled'),
            onChanged: (v) => _set('CoreBasicItem', 'LogEnabled', v),
          ),
          SettingsDropdown<String>(
            label: '日志等级 (Loglevel)',
            value: _str(core, 'Loglevel'),
            items: _items(<(String, String)>[
              ('debug', 'debug'),
              ('info', 'info'),
              ('warning', 'warning'),
              ('error', 'error'),
              ('none', 'none'),
            ]),
            onChanged: (v) => _set('CoreBasicItem', 'Loglevel', v),
          ),
          SettingsTextField(
            label: '默认指纹 (DefFingerprint)',
            value: _str(core, 'DefFingerprint'),
            onChanged: (v) => _set('CoreBasicItem', 'DefFingerprint', v),
          ),
          SettingsTextField(
            label: '默认 UA (DefUserAgent)',
            value: _str(core, 'DefUserAgent'),
            onChanged: (v) => _set('CoreBasicItem', 'DefUserAgent', v),
          ),
        ],
      ),
      SettingsSection(
        title: '出站绑定',
        child: <Widget>[
          SettingsTextField(
            label: 'SendThrough',
            value: _str(core, 'SendThrough'),
            onChanged: (v) => _set('CoreBasicItem', 'SendThrough', v),
          ),
          SettingsTextField(
            label: 'BindInterface',
            value: _str(core, 'BindInterface'),
            onChanged: (v) => _set('CoreBasicItem', 'BindInterface', v),
          ),
        ],
      ),
      SettingsSection(
        title: '多路复用 (Mux)',
        child: <Widget>[
          SettingsNumberField(
            label: 'Ray 并发 (Concurrency)',
            value: _int(_group('Mux4RayItem'), 'Concurrency'),
            onChanged: (v) => _set('Mux4RayItem', 'Concurrency', v),
          ),
          SettingsNumberField(
            label: 'Ray XUDP 并发',
            value: _int(_group('Mux4RayItem'), 'XudpConcurrency'),
            onChanged: (v) => _set('Mux4RayItem', 'XudpConcurrency', v),
          ),
          SettingsDropdown<String>(
            label: 'Ray XUDP 443 代理',
            value: _str(_group('Mux4RayItem'), 'XudpProxyUDP443'),
            items: _items(<(String, String)>[
              ('reject', 'reject'),
              ('skip', 'skip'),
            ]),
            onChanged: (v) => _set('Mux4RayItem', 'XudpProxyUDP443', v),
          ),
          SettingsDropdown<String>(
            label: 'sing-box 协议 (Mux4SboxProtocol)',
            value: _str(_group('Mux4SboxItem'), 'Protocol'),
            items: _items(<(String, String)>[
              ('h2mux', 'h2mux'),
              ('smux', 'smux'),
              ('yamux', 'yamux'),
              ('（不启用）', ''),
            ]),
            onChanged: (v) => _set('Mux4SboxItem', 'Protocol', v),
          ),
          SettingsCheckbox(
            label: 'sing-box 缓存文件 (EnableCacheFile4Sbox)',
            value: _bool(core, 'EnableCacheFile4Sbox'),
            onChanged: (v) => _set('CoreBasicItem', 'EnableCacheFile4Sbox', v),
          ),
        ],
      ),
      SettingsSection(
        title: 'Hysteria2',
        child: <Widget>[
          SettingsNumberField(
            label: '上行 Mbps',
            value: _int(_group('HysteriaItem'), 'UpMbps'),
            onChanged: (v) => _set('HysteriaItem', 'UpMbps', v),
          ),
          SettingsNumberField(
            label: '下行 Mbps',
            value: _int(_group('HysteriaItem'), 'DownMbps'),
            onChanged: (v) => _set('HysteriaItem', 'DownMbps', v),
          ),
          SettingsNumberField(
            label: 'Hop 间隔 (HopInterval)',
            value: _int(_group('HysteriaItem'), 'HopInterval'),
            onChanged: (v) => _set('HysteriaItem', 'HopInterval', v),
          ),
        ],
      ),
      SettingsSection(
        title: '分片 (Fragment)',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用分片 (EnableFragment)',
            value: _bool(core, 'EnableFragment'),
            onChanged: (v) => _set('CoreBasicItem', 'EnableFragment', v),
          ),
          SettingsDropdown<String>(
            label: '分片包 (Packets)',
            value: _str(_group('Fragment4RayItem'), 'Packets'),
            items: _items(<(String, String)>[
              ('tlshello', 'tlshello'),
              ('1-1', '1-1'),
              ('1-2', '1-2'),
              ('1-3', '1-3'),
              ('1-4', '1-4'),
              ('1-5', '1-5'),
            ]),
            onChanged: (v) => _set('Fragment4RayItem', 'Packets', v),
          ),
          SettingsTextField(
            label: '长度 (Lengths，逗号分隔)',
            value: _list(_group('Fragment4RayItem'), 'Lengths').join(','),
            width: 320,
            onChanged: (v) => _set(
              'Fragment4RayItem',
              'Lengths',
              v == null ? <String>[] : v.split(','),
            ),
          ),
          SettingsTextField(
            label: '延迟 (Delays，逗号分隔)',
            value: _list(_group('Fragment4RayItem'), 'Delays').join(','),
            width: 320,
            onChanged: (v) => _set(
              'Fragment4RayItem',
              'Delays',
              v == null ? <String>[] : v.split(','),
            ),
          ),
          SettingsTextField(
            label: 'MaxSplit',
            value: _str(_group('Fragment4RayItem'), 'MaxSplit'),
            onChanged: (v) => _set('Fragment4RayItem', 'MaxSplit', v),
          ),
          SettingsCheckbox(
            label: '最终分片 (EnableFinalFragment)',
            value: _bool(core, 'EnableFinalFragment'),
            onChanged: (v) => _set('CoreBasicItem', 'EnableFinalFragment', v),
          ),
        ],
      ),
      SettingsSection(
        title: '历史保留（原版 KCP 页在 OptionSettingWindow.xaml 中为注释）',
        child: <Widget>[
          SettingsNumberField(
            label: 'KCP MTU',
            value: _int(_group('KcpItem'), 'Mtu'),
            onChanged: (v) => _set('KcpItem', 'Mtu', v),
          ),
          SettingsNumberField(
            label: 'KCP TTI',
            value: _int(_group('KcpItem'), 'Tti'),
            onChanged: (v) => _set('KcpItem', 'Tti', v),
          ),
          SettingsNumberField(
            label: 'KCP 上行容量',
            value: _int(_group('KcpItem'), 'UplinkCapacity'),
            onChanged: (v) => _set('KcpItem', 'UplinkCapacity', v),
          ),
          SettingsNumberField(
            label: 'KCP 下行容量',
            value: _int(_group('KcpItem'), 'DownlinkCapacity'),
            onChanged: (v) => _set('KcpItem', 'DownlinkCapacity', v),
          ),
          SettingsNumberField(
            label: '拥塞窗口倍数 (CwndMultiplier)',
            value: _int(_group('KcpItem'), 'CwndMultiplier'),
            onChanged: (v) => _set('KcpItem', 'CwndMultiplier', v),
          ),
          SettingsNumberField(
            label: '最大发送窗口 (MaxSendingWindow)',
            value: _int(_group('KcpItem'), 'MaxSendingWindow'),
            onChanged: (v) => _set('KcpItem', 'MaxSendingWindow', v),
          ),
        ],
      ),
    ]);
  }

  Widget _displayTab() {
    final gui = _group('GuiItem');
    final ui = _group('UiItem');
    return _scroll(<Widget>[
      SettingsSection(
        title: '显示',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用统计 (EnableStatistics，需重启应用)',
            value: _bool(gui, 'EnableStatistics'),
            onChanged: (v) => _set('GuiItem', 'EnableStatistics', v),
          ),
          SettingsCheckbox(
            label: '显示实时速率 (DisplayRealTimeSpeed，需重启应用)',
            value: _bool(gui, 'DisplayRealTimeSpeed'),
            onChanged: (v) => _set('GuiItem', 'DisplayRealTimeSpeed', v),
          ),
          SettingsCheckbox(
            label: '保留旧的重组结果 (KeepOlderDedupl)',
            value: _bool(gui, 'KeepOlderDedupl'),
            onChanged: (v) => _set('GuiItem', 'KeepOlderDedupl', v),
          ),
          SettingsCheckbox(
            label: '自动调整列宽 (EnableAutoAdjustMainLvColWidth)',
            value: _bool(ui, 'EnableAutoAdjustMainLvColWidth'),
            onChanged: (v) =>
                _set('UiItem', 'EnableAutoAdjustMainLvColWidth', v),
          ),
          SettingsCheckbox(
            label: '隐藏 IP 信息 (HideColumnIpInfo)',
            value: _bool(ui, 'HideColumnIpInfo'),
            onChanged: (v) => _set('UiItem', 'HideColumnIpInfo', v),
          ),
          SettingsCheckbox(
            label: '双击激活节点 (DoubleClick2Activate)',
            value: _bool(ui, 'DoubleClick2Activate'),
            onChanged: (v) => _set('UiItem', 'DoubleClick2Activate', v),
          ),
          SettingsDropdown<String>(
            label: '主界面布局',
            value:
                _str(ui, 'MainGirdOrientation') ??
                _int(ui, 'MainGirdOrientation')?.toString(),
            items: _items(<(String, String)>[
              ('水平', '0'),
              ('垂直', '1'),
              ('标签', '2'),
            ]),
            onChanged: (v) => _set(
              'UiItem',
              'MainGirdOrientation',
              int.tryParse(v ?? '') ?? 1,
            ),
          ),
        ],
      ),
      SettingsSection(
        title: '窗口与托盘',
        child: <Widget>[
          SettingsCheckbox(
            label: '关闭时隐藏到托盘 (Hide2TrayWhenClose)',
            value: _bool(ui, 'Hide2TrayWhenClose'),
            onChanged: (v) => _set('UiItem', 'Hide2TrayWhenClose', v),
          ),
          SettingsCheckbox(
            label: '启动时自动隐藏 (AutoHideStartup)',
            value: _bool(ui, 'AutoHideStartup'),
            onChanged: (v) => _set('UiItem', 'AutoHideStartup', v),
          ),
          // Upstream binds this row to IsMacOS visibility; keep the same gate.
          if (Platform.isMacOS)
            SettingsCheckbox(
              label: '在 Dock 中显示 (MacOSShowInDock)',
              value: _bool(ui, 'MacOSShowInDock'),
              onChanged: (v) => _set('UiItem', 'MacOSShowInDock', v),
            ),
          SettingsCheckbox(
            label: '拖放排序 (EnableDragDropSort，需重启应用)',
            value: _bool(ui, 'EnableDragDropSort'),
            onChanged: (v) => _set('UiItem', 'EnableDragDropSort', v),
          ),
          SettingsNumberField(
            label: '托盘节点数上限 (TrayMenuServersLimit)',
            value: _int(gui, 'TrayMenuServersLimit'),
            onChanged: (v) => _set('GuiItem', 'TrayMenuServersLimit', v),
          ),
          SettingsNumberField(
            label: '自动更新间隔 (AutoUpdateInterval)',
            value: _int(gui, 'AutoUpdateInterval'),
            onChanged: (v) => _set('GuiItem', 'AutoUpdateInterval', v),
          ),
          // F-DESKTOP-003: the toggle only edits the draft. The real Run key
          // is written after a successful save (upstream `SaveSettingAsync`
          // runs `AutoStartupHandler.UpdateTask` after `SaveConfig`), so
          // Cancel never touches host autostart.
          SettingsCheckbox(
            key: const ValueKey('autorun-toggle'),
            label: '开机自启 (AutoRun)',
            value: _bool(gui, 'AutoRun'),
            onChanged: (v) => _set('GuiItem', 'AutoRun', v),
          ),
        ],
      ),
      SettingsSection(
        title: '字体与语言',
        child: <Widget>[
          SettingsTextField(
            label: '字体族 (CurrentFontFamily)',
            value: _str(ui, 'CurrentFontFamily'),
            onChanged: (v) => _set('UiItem', 'CurrentFontFamily', v),
          ),
          SettingsNumberField(
            label: '字号 (CurrentFontSize)',
            value: _int(ui, 'CurrentFontSize'),
            onChanged: (v) => _set('UiItem', 'CurrentFontSize', v),
          ),
          SettingsDropdown<String>(
            key: const ValueKey('settings-language'),
            label: '语言 (CurrentLanguage，需重启应用)',
            value: _str(ui, 'CurrentLanguage') ?? 'zh-Hans',
            items: _items(<(String, String)>[
              ('中文简体', 'zh-Hans'),
              ('中文繁體', 'zh-Hant'),
              ('English', 'en'),
              ('فارسی', 'fa'),
              ('Français', 'fr'),
              ('Русский', 'ru'),
              ('Magyar', 'hu'),
              ('Bahasa Indonesia', 'id'),
              ('Azərbaycan', 'az'),
            ]),
            onChanged: (v) => _set('UiItem', 'CurrentLanguage', v),
          ),
        ],
      ),
      SettingsSection(
        title: '测速',
        child: <Widget>[
          SettingsNumberField(
            label: '测速超时 (SpeedTestTimeout)',
            value: _int(_group('SpeedTestItem'), 'SpeedTestTimeout'),
            onChanged: (v) => _set('SpeedTestItem', 'SpeedTestTimeout', v),
          ),
          SettingsNumberField(
            label: '并发数 (MixedConcurrencyCount)',
            value: _int(_group('SpeedTestItem'), 'MixedConcurrencyCount'),
            onChanged: (v) => _set('SpeedTestItem', 'MixedConcurrencyCount', v),
          ),
          SettingsTextField(
            label: '测速 URL',
            value: _str(_group('SpeedTestItem'), 'SpeedTestUrl'),
            width: 360,
            onChanged: (v) => _set('SpeedTestItem', 'SpeedTestUrl', v),
          ),
          SettingsTextField(
            label: 'Ping URL',
            value: _str(_group('SpeedTestItem'), 'SpeedPingTestUrl'),
            width: 360,
            onChanged: (v) => _set('SpeedTestItem', 'SpeedPingTestUrl', v),
          ),
          SettingsTextField(
            label: 'UDP 测试目标 (UdpTestTarget)',
            value: _str(_group('SpeedTestItem'), 'UdpTestTarget'),
            width: 320,
            onChanged: (v) => _set('SpeedTestItem', 'UdpTestTarget', v),
          ),
          SettingsTextField(
            label: 'IP API URL',
            value: _str(_group('SpeedTestItem'), 'IPAPIUrl'),
            width: 320,
            onChanged: (v) => _set('SpeedTestItem', 'IPAPIUrl', v),
          ),
        ],
      ),
      SettingsSection(
        title: '资源与证书',
        child: <Widget>[
          SettingsTextField(
            key: const ValueKey('settings-sub-convert-url'),
            label: '订阅转换 (SubConvertUrl)',
            value: _str(_group('ConstItem'), 'SubConvertUrl'),
            width: 360,
            onChanged: (v) => _set('ConstItem', 'SubConvertUrl', v),
          ),
          SettingsTextField(
            key: const ValueKey('settings-geo-source-url'),
            label: 'Geo 文件来源 (GeoSourceUrl)',
            value: _str(_group('ConstItem'), 'GeoSourceUrl'),
            width: 360,
            onChanged: (v) => _set('ConstItem', 'GeoSourceUrl', v),
          ),
          SettingsTextField(
            key: const ValueKey('settings-srs-source-url'),
            label: 'SRS 文件来源 (SrsSourceUrl)',
            value: _str(_group('ConstItem'), 'SrsSourceUrl'),
            width: 360,
            onChanged: (v) => _set('ConstItem', 'SrsSourceUrl', v),
          ),
          SettingsTextField(
            key: const ValueKey('settings-route-rules-source-url'),
            label: '路由规则来源 (RouteRulesTemplateSourceUrl)',
            value: _str(_group('ConstItem'), 'RouteRulesTemplateSourceUrl'),
            width: 360,
            onChanged: (v) =>
                _set('ConstItem', 'RouteRulesTemplateSourceUrl', v),
          ),
          SettingsCheckbox(
            key: const ValueKey('settings-enable-hwa'),
            label: '硬件加速 (EnableHWA，需重启应用)',
            value: _bool(gui, 'EnableHWA'),
            onChanged: (v) => _set('GuiItem', 'EnableHWA', v),
          ),
          SettingsDropdown<String>(
            key: const ValueKey('settings-root-cert'),
            label: '根证书来源 (RootCertProvider)',
            value: _str(gui, 'RootCertProvider') ?? 'system',
            items: _items(<(String, String)>[
              ('系统 (system)', 'system'),
              ('Mozilla', 'mozilla'),
              ('Chrome', 'chrome'),
            ]),
            onChanged: (v) => _set('GuiItem', 'RootCertProvider', v),
          ),
        ],
      ),
      // Upstream keeps DNS FakeIP / Happy Eyeballs in DNSSettingWindow, not the
      // option window; retained here so the config stays editable until FIX-16B
      // adds the DNS window. Not counted as an upstream Window field.
      SettingsSection(
        title: '历史保留（原版为独立 DNS 设置窗口）',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用 FakeIP',
            value: _bool(_group('SimpleDNSItem'), 'FakeIP'),
            onChanged: (v) => _set('SimpleDNSItem', 'FakeIP', v),
          ),
          if (_bool(_group('SimpleDNSItem'), 'FakeIP'))
            SettingsCheckbox(
              label: '全局 FakeIP (GlobalFakeIp)',
              value: _bool(_group('SimpleDNSItem'), 'GlobalFakeIp'),
              onChanged: (v) => _set('SimpleDNSItem', 'GlobalFakeIp', v),
            ),
          SettingsCheckbox(
            label: '启用 Happy Eyeballs (EnableHappyEyeballs)',
            value: _bool(_group('SimpleDNSItem'), 'EnableHappyEyeballs'),
            onChanged: (v) => _set('SimpleDNSItem', 'EnableHappyEyeballs', v),
          ),
          SettingsNumberField(
            label: '尝试延迟 (TryDelayMs)',
            value: _int(_group('HappyEyeballs4RayItem'), 'TryDelayMs'),
            onChanged: (v) => _set('HappyEyeballs4RayItem', 'TryDelayMs', v),
          ),
          SettingsCheckbox(
            label: '优先 IPv6 (PrioritizeIPv6)',
            value: _bool(_group('HappyEyeballs4RayItem'), 'PrioritizeIPv6'),
            onChanged: (v) =>
                _set('HappyEyeballs4RayItem', 'PrioritizeIPv6', v),
          ),
          SettingsNumberField(
            label: '交错 (Interleave)',
            value: _int(_group('HappyEyeballs4RayItem'), 'Interleave'),
            onChanged: (v) => _set('HappyEyeballs4RayItem', 'Interleave', v),
          ),
          SettingsNumberField(
            label: '最大并发尝试 (MaxConcurrentTry)',
            value: _int(_group('HappyEyeballs4RayItem'), 'MaxConcurrentTry'),
            onChanged: (v) =>
                _set('HappyEyeballs4RayItem', 'MaxConcurrentTry', v),
          ),
        ],
      ),
      // ClashUIItem has no OptionSettingWindow control upstream; retained so the
      // config stays editable until FIX-16C wires the Clash UI consumers.
      SettingsSection(
        title: '历史保留（原版 Clash UI 设置）',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用 IPv6 (ClashUIItem.EnableIPv6)',
            value: _bool(_group('ClashUIItem'), 'EnableIPv6'),
            onChanged: (v) => _set('ClashUIItem', 'EnableIPv6', v),
          ),
          SettingsCheckbox(
            label: '合并 Mixin (EnableMixinContent)',
            value: _bool(_group('ClashUIItem'), 'EnableMixinContent'),
            onChanged: (v) => _set('ClashUIItem', 'EnableMixinContent', v),
          ),
          SettingsNumberField(
            label: '代理排序 (ProxiesSorting)',
            value: _int(_group('ClashUIItem'), 'ProxiesSorting'),
            onChanged: (v) => _set('ClashUIItem', 'ProxiesSorting', v),
          ),
          SettingsCheckbox(
            label: '代理自动刷新',
            value: _bool(_group('ClashUIItem'), 'ProxiesAutoRefresh'),
            onChanged: (v) => _set('ClashUIItem', 'ProxiesAutoRefresh', v),
          ),
          SettingsNumberField(
            label: '代理刷新间隔',
            value: _int(_group('ClashUIItem'), 'ProxiesRefreshInterval'),
            onChanged: (v) => _set('ClashUIItem', 'ProxiesRefreshInterval', v),
          ),
        ],
      ),
    ]);
  }

  Widget _systemProxyTab() {
    final proxy = _group('SystemProxyItem');
    return _scroll(<Widget>[
      SettingsNote('系统代理的实际读写 (WinINET/WinHTTP) 属于 T13；本页仅保存配置。'),
      SettingsDropdown<int>(
        label: '系统代理类型 (SysProxyType)',
        value: _int(proxy, 'SysProxyType'),
        items: _items(<(String, int)>[
          ('清除', 0),
          ('设置', 1),
          ('不改变', 2),
          ('PAC', 3),
        ]),
        onChanged: (v) => _set('SystemProxyItem', 'SysProxyType', v),
      ),
      SettingsCheckbox(
        label: '忽略本地地址 (NotProxyLocalAddress)',
        value: _bool(proxy, 'NotProxyLocalAddress'),
        onChanged: (v) => _set('SystemProxyItem', 'NotProxyLocalAddress', v),
      ),
      SettingsTextField(
        label: '例外列表 (SystemProxyExceptions)',
        value: _str(proxy, 'SystemProxyExceptions'),
        width: 320,
        onChanged: (v) => _set('SystemProxyItem', 'SystemProxyExceptions', v),
      ),
      SettingsTextField(
        label: '高级协议 (SystemProxyAdvancedProtocol)',
        value: _str(proxy, 'SystemProxyAdvancedProtocol'),
        onChanged: (v) =>
            _set('SystemProxyItem', 'SystemProxyAdvancedProtocol', v),
      ),
      SettingsTextField(
        label: 'PAC 路径 (CustomSystemProxyPacPath)',
        value: _str(proxy, 'CustomSystemProxyPacPath'),
        width: 320,
        onChanged: (v) =>
            _set('SystemProxyItem', 'CustomSystemProxyPacPath', v),
      ),
      SettingsTextField(
        label: 'PAC 脚本路径 (CustomSystemProxyScriptPath)',
        value: _str(proxy, 'CustomSystemProxyScriptPath'),
        width: 320,
        onChanged: (v) =>
            _set('SystemProxyItem', 'CustomSystemProxyScriptPath', v),
      ),
    ]);
  }

  Widget _tunTab() {
    final tun = _group('TunModeItem');
    return _scroll(<Widget>[
      SettingsNote('TUN 模式的实际启用 (虚拟网卡/路由) 属于 T13；本页仅保存配置。'),
      SettingsCheckbox(
        label: '启用 Tun (EnableTun，需重启内核)',
        value: _bool(tun, 'EnableTun'),
        onChanged: (v) => _set('TunModeItem', 'EnableTun', v),
      ),
      SettingsCheckbox(
        label: '自动路由 (AutoRoute)',
        value: _bool(tun, 'AutoRoute'),
        onChanged: (v) => _set('TunModeItem', 'AutoRoute', v),
      ),
      SettingsCheckbox(
        label: '严格路由 (StrictRoute)',
        value: _bool(tun, 'StrictRoute'),
        onChanged: (v) => _set('TunModeItem', 'StrictRoute', v),
      ),
      SettingsDropdown<String>(
        label: '协议栈 (Stack)',
        value: _str(tun, 'Stack'),
        items: _items(<(String, String)>[
          ('gvisor', 'gvisor'),
          ('system', 'system'),
          ('mixed', 'mixed'),
        ]),
        onChanged: (v) => _set('TunModeItem', 'Stack', v),
      ),
      SettingsNumberField(
        label: 'MTU',
        value: _int(tun, 'Mtu'),
        onChanged: (v) => _set('TunModeItem', 'Mtu', v),
      ),
      SettingsDropdown<String>(
        label: 'ICMP 路由 (IcmpRouting)',
        value: _str(tun, 'IcmpRouting'),
        items: _items(<(String, String)>[
          ('rule', 'rule'),
          ('direct', 'direct'),
          ('unreachable', 'unreachable'),
          ('drop', 'drop'),
          ('reply', 'reply'),
        ]),
        onChanged: (v) => _set('TunModeItem', 'IcmpRouting', v),
      ),
      SettingsCheckbox(
        label: '启用 IPv6 地址 (EnableIPv6Address)',
        value: _bool(tun, 'EnableIPv6Address'),
        onChanged: (v) => _set('TunModeItem', 'EnableIPv6Address', v),
      ),
      SettingsCheckbox(
        label: '旧版保护 (EnableLegacyProtect)',
        value: _bool(tun, 'EnableLegacyProtect'),
        onChanged: (v) => _set('TunModeItem', 'EnableLegacyProtect', v),
      ),
      SettingsTextField(
        label: '路由排除地址 (逗号分隔)',
        value: _list(tun, 'RouteExcludeAddress').join(','),
        width: 320,
        onChanged: (v) => _set(
          'TunModeItem',
          'RouteExcludeAddress',
          v == null ? <String>[] : v.split(','),
        ),
      ),
      SettingsTextField(
        label: 'IPv4 地址 (IPv4Address)',
        value: _str(tun, 'IPv4Address'),
        onChanged: (v) => _set('TunModeItem', 'IPv4Address', v),
      ),
      SettingsTextField(
        label: 'IPv6 地址 (IPv6Address)',
        value: _str(tun, 'IPv6Address'),
        onChanged: (v) => _set('TunModeItem', 'IPv6Address', v),
      ),
    ]);
  }

  Widget _coreTypeTab() {
    final items = _coreTypeItems();
    return _scroll(<Widget>[
      SettingsNote('内核类型按协议选择 (ConfigType → CoreType)；仅覆盖上游有下拉框的 8 项。'),
      for (final entry in _coreTypeControls)
        SettingsDropdown<int>(
          label: '${entry.label} (${entry.configType})',
          value: _coreTypeValue(items, entry.configType),
          items: _items(_coreTypes.map((c) => (c.$1, c.$2)).toList()),
          onChanged: (v) => setState(() {
            _setCoreType(items, entry.configType, v ?? 2);
          }),
        ),
    ]);
  }

  Widget _scroll(List<Widget> children) => SingleChildScrollView(
    padding: const EdgeInsets.symmetric(horizontal: 4),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: children,
    ),
  );

  List<Map<String, dynamic>> _coreTypeItems() {
    final value = _draft['CoreTypeItem'];
    if (value is List) {
      return value.whereType<Map<String, dynamic>>().toList();
    }
    final created = <Map<String, dynamic>>[
      for (final entry in _coreTypeControls)
        <String, dynamic>{'ConfigType': entry.configType, 'CoreType': 2},
    ];
    _draft['CoreTypeItem'] = created;
    return created;
  }

  static int? _coreTypeValue(List<Map<String, dynamic>> items, int configType) {
    for (final item in items) {
      if ((item['ConfigType'] as num?)?.toInt() == configType) {
        return (item['CoreType'] as num?)?.toInt();
      }
    }
    return 2;
  }

  void _setCoreType(
    List<Map<String, dynamic>> items,
    int configType,
    int coreType,
  ) {
    for (final item in items) {
      if ((item['ConfigType'] as num?)?.toInt() == configType) {
        item['CoreType'] = coreType;
        return;
      }
    }
    items.add(<String, dynamic>{
      'ConfigType': configType,
      'CoreType': coreType,
    });
  }

  static const List<({String label, int configType})> _coreTypeControls =
      <({String label, int configType})>[
        (label: 'VMess', configType: 1),
        (label: 'Custom', configType: 2),
        (label: 'Shadowsocks', configType: 3),
        (label: 'SOCKS', configType: 4),
        (label: 'VLESS', configType: 5),
        (label: 'Trojan', configType: 6),
        (label: 'Hysteria2', configType: 7),
        (label: 'WireGuard', configType: 9),
      ];

  static const List<(String, int)> _coreTypes = <(String, int)>[
    ('v2fly', 1),
    ('Xray', 2),
    ('v2fly_v5', 4),
    ('mihomo', 13),
    ('hysteria', 21),
    ('naiveproxy', 22),
    ('tuic', 23),
    ('sing_box', 24),
    ('juicity', 25),
    ('hysteria2', 26),
    ('brook', 27),
    ('overtls', 28),
    ('shadowquic', 29),
    ('mieru', 30),
  ];
}
