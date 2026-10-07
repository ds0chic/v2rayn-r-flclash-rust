import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n_context.dart';

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
  const OptionSettingWindow({super.key, this.host, this.standalone = false});

  /// When non-null the editor runs in the independent desktop settings window:
  /// it loads its snapshot and persists the draft through [host] instead of the
  /// in-process settings controller, so the second engine never touches Rust.
  final SettingsEditorHost? host;

  /// When true the editor renders as a full window body (no dialog chrome).
  final bool standalone;

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
  bool _loadFailed = false;
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
    if (widget.host != null) {
      _loadHostSnapshot();
      return;
    }
    // Deferred so the provider is not mutated during the widget build; the
    // draft is derived in build() once the document is loaded.
    Future<void>.microtask(() {
      if (!mounted) return;
      ref.read(settingsControllerProvider.notifier).load();
      if (mounted) setState(() {});
    });
  }

  /// Seed the draft from the snapshot supplied by the main window. R4-12/D34:
  /// a failed or empty read stays an error state. The window must never present
  /// a defaults-only draft that 确定 could write over the real settings.
  Future<void> _loadHostSnapshot() async {
    try {
      final document = await widget.host!.loadSnapshot();
      if (!mounted) return;
      if (document.isEmpty) {
        setState(() {
          _loadFailed = true;
          _error = 'error.settings_load_failed';
        });
        return;
      }
      setState(() {
        // Fill missing/null scalar fields with the upstream defaults so a
        // partial snapshot renders the canonical value instead of a CLR
        // zero/blank (and a literal `null` can never leak into the draft).
        _draft = mergeWithSettingsDefaults(document);
        _draftInit = true;
        _loadFailed = false;
      });
    } on Object {
      if (!mounted) return;
      setState(() {
        _loadFailed = true;
        _error = 'error.settings_load_failed';
      });
    }
  }

  void _retryLoadSnapshot() {
    setState(() {
      _loadFailed = false;
      _error = null;
    });
    _loadHostSnapshot();
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

  /// Wave B (FLD-CFG-131..135): the ClashUI poll/sort keys rolled back to the
  /// persisted canonical when an in-process save fails, so the monitor tabs
  /// (which read the canonical document) never disagree with this window.
  static const _clashUiRollbackKeys = <String>[
    'ProxiesSorting',
    'ProxiesAutoRefresh',
    'ProxiesRefreshInterval',
    'ConnectionsAutoRefresh',
    'ConnectionsRefreshInterval',
  ];

  void _rollbackClashUiDraft() {
    if (widget.host != null) return;
    final persisted = ref
        .read(settingsControllerProvider)
        .document['ClashUIItem'];
    if (persisted is! Map) return;
    final group = _draft['ClashUIItem'];
    if (group is! Map<String, dynamic>) return;
    for (final key in _clashUiRollbackKeys) {
      if (persisted.containsKey(key)) {
        group[key] = persisted[key];
      } else {
        group.remove(key);
      }
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
    final SettingsViewState state = widget.host != null
        ? SettingsViewState(loaded: _draftInit)
        : ref.watch(settingsControllerProvider);
    // Seed the editable draft exactly once, from the persisted document, after
    // the first successful load. The old `_draft.isEmpty` guard failed whenever
    // a tab built (and created a group map) before load finished, which silently
    // replaced the stored values with defaults on reopen.
    if (widget.host == null && !_draftInit && state.loaded) {
      _draft = mergeWithSettingsDefaults(
        ref.read(settingsControllerProvider.notifier).draft(),
      );
      _draftInit = true;
    }
    if (widget.host != null && !_draftInit) {
      if (_loadFailed) {
        return Scaffold(
          body: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Text(
                  _error == null
                      ? context.tr('settingsLoadFailed')
                      : context.messageText(_error!),
                  key: const ValueKey('settings-load-error'),
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
                const SizedBox(height: 8),
                FilledButton(
                  key: const ValueKey('settings-load-retry'),
                  onPressed: _retryLoadSnapshot,
                  child: Text(context.tr('retry')),
                ),
              ],
            ),
          ),
        );
      }
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }

    final tabColumn = Column(
      children: <Widget>[
        TabBar(
          controller: _tabs,
          isScrollable: true,
          tabAlignment: TabAlignment.start,
          labelStyle: const TextStyle(fontSize: 12),
          // Frozen `OptionSettingWindow.xaml` TabItem headers:
          // TbSettingsCore / TbSettingsN / TbSettingsSystemproxy /
          // TbSettingsTunMode / TbSettingsCoreType.
          tabs: <Tab>[
            Tab(text: context.tr('TbSettingsCore')),
            Tab(text: context.tr('TbSettingsN')),
            Tab(text: context.tr('TbSettingsSystemproxy')),
            Tab(text: context.tr('TbSettingsTunMode')),
            Tab(text: context.tr('TbSettingsCoreType')),
          ],
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.all(4),
            child: Text(
              context.messageText(_error!),
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
              context.messageText(state.status!),
              style: const TextStyle(fontSize: 11),
            ),
          ),
        // SP-01: a failed settings read is a visible recovery entry, never a
        // silent default draft. The retry re-runs the controller load; saves
        // stay blocked until it succeeds, so the damaged source file is
        // never defaulted over.
        if (widget.host == null && state.loadFailed)
          Padding(
            padding: const EdgeInsets.all(4),
            child: FilledButton.tonal(
              key: const ValueKey('settings-load-retry-inline'),
              onPressed: () {
                ref.read(settingsControllerProvider.notifier).load();
                if (mounted) setState(() {});
              },
              child: Text(context.tr('retry')),
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
    );

    final actions = <Widget>[
      // Upstream `OptionSettingWindow` has a single 确定/取消 pair (btnSave /
      // btnCancel). FIX-08 draft semantics are preserved: 确定 saves the
      // visible draft and applies the real plan; 取消 discards the draft
      // without touching storage. A save error keeps the window open with the
      // error shown and never reports success or applies a stale document.
      FilledButton(
        key: const ValueKey('settings-save'),
        onPressed: () => _save(applyAfter: true),
        child: Text(context.tr('TbConfirm')),
      ),
      TextButton(
        key: const ValueKey('settings-cancel'),
        onPressed: _cancel,
        child: Text(context.tr('TbCancel')),
      ),
    ];

    if (widget.standalone) {
      return CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.escape): _cancel,
        },
        child: Focus(
          autofocus: true,
          child: Scaffold(
            body: Column(
              children: <Widget>[
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                    child: tabColumn,
                  ),
                ),
                const Divider(height: 1),
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 16,
                    vertical: 8,
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: <Widget>[
                      for (final action in actions)
                        Padding(
                          padding: const EdgeInsets.only(left: 8),
                          child: action,
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      );
    }

    return AlertDialog(
      // Upstream `OptionSettingWindow` Title = ResUI.menuSetting (设置).
      title: Text(
        context.tr('menuSetting'),
        style: const TextStyle(fontSize: 15),
      ),
      contentPadding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
      content: SizedBox(width: 720, height: 520, child: tabColumn),
      actions: actions,
    );
  }

  /// 取消 / Esc / title-bar close discard the draft without saving.
  void _cancel() {
    final host = widget.host;
    if (host != null) {
      host.close();
      return;
    }
    Navigator.of(context).pop();
  }

  /// Upstream `OptionSettingViewModel.SaveSettingAsync` rejects a non-numeric
  /// or out-of-range local port and malformed fragment ranges before touching
  /// storage.
  String? _validateDraft() {
    final inbound = _inboundListener();
    final port = (inbound['LocalPort'] as num?)?.toInt();
    if (port == null || port <= 0 || port >= 65536) {
      return 'validate.local_port';
    }
    final fragment = _group('Fragment4RayItem');
    for (final key in <String>['Lengths', 'Delays']) {
      for (final range in _list(fragment, key)) {
        if (!_isValidRange(range)) return 'validate.fragment';
      }
    }
    final maxSplit = _str(fragment, 'MaxSplit');
    // Upstream `OptionSettingViewModel.SaveSettingAsync` accepts a blank or
    // `Utils.TryParseMaxSplit(input, 0, 10000)` value (single int or
    // `from-to` range); an integer-only check wrongly rejects `1-3` (SP-24).
    if (maxSplit != null &&
        maxSplit.isNotEmpty &&
        !_isValidMaxSplit(maxSplit)) {
      return 'validate.fragment';
    }
    return null;
  }

  /// Upstream `Utils.String2List`: comma split that drops zero-length
  /// entries (`RemoveEmptyEntries`), so a trailing comma stores no phantom
  /// item. Whitespace-only entries are kept (upstream does not trim them);
  /// they fail the range validators instead, like upstream.
  static List<String> _splitList(String? raw) {
    if (raw == null) return <String>[];
    return raw.split(',').where((s) => s.isNotEmpty).toList();
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
      final start = int.tryParse(parts[0].trim());
      final end = int.tryParse(parts[1].trim());
      return start != null &&
          end != null &&
          start >= 0 &&
          end >= 0 &&
          start <= end;
    }
    return false;
  }

  /// Upstream `Utils.TryParseMaxSplit(input, 0, 10000)`: blank is accepted,
  /// otherwise one `int` or a `from-to` pair with both ends inside
  /// `[0, 10000]` and `from <= to` (`int.TryParse` tolerates surrounding
  /// whitespace, so each part is trimmed before parsing).
  static bool _isValidMaxSplit(String raw) {
    if (raw.trim().isEmpty) return true;
    final parts = raw.split('-');
    if (parts.length > 2) return false;
    final from = int.tryParse(parts[0].trim());
    if (from == null) return false;
    final to = parts.length == 2 ? int.tryParse(parts[1].trim()) : from;
    if (to == null) return false;
    return from >= 0 && to <= 10000 && from <= to;
  }

  Future<void> _save({bool applyAfter = false}) async {
    final localError = _validateDraft();
    if (localError != null) {
      setState(() => _error = localError);
      return;
    }
    final host = widget.host;
    if (host != null) {
      // R4-12/D34: without a successfully read snapshot there is no valid draft
      // to persist; refuse instead of writing defaults over the real settings.
      if (_loadFailed || !_draftInit) {
        setState(() => _error = 'error.settings_load_failed');
        return;
      }
      // Independent-window path: the main engine owns persistence and applies
      // the real plan; this engine only forwards the draft. A failed save keeps
      // the window open with the error shown.
      final outcome = await host.save(_draft);
      if (!mounted) return;
      if (outcome.ok) {
        // R4-13: carry the restart/apply notice back to the user before the
        // window closes instead of silently reporting success.
        final messenger = ScaffoldMessenger.maybeOf(context);
        await host.close();
        if (outcome.message != null) {
          messenger?.showSnackBar(
            SnackBar(
              content: Text(outcome.message!),
              duration: const Duration(seconds: 3),
            ),
          );
        }
      } else {
        setState(
          () => _error = outcome.message ?? 'error.settings_save_failed',
        );
      }
      return;
    }
    // In-process path: the controller persists the draft, syncs autostart and
    // awaits the real plan apply. The outcome distinguishes "已保存" from
    // "已应用" so an apply failure stays visible instead of a fake success.
    final outcome = applyAfter
        ? await ref
              .read(settingsControllerProvider.notifier)
              .saveAndApply(_draft)
        : _saveOnly();
    if (!mounted) return;
    if (outcome.ok) {
      Navigator.of(context).pop();
    } else {
      setState(() {
        _error = outcome.message ?? 'error.settings_save_failed';
        _rollbackClashUiDraft();
      });
    }
  }

  /// Legacy save-only path (used when [applyAfter] is false): persists the
  /// draft and reports the restart hint without driving the runtime apply.
  SettingsApplyOutcome _saveOnly() {
    final result = ref
        .read(settingsControllerProvider.notifier)
        .saveDocument(_draft);
    return SettingsApplyOutcome(
      ok: result.ok,
      saved: result.ok,
      applied: false,
      message: result.ok
          ? null
          : (result.error?.messageKey ?? 'error.settings_save_failed'),
      statusKey: result.ok ? 'settings.saved' : null,
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
          // Frozen XAML row 0: TbSettingsSocksPort.
          SettingsNumberField(
            key: const ValueKey('settings-local-port'),
            label: '本地混合监听端口',
            value: _int(inbound, 'LocalPort'),
            onChanged: (v) => _set('Inbound', 'LocalPort', v),
          ),
          // Frozen XAML row 0, column 2: TbSettingsSocksPortTip (O4).
          const Padding(
            padding: EdgeInsets.only(left: 172, bottom: 2),
            child: Text(
              'Pac 端口 = +3；Xray API 端口 = +4；mihomo API 端口 = +5；',
              style: TextStyle(fontSize: 11),
            ),
          ),
          // Frozen XAML row 2: TbSettingsSecondLocalPortEnabled.
          SettingsCheckbox(
            label: '开启第二个本地监听端口',
            value: _bool(inbound, 'SecondLocalPortEnabled'),
            onChanged: (v) => _set('Inbound', 'SecondLocalPortEnabled', v),
          ),
          // Frozen XAML row 3: TbSettingsUdpEnabled.
          SettingsCheckbox(
            label: '开启 UDP',
            value: _bool(inbound, 'UdpEnabled'),
            onChanged: (v) => _set('Inbound', 'UdpEnabled', v),
          ),
          // Frozen XAML row 4: TbSettingsSniffingEnabled.
          SettingsCheckbox(
            label: '开启流量探测',
            value: _bool(inbound, 'SniffingEnabled'),
            onChanged: (v) => _set('Inbound', 'SniffingEnabled', v),
          ),
          // Frozen XAML row 5: TbSettingsDestOverride. Upstream shows the chip
          // list unconditionally (not gated on 开启流量探测).
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 3),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                const SizedBox(
                  width: 168,
                  child: Padding(
                    padding: EdgeInsets.only(top: 2),
                    child: Text('流量探测类型', style: TextStyle(fontSize: 12)),
                  ),
                ),
                Expanded(
                  child: Wrap(
                    spacing: 8,
                    children: <Widget>[
                      for (final option in <String>['http', 'tls', 'quic'])
                        FilterChip(
                          label: Text(
                            option,
                            style: const TextStyle(fontSize: 11),
                          ),
                          selected: _list(
                            inbound,
                            'DestOverride',
                          ).contains(option),
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
                ),
              ],
            ),
          ),
          // Frozen XAML row 6: TbSettingsRouteOnly.
          SettingsCheckbox(
            label: '仅限路由 (routeOnly)',
            value: _bool(inbound, 'RouteOnly'),
            onChanged: (v) => _set('Inbound', 'RouteOnly', v),
          ),
          // Frozen XAML row 7: TbSettingsAllowLAN.
          SettingsCheckbox(
            label: '允许来自局域网的连接',
            value: _bool(inbound, 'AllowLANConn'),
            onChanged: (v) => _set('Inbound', 'AllowLANConn', v),
          ),
          // Frozen XAML row 8: TbSettingsNewPort4LAN. Upstream renders it
          // unconditionally; only the User/Pass edits are disabled until set.
          SettingsCheckbox(
            label: '为局域网开启新的端口',
            value: _bool(inbound, 'NewPort4LAN'),
            onChanged: (v) => _set('Inbound', 'NewPort4LAN', v),
          ),
          // Frozen XAML rows 9/10: TbSettingsUser / TbSettingsPass. Upstream
          // renders both unconditionally (O5: they were missing from RC) but
          // binds `togNewPort4LAN` to `txtuser.IsEnabled`/`txtpass.IsEnabled`,
          // so the auth fields are editable only when the LAN port is enabled.
          SettingsTextField(
            label: '认证用户名',
            value: _str(inbound, 'User'),
            enabled: _bool(inbound, 'NewPort4LAN'),
            onChanged: (v) => _set('Inbound', 'User', v),
          ),
          SettingsTextField(
            label: '认证密码',
            value: _str(inbound, 'Pass'),
            enabled: _bool(inbound, 'NewPort4LAN'),
            onChanged: (v) => _set('Inbound', 'Pass', v),
          ),
        ],
      ),
      SettingsSection(
        title: '日志与指纹',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用日志存到文件',
            value: _bool(core, 'LogEnabled'),
            onChanged: (v) => _set('CoreBasicItem', 'LogEnabled', v),
          ),
          SettingsDropdown<String>(
            label: '日志等级',
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
            label: '默认 TLS 指纹 (fingerprint)',
            value: _str(core, 'DefFingerprint'),
            onChanged: (v) => _set('CoreBasicItem', 'DefFingerprint', v),
          ),
          SettingsTextField(
            label: '用户代理 (User-Agent)',
            value: _str(core, 'DefUserAgent'),
            onChanged: (v) => _set('CoreBasicItem', 'DefUserAgent', v),
          ),
          const Padding(
            padding: EdgeInsets.only(left: 172, bottom: 2),
            child: Text(
              '仅对 raw/http、ws、gRPC、xhttp 生效',
              style: TextStyle(fontSize: 11),
            ),
          ),
        ],
      ),
      SettingsSection(
        title: '出站绑定',
        child: <Widget>[
          SettingsTextField(
            label: '本地出站地址 (SendThrough)',
            value: _str(core, 'SendThrough'),
            onChanged: (v) => _set('CoreBasicItem', 'SendThrough', v),
          ),
          SettingsTextField(
            label: '绑定网口',
            value: _str(core, 'BindInterface'),
            onChanged: (v) => _set('CoreBasicItem', 'BindInterface', v),
          ),
        ],
      ),
      SettingsSection(
        title: '多路复用 (Mux)',
        child: <Widget>[
          SettingsNumberField(
            label: 'Xray Mux concurrency',
            value: _int(_group('Mux4RayItem'), 'Concurrency'),
            onChanged: (v) => _set('Mux4RayItem', 'Concurrency', v),
          ),
          SettingsNumberField(
            label: 'Xray Mux XUDP concurrency',
            value: _int(_group('Mux4RayItem'), 'XudpConcurrency'),
            onChanged: (v) => _set('Mux4RayItem', 'XudpConcurrency', v),
          ),
          SettingsDropdown<String>(
            label: 'Xray Mux XUDP proxy UDP443',
            value: _str(_group('Mux4RayItem'), 'XudpProxyUDP443'),
            items: _items(<(String, String)>[
              ('reject', 'reject'),
              ('skip', 'skip'),
            ]),
            onChanged: (v) => _set('Mux4RayItem', 'XudpProxyUDP443', v),
          ),
          SettingsDropdown<String>(
            label: 'sing-box Mux 多路复用协议',
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
            label: '启用 sing-box (规则集文件) 的缓存文件',
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
            label: 'Hop 间隔',
            value: _int(_group('HysteriaItem'), 'HopInterval'),
            onChanged: (v) => _set('HysteriaItem', 'HopInterval', v),
          ),
        ],
      ),
      SettingsSection(
        title: '分片 (Fragment)',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用分片 (Fragment)',
            value: _bool(core, 'EnableFragment'),
            onChanged: (v) => _set('CoreBasicItem', 'EnableFragment', v),
          ),
          SettingsDropdown<String>(
            label: '分片包类型',
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
            key: const ValueKey('fragment-lengths'),
            label: '分片长度 (逗号分隔)',
            value: _list(_group('Fragment4RayItem'), 'Lengths').join(','),
            width: 320,
            onChanged: (v) =>
                _set('Fragment4RayItem', 'Lengths', _splitList(v)),
          ),
          SettingsTextField(
            label: '分片间隔 (逗号分隔)',
            value: _list(_group('Fragment4RayItem'), 'Delays').join(','),
            width: 320,
            onChanged: (v) => _set('Fragment4RayItem', 'Delays', _splitList(v)),
          ),
          SettingsTextField(
            key: const ValueKey('fragment-maxsplit'),
            label: '最大分片数',
            value: _str(_group('Fragment4RayItem'), 'MaxSplit'),
            onChanged: (v) => _set('Fragment4RayItem', 'MaxSplit', v),
          ),
          SettingsCheckbox(
            label: '启用末端分片',
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
            label: '拥塞窗口倍数',
            value: _int(_group('KcpItem'), 'CwndMultiplier'),
            onChanged: (v) => _set('KcpItem', 'CwndMultiplier', v),
          ),
          SettingsNumberField(
            label: '最大发送窗口',
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
            label: '启用流量统计 (需重启)',
            value: _bool(gui, 'EnableStatistics'),
            onChanged: (v) => _set('GuiItem', 'EnableStatistics', v),
          ),
          SettingsCheckbox(
            label: '显示实时速度 (需重启)',
            value: _bool(gui, 'DisplayRealTimeSpeed'),
            onChanged: (v) => _set('GuiItem', 'DisplayRealTimeSpeed', v),
          ),
          SettingsCheckbox(
            label: '去重时保留序号较小的项',
            value: _bool(gui, 'KeepOlderDedupl'),
            onChanged: (v) => _set('GuiItem', 'KeepOlderDedupl', v),
          ),
          SettingsCheckbox(
            label: '自动调整配置列宽在更新订阅后',
            value: _bool(ui, 'EnableAutoAdjustMainLvColWidth'),
            onChanged: (v) =>
                _set('UiItem', 'EnableAutoAdjustMainLvColWidth', v),
          ),
          SettingsCheckbox(
            label: '隐藏 IP 信息',
            value: _bool(ui, 'HideColumnIpInfo'),
            onChanged: (v) => _set('UiItem', 'HideColumnIpInfo', v),
          ),
          SettingsCheckbox(
            label: '主界面双击设为活动',
            value: _bool(ui, 'DoubleClick2Activate'),
            onChanged: (v) => _set('UiItem', 'DoubleClick2Activate', v),
          ),
          SettingsDropdown<String>(
            label: '主界面布局方向 (需重启)',
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
            label: '关闭窗口时隐藏至托盘',
            value: _bool(ui, 'Hide2TrayWhenClose'),
            onChanged: (v) => _set('UiItem', 'Hide2TrayWhenClose', v),
          ),
          SettingsCheckbox(
            label: '启动后隐藏窗口',
            value: _bool(ui, 'AutoHideStartup'),
            onChanged: (v) => _set('UiItem', 'AutoHideStartup', v),
          ),
          // Upstream binds this row to IsMacOS visibility; keep the same gate.
          if (Platform.isMacOS)
            SettingsCheckbox(
              label: 'macOS 在 Dock 栏中显示 (需重启)',
              value: _bool(ui, 'MacOSShowInDock'),
              onChanged: (v) => _set('UiItem', 'MacOSShowInDock', v),
            ),
          SettingsCheckbox(
            label: '启用配置拖放排序 (需重启)',
            value: _bool(ui, 'EnableDragDropSort'),
            onChanged: (v) => _set('UiItem', 'EnableDragDropSort', v),
          ),
          SettingsNumberField(
            label: '托盘右键菜单配置展示数量限制',
            value: _int(gui, 'TrayMenuServersLimit'),
            onChanged: (v) => _set('GuiItem', 'TrayMenuServersLimit', v),
          ),
          SettingsNumberField(
            label: '自动更新 Geo 文件的间隔 (小时)',
            value: _int(gui, 'AutoUpdateInterval'),
            onChanged: (v) => _set('GuiItem', 'AutoUpdateInterval', v),
          ),
          // F-DESKTOP-003: the toggle only edits the draft. The real Run key
          // is written after a successful save (upstream `SaveSettingAsync`
          // runs `AutoStartupHandler.UpdateTask` after `SaveConfig`), so
          // Cancel never touches host autostart.
          SettingsCheckbox(
            key: const ValueKey('autorun-toggle'),
            label: '开机启动 (可能会不成功)',
            value: _bool(gui, 'AutoRun'),
            onChanged: (v) => _set('GuiItem', 'AutoRun', v),
          ),
        ],
      ),
      SettingsSection(
        title: '字体与语言',
        child: <Widget>[
          SettingsTextField(
            label: '当前字体 (需重启)',
            value: _str(ui, 'CurrentFontFamily'),
            onChanged: (v) => _set('UiItem', 'CurrentFontFamily', v),
          ),
          SettingsNumberField(
            label: '字体大小',
            value: _int(ui, 'CurrentFontSize'),
            onChanged: (v) => _set('UiItem', 'CurrentFontSize', v),
          ),
          SettingsDropdown<String>(
            key: const ValueKey('settings-language'),
            label: '语言 (需重启)',
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
            label: '测速单个超时值',
            value: _int(_group('SpeedTestItem'), 'SpeedTestTimeout'),
            onChanged: (v) => _set('SpeedTestItem', 'SpeedTestTimeout', v),
          ),
          SettingsNumberField(
            label: '多线程测试时的并发数量',
            value: _int(_group('SpeedTestItem'), 'MixedConcurrencyCount'),
            onChanged: (v) => _set('SpeedTestItem', 'MixedConcurrencyCount', v),
          ),
          SettingsTextField(
            label: '测速文件地址',
            value: _str(_group('SpeedTestItem'), 'SpeedTestUrl'),
            width: 360,
            onChanged: (v) => _set('SpeedTestItem', 'SpeedTestUrl', v),
          ),
          SettingsTextField(
            label: '真连接测试地址',
            value: _str(_group('SpeedTestItem'), 'SpeedPingTestUrl'),
            width: 360,
            onChanged: (v) => _set('SpeedTestItem', 'SpeedPingTestUrl', v),
          ),
          SettingsTextField(
            label: 'UDP 测试地址',
            value: _str(_group('SpeedTestItem'), 'UdpTestTarget'),
            width: 320,
            onChanged: (v) => _set('SpeedTestItem', 'UdpTestTarget', v),
          ),
          SettingsTextField(
            label: '当前连接信息测试地址',
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
            label: '订阅转换网址 (可选)',
            value: _str(_group('ConstItem'), 'SubConvertUrl'),
            width: 360,
            onChanged: (v) => _set('ConstItem', 'SubConvertUrl', v),
          ),
          SettingsTextField(
            key: const ValueKey('settings-geo-source-url'),
            label: 'Geo 文件来源 (可选)',
            value: _str(_group('ConstItem'), 'GeoSourceUrl'),
            width: 360,
            onChanged: (v) => _set('ConstItem', 'GeoSourceUrl', v),
          ),
          SettingsTextField(
            key: const ValueKey('settings-srs-source-url'),
            label: 'sing-box ruleset 文件来源 (可选)',
            value: _str(_group('ConstItem'), 'SrsSourceUrl'),
            width: 360,
            onChanged: (v) => _set('ConstItem', 'SrsSourceUrl', v),
          ),
          SettingsTextField(
            key: const ValueKey('settings-route-rules-source-url'),
            label: '路由规则集来源 (可选)',
            value: _str(_group('ConstItem'), 'RouteRulesTemplateSourceUrl'),
            width: 360,
            onChanged: (v) =>
                _set('ConstItem', 'RouteRulesTemplateSourceUrl', v),
          ),
          SettingsCheckbox(
            key: const ValueKey('settings-enable-hwa'),
            label: '启用硬件加速 (需重启)',
            value: _bool(gui, 'EnableHWA'),
            onChanged: (v) => _set('GuiItem', 'EnableHWA', v),
          ),
          SettingsDropdown<String>(
            key: const ValueKey('settings-root-cert'),
            label: '根证书提供者',
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
            key: const ValueKey('fakeip-toggle'),
            label: '启用 FakeIP',
            value: _bool(_group('SimpleDNSItem'), 'FakeIP'),
            onChanged: (v) => _set('SimpleDNSItem', 'FakeIP', v),
          ),
          // Upstream `DNSSettingWindow` reveals GlobalFakeIp only while FakeIP
          // is checked; hiding it is a visible field linkage.
          if (_bool(_group('SimpleDNSItem'), 'FakeIP'))
            SettingsCheckbox(
              key: const ValueKey('global-fakeip-toggle'),
              label: '全局 FakeIP',
              value: _bool(_group('SimpleDNSItem'), 'GlobalFakeIp'),
              onChanged: (v) => _set('SimpleDNSItem', 'GlobalFakeIp', v),
            ),
          SettingsCheckbox(
            key: const ValueKey('happy-eyeballs-toggle'),
            label: '启用 Happy Eyeballs',
            value: _bool(_group('SimpleDNSItem'), 'EnableHappyEyeballs'),
            onChanged: (v) => _set('SimpleDNSItem', 'EnableHappyEyeballs', v),
          ),
          // Upstream `V2rayDnsService.FillSockoptDomainStrategy` only emits
          // the `happyEyeballs` block while the toggle is on, so the retained
          // parameter editors follow the same gate (values are kept while
          // hidden, mirroring the FakeIP/GlobalFakeIp linkage above).
          if (_bool(_group('SimpleDNSItem'), 'EnableHappyEyeballs')) ...[
            SettingsNumberField(
              key: const ValueKey('happy-try-delay'),
              label: '尝试延迟',
              value: _int(_group('HappyEyeballs4RayItem'), 'TryDelayMs'),
              onChanged: (v) => _set('HappyEyeballs4RayItem', 'TryDelayMs', v),
            ),
            SettingsCheckbox(
              key: const ValueKey('happy-prioritize-ipv6'),
              label: '优先 IPv6',
              value: _bool(_group('HappyEyeballs4RayItem'), 'PrioritizeIPv6'),
              onChanged: (v) =>
                  _set('HappyEyeballs4RayItem', 'PrioritizeIPv6', v),
            ),
            SettingsNumberField(
              key: const ValueKey('happy-interleave'),
              label: '交错',
              value: _int(_group('HappyEyeballs4RayItem'), 'Interleave'),
              onChanged: (v) => _set('HappyEyeballs4RayItem', 'Interleave', v),
            ),
            SettingsNumberField(
              key: const ValueKey('happy-max-concurrent'),
              label: '最大并发尝试',
              value: _int(_group('HappyEyeballs4RayItem'), 'MaxConcurrentTry'),
              onChanged: (v) =>
                  _set('HappyEyeballs4RayItem', 'MaxConcurrentTry', v),
            ),
          ],
        ],
      ),
      // ClashUIItem has no OptionSettingWindow control upstream; the monitor
      // Clash tabs now consume these fields (FIX-16C), so they stay editable.
      SettingsSection(
        title: '历史保留（原版 Clash UI 设置）',
        child: <Widget>[
          SettingsCheckbox(
            label: '启用 IPv6',
            value: _bool(_group('ClashUIItem'), 'EnableIPv6'),
            onChanged: (v) => _set('ClashUIItem', 'EnableIPv6', v),
          ),
          SettingsCheckbox(
            label: '合并 Mixin',
            value: _bool(_group('ClashUIItem'), 'EnableMixinContent'),
            onChanged: (v) => _set('ClashUIItem', 'EnableMixinContent', v),
          ),
          SettingsDropdown<int>(
            label: '代理排序',
            value: _int(_group('ClashUIItem'), 'ProxiesSorting'),
            items: _items(<(String, int)>[('延迟', 0), ('名称', 1)]),
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
          SettingsCheckbox(
            label: '连接自动刷新',
            value: _bool(_group('ClashUIItem'), 'ConnectionsAutoRefresh'),
            onChanged: (v) => _set('ClashUIItem', 'ConnectionsAutoRefresh', v),
          ),
          SettingsNumberField(
            label: '连接刷新间隔',
            value: _int(_group('ClashUIItem'), 'ConnectionsRefreshInterval'),
            onChanged: (v) =>
                _set('ClashUIItem', 'ConnectionsRefreshInterval', v),
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
        label: '系统代理类型',
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
        label: '请勿将代理服务器用于本地 (Intranet) 地址',
        value: _bool(proxy, 'NotProxyLocalAddress'),
        onChanged: (v) => _set('SystemProxyItem', 'NotProxyLocalAddress', v),
      ),
      SettingsTextField(
        label: '例外',
        value: _str(proxy, 'SystemProxyExceptions'),
        width: 320,
        onChanged: (v) => _set('SystemProxyItem', 'SystemProxyExceptions', v),
      ),
      SettingsTextField(
        label: '高级代理设置，协议选择 (可选)',
        value: _str(proxy, 'SystemProxyAdvancedProtocol'),
        onChanged: (v) =>
            _set('SystemProxyItem', 'SystemProxyAdvancedProtocol', v),
      ),
      SettingsTextField(
        label: '自定义 PAC 文件路径',
        value: _str(proxy, 'CustomSystemProxyPacPath'),
        width: 320,
        onChanged: (v) =>
            _set('SystemProxyItem', 'CustomSystemProxyPacPath', v),
      ),
      SettingsTextField(
        label: '自定义系统代理脚本文件路径',
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
        label: '启用 Tun',
        value: _bool(tun, 'EnableTun'),
        onChanged: (v) => _set('TunModeItem', 'EnableTun', v),
      ),
      SettingsCheckbox(
        label: '自动路由',
        value: _bool(tun, 'AutoRoute'),
        onChanged: (v) => _set('TunModeItem', 'AutoRoute', v),
      ),
      SettingsCheckbox(
        label: '严格路由',
        value: _bool(tun, 'StrictRoute'),
        onChanged: (v) => _set('TunModeItem', 'StrictRoute', v),
      ),
      SettingsDropdown<String>(
        label: '协议栈',
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
        label: 'ICMP 路由策略',
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
        label: '启用 IPv6',
        value: _bool(tun, 'EnableIPv6Address'),
        onChanged: (v) => _set('TunModeItem', 'EnableIPv6Address', v),
      ),
      SettingsCheckbox(
        label: '旧版 TUN 保护',
        value: _bool(tun, 'EnableLegacyProtect'),
        onChanged: (v) => _set('TunModeItem', 'EnableLegacyProtect', v),
      ),
      SettingsTextField(
        key: const ValueKey('tun-route-exclude'),
        label: '路由排除地址',
        value: _list(tun, 'RouteExcludeAddress').join(','),
        width: 320,
        onChanged: (v) =>
            _set('TunModeItem', 'RouteExcludeAddress', _splitList(v)),
      ),
      SettingsTextField(
        label: 'Ipv4 地址',
        value: _str(tun, 'IPv4Address'),
        onChanged: (v) => _set('TunModeItem', 'IPv4Address', v),
      ),
      SettingsTextField(
        label: 'Ipv6 地址',
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
          label: entry.label,
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

  // Frozen `OptionSettingWindow.xaml` CoreType tab row labels.
  static const List<({String label, int configType})> _coreTypeControls =
      <({String label, int configType})>[
        (label: 'VMess', configType: 1),
        (label: 'Custom Pre', configType: 2),
        (label: 'Shadowsocks', configType: 3),
        (label: 'Socks', configType: 4),
        (label: 'VLESS', configType: 5),
        (label: 'Trojan', configType: 6),
        (label: 'Hysteria2', configType: 7),
        (label: 'Wireguard', configType: 9),
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
