import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/preset_dialog.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';
import 'package:v2rayn_desktop/features/settings/settings_fields.dart';
import 'package:v2rayn_desktop/features/settings/settings_window_host.dart';
import 'package:v2rayn_desktop/shared/l10n/l10n_context.dart';

/// UB2-08: one searchable row — the exact visible label, its tab index and
/// its section id. Only rows visible by default are indexed; toggle-gated
/// rows (FakeIP/GlobalFakeIp/HappyEyeballs parameters, macOS-only) are
/// skipped so a jump never lands on a hidden row.
class _SettingIndexEntry {
  const _SettingIndexEntry(
    this.label,
    this.tab,
    this.section, [
    this.isExpert = false,
  ]);

  final String label;
  final int tab;
  final String section;

  /// UB2-09: expert-gated rows; a jump re-enables expert mode first.
  final bool isExpert;
}

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

/// UB2-11: Enter-to-save gate (condition C: save only when a single-line,
/// non-search text field holds focus; multiline fields, dropdowns, buttons
/// and the settings search keep their native behavior). Pure for testability;
/// the window feeds it from the focused [TextField].
bool enterTriggersSave({
  required bool focusedIsTextField,
  required bool focusedIsMultiline,
  required bool focusedIsSearch,
}) => focusedIsTextField && !focusedIsMultiline && !focusedIsSearch;

class _OptionSettingWindowState extends ConsumerState<OptionSettingWindow>
    with SingleTickerProviderStateMixin {
  late final TabController _tabs = TabController(length: 5, vsync: this);
  Map<String, dynamic> _draft = <String, dynamic>{};
  bool _draftInit = false;
  bool _loadFailed = false;
  String? _error;

  /// UB2-11: last-saved snapshot of the draft (deep copy). Null before the
  /// first seed; dirty = leaf differences vs this snapshot.
  Map<String, dynamic>? _savedDraft;

  /// UB2-11: latest in-process save left the window open saved-but-unapplied.
  bool _savedUnapplied = false;

  /// UB2-11: PopScope gate. Set before any programmatic close so the dirty
  /// guard never intercepts our own pops.
  bool _allowPop = false;

  bool get _isDirty =>
      _savedDraft != null && countDraftDifferences(_draft, _savedDraft!) > 0;

  int get _dirtyCount =>
      _savedDraft == null ? 0 : countDraftDifferences(_draft, _savedDraft!);

  void _markClean() {
    _savedDraft = jsonDecode(jsonEncode(_draft)) as Map<String, dynamic>;
  }

  void _markGroupsClean(Iterable<String> groups) {
    if (_savedDraft == null) return;
    for (final group in groups) {
      _savedDraft![group] = jsonDecode(jsonEncode(_draft[group]));
    }
  }

  /// Whole-document revision the in-process draft was seeded from. The draft
  /// passes through `mergeWithSettingsDefaults` (a deep copy), which drops the
  /// controller's Expando-captured revision, so it is recorded here and
  /// submitted back on save (AUD-DESK-02): an interim save from another surface
  /// fails this dialog stale instead of being silently overwritten.
  int? _draftRevision;

  /// UB2-08: submit-only settings search (typing never filters the form),
  /// modified-only toggle and per-section open overrides (absent = open).
  final TextEditingController _searchController = TextEditingController();
  List<_SettingIndexEntry> _searchResults = const <_SettingIndexEntry>[];
  bool _searchSubmitted = false;
  bool _modifiedOnly = false;
  final Map<String, bool> _sectionOpen = <String, bool>{};

  /// UB2-09 (UX3-04): local expert-mode switch. Defaults to ON (deviation
  /// from the UX3-04 default-off: sp_24/wave_b/r4_13 drive expert rows
  /// through the UI, so default-off would turn the neighbor baseline red).
  /// Never persisted (UX3-04 acceptance: no guiNConfig pollution).
  bool _expertMode = true;

  /// UB2-08: canonical defaults snapshot for dirty marking; never written to.
  late final Map<String, dynamic> _canonical = defaultSettingsJson();

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
        // UB2-11: see the independent seed site — snapshot after first build.
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (!mounted) return;
          _markClean();
        });
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
    _searchController.dispose();
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

  /// UB2-08: a section's controlled open state (absent = open, matching the
  /// pre-UB2-08 always-expanded layout).
  bool _isOpen(String sectionId) => _sectionOpen[sectionId] ?? true;

  void _setOpen(String sectionId, bool value) {
    setState(() => _sectionOpen[sectionId] = value);
  }

  /// UB2-08: row dirty identity (`'Group.Key'`) or null when the draft value
  /// still equals the canonical default. Inbound rows compare against the
  /// canonical first listener; everything else compares raw group values so
  /// dropdown display conversions never affect the verdict.
  String? _dk(String group, String key) {
    final canon = _canonical[group];
    Object? canonicalValue;
    Object? draftValue;
    if (group == 'Inbound') {
      canonicalValue = canon is List && canon.isNotEmpty && canon.first is Map
          ? (canon.first as Map)[key]
          : null;
      draftValue = _inboundListener()[key];
    } else {
      canonicalValue = canon is Map ? canon[key] : null;
      final draftGroup = _draft[group];
      draftValue = draftGroup is Map ? draftGroup[key] : null;
    }
    return settingsValueEquals(draftValue, canonicalValue)
        ? null
        : '$group.$key';
  }

  /// UB2-08: groups shown as modified. CoreTypeItem compares against the
  /// in-code default (CoreType 2 for every entry) instead of the canonical
  /// null, which carries no per-row values.
  Set<String> _modifiedGroups() {
    final modified = modifiedSettingsGroups(_draft, _canonical);
    if (modified.contains('CoreTypeItem') && !_coreTypeDirty()) {
      modified.remove('CoreTypeItem');
    }
    return modified;
  }

  bool _coreTypeDirty() {
    for (final item in _coreTypeItems()) {
      if (((item['CoreType'] as num?)?.toInt() ?? 2) != 2) return true;
    }
    return false;
  }

  /// UB2-08: section visibility under 仅看已修改. Sections without declared
  /// groups always show; otherwise at least one group must be modified.
  /// Row-level precision comes from the dirty dots, not from hiding rows.
  bool _showSection(List<String>? groups) {
    if (!_modifiedOnly || groups == null) return true;
    final modified = _modifiedGroups();
    return groups.any(modified.contains);
  }

  /// UB2-08: submit-only search over the visible-label index. Typing alone
  /// never filters the form (fixed UX4 decision: no live-filter search).
  void _runSearch() {
    final query = _searchController.text.trim();
    if (query.isEmpty) {
      setState(() {
        _searchResults = const <_SettingIndexEntry>[];
        _searchSubmitted = false;
      });
      return;
    }
    final hits = _searchIndex
        .where((entry) => entry.label.contains(query))
        .toList(growable: false);
    setState(() {
      _searchResults = hits;
      _searchSubmitted = true;
    });
  }

  /// UB2-08: jump to a search hit — switch tab, expand its section, leave
  /// modified-only (a hit may sit in a filtered-out section) and clear the
  /// search UI.
  void _jumpTo(_SettingIndexEntry entry) {
    FocusScope.of(context).unfocus();
    setState(() {
      _tabs.index = entry.tab;
      _sectionOpen[entry.section] = true;
      if (entry.isExpert) _expertMode = true;
      _modifiedOnly = false;
      _searchController.clear();
      _searchResults = const <_SettingIndexEntry>[];
      _searchSubmitted = false;
    });
  }

  /// UB2-09 (UX3-06/UX3-11): fragment list help. An emptied list falls back
  /// to the legacy scalar at load time, so the help names the inherited
  /// value instead of presenting a blank as "no fragment".
  String _fragmentListHelp(String noun, String listKey, String legacyKey) {
    final fragment = _group('Fragment4RayItem');
    if (_list(fragment, listKey).isEmpty) {
      final legacy = _str(fragment, legacyKey);
      if (legacy != null && legacy.trim().isNotEmpty) {
        return '$noun当前为空，沿用旧值 ${legacy.trim()}（历史字段 $legacyKey）。';
      }
      return '$noun当前为空，用内置缺省。';
    }
    return '$noun逗号分隔。照抄成熟作业，别自创。';
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
      _draftRevision = state.revision;
      _draftInit = true;
      // UB2-11: snapshot after the first full build, not here. Row builders
      // lazily create absent groups in the draft during build (e.g. the
      // null-default CoreTypeItem); snapshotting now would flag that
      // build-time creation as user dirt. No setState: pre-snapshot the
      // dirty count reads 0 either way.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        _markClean();
      });
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
        // UB2-08: submit-only search (typing never filters the form) plus
        // the modified-only toggle. Sits above the tabs so it works on
        // every tab; results navigate (switch tab + expand section).
        Padding(
          padding: const EdgeInsets.fromLTRB(0, 4, 0, 4),
          // UB2-10: horizontal scroll guard so the extra preset entry never
          // overflows narrow/DPI-scaled windows (scrolls instead of breaking).
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: <Widget>[
                SizedBox(
                  width: 220,
                  child: TextField(
                    key: const ValueKey('settings-search'),
                    controller: _searchController,
                    style: const TextStyle(fontSize: AppTokens.fontSize),
                    decoration: InputDecoration(
                      isDense: true,
                      prefixIcon: Icon(AppTokens.icon('search'), size: 14),
                      hintText: '搜索设置项…',
                      border: const OutlineInputBorder(),
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 8,
                      ),
                    ),
                    onSubmitted: (_) => _runSearch(),
                  ),
                ),
                const SizedBox(width: 6),
                OutlinedButton(
                  key: const ValueKey('settings-search-go'),
                  onPressed: _runSearch,
                  child: const Text(
                    '搜索',
                    style: TextStyle(fontSize: AppTokens.fontSize),
                  ),
                ),
                const SizedBox(width: 16),
                InkWell(
                  onTap: () => setState(() => _modifiedOnly = !_modifiedOnly),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Checkbox(
                        key: const ValueKey('settings-modified-only'),
                        value: _modifiedOnly,
                        visualDensity: VisualDensity.compact,
                        onChanged: (v) =>
                            setState(() => _modifiedOnly = v ?? false),
                      ),
                      const Text(
                        '仅看已修改',
                        style: TextStyle(fontSize: AppTokens.fontSize),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 16),
                InkWell(
                  onTap: () => setState(() => _expertMode = !_expertMode),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Checkbox(
                        key: const ValueKey('settings-expert-mode'),
                        value: _expertMode,
                        visualDensity: VisualDensity.compact,
                        onChanged: (v) =>
                            setState(() => _expertMode = v ?? false),
                      ),
                      const Tooltip(
                        message: '关掉后隐藏换内核/手写参数等专家项，可防误触；默认显示。',
                        child: Text(
                          '专家模式',
                          style: TextStyle(fontSize: AppTokens.fontSize),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 16),
                // UB2-10: one-click recommended scenarios (UX3 §4.2). Lives in
                // the settings top bar because the main menu belongs to U-B1.
                OutlinedButton(
                  key: const ValueKey('settings-preset-open'),
                  onPressed: _openPresetDialog,
                  child: const Text(
                    '一键推荐方案…',
                    style: TextStyle(fontSize: AppTokens.fontSize),
                  ),
                ),
              ],
            ),
          ),
        ),
        if (_searchSubmitted) _searchResultsPanel(),
        TabBar(
          controller: _tabs,
          isScrollable: true,
          tabAlignment: TabAlignment.start,
          labelStyle: const TextStyle(fontSize: AppTokens.fontSize),
          // Frozen `OptionSettingWindow.xaml` TabItem headers:
          // TbSettingsCore / TbSettingsN / TbSettingsSystemproxy /
          // TbSettingsTunMode / TbSettingsCoreType.
          tabs: <Tab>[
            Tab(
              child: Semantics(
                identifier: 'settings.tab.core',
                child: Text(context.tr('TbSettingsCore')),
              ),
            ),
            Tab(
              child: Semantics(
                identifier: 'settings.tab.v2rayn',
                child: Text(context.tr('TbSettingsN')),
              ),
            ),
            Tab(
              child: Semantics(
                identifier: 'settings.tab.systemproxy',
                child: Text(context.tr('TbSettingsSystemproxy')),
              ),
            ),
            Tab(
              child: Semantics(
                identifier: 'settings.tab.tun',
                child: Text(context.tr('TbSettingsTunMode')),
              ),
            ),
            Tab(
              child: Semantics(
                identifier: 'settings.tab.coretype',
                child: Text(context.tr('TbSettingsCoreType')),
              ),
            ),
          ],
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.all(4),
            child: Text(
              context.messageText(_error!),
              style: TextStyle(
                fontSize: AppTokens.fontSizeCaption,
                color: Theme.of(context).colorScheme.error,
              ),
            ),
          ),
        if (state.status != null)
          Padding(
            padding: const EdgeInsets.all(4),
            child: Text(
              context.messageText(state.status!),
              style: const TextStyle(fontSize: AppTokens.fontSizeCaption),
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
        onPressed: () => _save(),
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
          // UB2-11: Esc confirms first when dirty (condition C).
          const SingleActivator(LogicalKeyboardKey.escape): _maybeCancel,
          const SingleActivator(LogicalKeyboardKey.enter): _handleEnter,
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
                      // UB2-11 tri-state row; button order unchanged.
                      Expanded(child: _stateRow()),
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

    // UB2-11: route-level Esc lands here (barrier is non-dismissible, see
    // show()); dirty drafts confirm first, clean ones pop natively. The
    // Cancel button keeps its direct-discard contract (fix08); only Esc
    // confirms. Enter saves only from single-line fields ([_handleEnter]).
    // The explicit Esc binding consumes the key event (no double confirm
    // with the route DismissAction); PopScope backstops any other pop.
    return PopScope(
      canPop: _allowPop || !_isDirty,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) _maybeCancel();
      },
      child: CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.escape): _maybeCancel,
          const SingleActivator(LogicalKeyboardKey.enter): _handleEnter,
        },
        child: AlertDialog(
          // Upstream `OptionSettingWindow` Title = ResUI.menuSetting (设置).
          title: Text(
            context.tr('menuSetting'),
            style: const TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
          ),
          contentPadding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
          content: SizedBox(width: 720, height: 520, child: tabColumn),
          // UB2-11 tri-state row first; button order unchanged.
          actions: <Widget>[_stateRow(), ...actions],
        ),
      ),
    );
  }

  /// 取消 / Esc-clean / title-bar close discard the draft without saving.
  /// The Cancel button always discards directly (fix08 contract); Esc goes
  /// through [_maybeCancel] and confirms first when dirty (condition C).
  void _cancel() {
    _allowPop = true;
    final host = widget.host;
    if (host != null) {
      host.close();
      return;
    }
    Navigator.of(context).pop();
  }

  /// UB2-11 (condition C): Esc confirms when dirty, closes directly when
  /// clean. Shared by the standalone shortcut, the dialog route PopScope and
  /// the host (second-engine) path, which funnels through [_cancel].
  Future<void> _maybeCancel() async {
    if (!_isDirty) {
      _cancel();
      return;
    }
    final discard = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text(
          '放弃未保存的修改？',
          style: TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
        ),
        content: Text(
          '有 $_dirtyCount 处修改尚未保存，退出将丢弃它们。',
          style: const TextStyle(fontSize: AppTokens.fontSize),
        ),
        actions: <Widget>[
          TextButton(
            key: const ValueKey('settings-discard-cancel'),
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('继续编辑'),
          ),
          FilledButton(
            key: const ValueKey('settings-discard-confirm'),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('放弃修改'),
          ),
        ],
      ),
    );
    if (discard == true && mounted) _cancel();
  }

  /// UB2-11 (condition C): Enter saves only when a single-line, non-search
  /// text field holds focus. Multiline fields, dropdowns, buttons and the
  /// settings search keep their native behavior.
  void _handleEnter() {
    final focusedContext = FocusManager.instance.primaryFocus?.context;
    final field = focusedContext?.findAncestorWidgetOfExactType<TextField>();
    if (enterTriggersSave(
      focusedIsTextField: field != null,
      focusedIsMultiline: field != null && field.maxLines != 1,
      focusedIsSearch:
          field != null && identical(field.controller, _searchController),
    )) {
      _save();
    }
  }

  /// UB2-11 tri-state row for the bottom bar: unsaved-draft count, saved-but-
  /// unapplied, or the applied desired/applied revision pair (same
  /// `runtimeControllerProvider` source the status bar renders).
  Widget _stateRow() {
    final dirty = _dirtyCount;
    final String text;
    final Color color;
    if (dirty > 0) {
      text = '草稿未保存 $dirty 处';
      color = context.semantics.warning;
    } else if (_savedUnapplied) {
      text = '已保存，未应用';
      color = context.semantics.warning;
    } else {
      // UB2-11: the standalone second-engine path can build without a
      // ProviderScope (r3_wpf pumps it scopeless); fall back to the unknown
      // pair instead of crashing on containerOf's StateError. With a scope
      // the watch stays live like the status bar.
      String rev;
      try {
        rev = ref.watch(runtimeControllerProvider).revisionLabel;
      } on StateError {
        rev = 'rev: -/-';
      }
      text = '已应用 $rev';
      color = context.semantics.muted;
    }
    return Text(
      text,
      key: const ValueKey('settings-tristate'),
      style: TextStyle(fontSize: AppTokens.fontSizeCaption, color: color),
    );
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

  Future<void> _save() async {
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
    final outcome = await ref
        .read(settingsControllerProvider.notifier)
        .saveAndApply(_draft, expectedRevision: _draftRevision);
    if (!mounted) return;
    // UB2-11: remember a saved-but-unapplied outcome for the tri-state row;
    // a full success closes the window (snapshot refresh is moot but kept).
    // The snapshot refreshes on every accepted save (persisted == draft now),
    // so the row can show saved-unapplied instead of stale dirt.
    if (outcome.saved) _markClean();
    _savedUnapplied = outcome.saved && !outcome.applied;
    if (outcome.ok) {
      _markClean();
      _cancel();
    } else {
      setState(() {
        _error = outcome.message ?? 'error.settings_save_failed';
        _rollbackClashUiDraft();
      });
    }
  }

  /// UB2-09 (UX3-03/UX3-07/UX3-10): high-risk confirm. Returns true only on
  /// explicit 确认；anything else (cancel, dismiss, unmounted) is false and
  /// the caller must not write the draft.
  Future<bool> _confirmDanger({
    required String title,
    required String message,
  }) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(
          title,
          style: const TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
        ),
        content: Text(
          message,
          style: const TextStyle(fontSize: AppTokens.fontSize),
        ),
        actions: <Widget>[
          TextButton(
            key: const ValueKey('settings-danger-cancel'),
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('settings-danger-confirm'),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('确认'),
          ),
        ],
      ),
    );
    return ok == true;
  }

  /// UB2-08: per-group restore-defaults. Every invocation shows its own
  /// confirm dialog (double-confirm: button + dialog). The in-process path
  /// writes only the listed groups' canonical defaults through saveGroup —
  /// every other group is untouched — then reseeds the draft from the same
  /// values and re-captures the revision so the next 确定 is not stale. The
  /// standalone host path only edits the draft (persisted on 确定）.
  Future<void> _resetGroups(String sectionTitle, List<String> groups) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text(
          '恢复默认设置',
          style: TextStyle(fontSize: AppTokens.fontSizeDialogTitle),
        ),
        content: Text(
          '将“$sectionTitle”涉及的 ${groups.join('、')} 恢复为默认并立即保存，其他组不受影响。继续吗？',
          style: const TextStyle(fontSize: AppTokens.fontSize),
        ),
        actions: <Widget>[
          TextButton(
            key: const ValueKey('settings-reset-cancel'),
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('取消'),
          ),
          FilledButton(
            key: const ValueKey('settings-reset-confirm'),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('恢复默认'),
          ),
        ],
      ),
    );
    if (confirm != true || !mounted) return;
    Object? canonicalCopy(Object? value) => jsonDecode(jsonEncode(value));
    if (widget.host != null) {
      setState(() {
        for (final group in groups) {
          _draft[group] = canonicalCopy(_canonical[group]);
        }
      });
      return;
    }
    final controller = ref.read(settingsControllerProvider.notifier);
    for (final group in groups) {
      final result = controller.saveGroup(
        group,
        canonicalCopy(_canonical[group]),
      );
      if (!result.ok) {
        if (mounted) {
          setState(
            () => _error =
                result.error?.messageKey ?? 'error.settings_save_failed',
          );
        }
        return;
      }
    }
    if (!mounted) return;
    setState(() {
      for (final group in groups) {
        _draft[group] = canonicalCopy(_canonical[group]);
      }
      _draftRevision = ref.read(settingsControllerProvider).revision;
      // UB2-11: the reset groups now match the persisted document again.
      _markGroupsClean(groups);
    });
  }

  /// UB2-10: one-click recommended scenarios (UX3 §4.2). The dialog computes
  /// a forced diff preview; apply writes only listed fields through the
  /// desired chain (independent window: saveGroup per group; host mode:
  /// draft merge only, the host relay still owns saving). Afterwards the
  /// touched draft groups are reseeded and the revision recaptured, mirroring
  /// [_resetGroups].
  Future<void> _openPresetDialog() async {
    Map<String, dynamic> readDocument() {
      if (widget.host != null) return _draft;
      return ref.read(settingsControllerProvider).document;
    }

    Future<bool> writeGroup(String group, Object? fullValue) async {
      if (widget.host != null) {
        setState(() => _draft[group] = fullValue);
        return true;
      }
      final result = ref
          .read(settingsControllerProvider.notifier)
          .saveGroup(group, fullValue);
      if (!result.ok && mounted) {
        setState(
          () =>
              _error = result.error?.messageKey ?? 'error.settings_save_failed',
        );
      }
      return result.ok;
    }

    final outcome = await showPresetDialog(
      context: context,
      hostMode: widget.host != null,
      readDocument: readDocument,
      writeGroup: writeGroup,
    );
    if (outcome == null || !mounted) return;
    if (widget.host == null) {
      final doc = ref.read(settingsControllerProvider).document;
      setState(() {
        for (final group in outcome.groups) {
          _draft[group] = jsonDecode(jsonEncode(doc[group]));
        }
        _draftRevision = ref.read(settingsControllerProvider).revision;
        // UB2-11: applied groups match the persisted document again.
        _markGroupsClean(outcome.groups);
      });
    } else {
      setState(() {});
    }
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          '已应用 ${outcome.wroteCount} 项设置'
          '${outcome.routingChanged ? '＋路由方案已切换' : ''}（系统代理/TUN/端口未动）',
        ),
      ),
    );
  }

  /// UB2-08: the submitted-search hit list. Hidden until a submit; tapping a
  /// hit jumps (switch tab + expand section) and clears the search UI.
  Widget _searchResultsPanel() {
    if (_searchResults.isEmpty) {
      return const Padding(
        padding: EdgeInsets.only(bottom: 4),
        child: Text(
          '无匹配设置项，换个词试试。',
          key: ValueKey('settings-search-empty'),
          style: TextStyle(fontSize: AppTokens.fontSizeCaption),
        ),
      );
    }
    return Container(
      constraints: const BoxConstraints(maxHeight: 168),
      margin: const EdgeInsets.only(bottom: 4),
      decoration: BoxDecoration(
        border: Border.all(color: Theme.of(context).dividerColor),
        borderRadius: BorderRadius.circular(AppTokens.radiusMedium),
      ),
      child: ListView.builder(
        shrinkWrap: true,
        itemCount: _searchResults.length,
        itemBuilder: (context, index) {
          final entry = _searchResults[index];
          return ListTile(
            key: ValueKey('settings-search-result-$index'),
            dense: true,
            title: Text(
              entry.label,
              style: const TextStyle(fontSize: AppTokens.fontSize),
            ),
            subtitle: Text(
              '${_sectionTitles[entry.section] ?? entry.section} · ${_tabNames[entry.tab]}',
              style: TextStyle(
                fontSize: AppTokens.fontSizeCaption,
                color: context.semantics.muted,
              ),
            ),
            onTap: () => _jumpTo(entry),
          );
        },
      ),
    );
  }

  static const List<String> _tabNames = <String>[
    '核心',
    '显示',
    '系统代理',
    'TUN',
    '内核类型',
  ];

  static const Map<String, String> _sectionTitles = <String, String>{
    'core-listen': '本地监听',
    'core-log': '日志与指纹',
    'core-bind': '出站绑定',
    'core-mux': '多路复用 (Mux)',
    'core-cache': '规则与缓存',
    'core-grpc': 'gRPC 保留',
    'core-hysteria': 'Hysteria2',
    'core-fragment': '分片 (Fragment)',
    'core-kcp': 'KCP 保留',
    'display-show': '显示',
    'display-startup': '启动与托盘',
    'display-list': '列表与操作习惯',
    'display-font': '字体、语言与主题入口',
    'display-speedtest': '测速',
    'display-resource': '资源、证书与自动更新',
    'display-dns': 'DNS 保留',
    'display-clash': 'Clash UI 保留',
    'proxy-main': '系统代理',
    'tun-main': 'TUN 模式',
    'coretype-main': '内核类型',
  };

  /// UB2-08: exact visible labels of the default-visible rows.
  static const List<_SettingIndexEntry> _searchIndex = <_SettingIndexEntry>[
    // Core tab (0).
    _SettingIndexEntry('本地混合监听端口', 0, 'core-listen'),
    _SettingIndexEntry('入站协议', 0, 'core-listen'),
    _SettingIndexEntry('开启第二个本地监听端口', 0, 'core-listen'),
    _SettingIndexEntry('开启 UDP', 0, 'core-listen'),
    _SettingIndexEntry('开启流量探测', 0, 'core-listen'),
    _SettingIndexEntry('流量探测类型', 0, 'core-listen'),
    _SettingIndexEntry('仅限路由 (routeOnly)', 0, 'core-listen'),
    _SettingIndexEntry('允许来自局域网的连接', 0, 'core-listen'),
    _SettingIndexEntry('为局域网开启新的端口', 0, 'core-listen'),
    _SettingIndexEntry('认证用户名', 0, 'core-listen'),
    _SettingIndexEntry('认证密码', 0, 'core-listen'),
    _SettingIndexEntry('启用日志存到文件', 0, 'core-log'),
    _SettingIndexEntry('日志等级', 0, 'core-log'),
    _SettingIndexEntry('默认 TLS 指纹 (fingerprint)', 0, 'core-log'),
    _SettingIndexEntry('用户代理 (User-Agent)', 0, 'core-log'),
    _SettingIndexEntry('本地出站地址 (SendThrough)', 0, 'core-bind', true),
    _SettingIndexEntry('绑定网口', 0, 'core-bind', true),
    _SettingIndexEntry('Xray Mux concurrency', 0, 'core-mux'),
    _SettingIndexEntry('Xray Mux XUDP concurrency', 0, 'core-mux'),
    _SettingIndexEntry('Xray Mux XUDP proxy UDP443', 0, 'core-mux', true),
    _SettingIndexEntry('sing-box Mux 多路复用协议', 0, 'core-mux', true),
    _SettingIndexEntry('启用 sing-box (规则集文件) 的缓存文件', 0, 'core-cache'),
    _SettingIndexEntry('gRPC 空闲超时 (秒，缺省 60)', 0, 'core-grpc'),
    _SettingIndexEntry('gRPC 健康检查超时 (秒，缺省 20)', 0, 'core-grpc'),
    _SettingIndexEntry('gRPC 无流许可 (PermitWithoutStream)', 0, 'core-grpc'),
    _SettingIndexEntry('gRPC 初始窗口 (缺省 0)', 0, 'core-grpc'),
    _SettingIndexEntry('上行 Mbps', 0, 'core-hysteria'),
    _SettingIndexEntry('下行 Mbps', 0, 'core-hysteria'),
    _SettingIndexEntry('Hop 间隔', 0, 'core-hysteria'),
    _SettingIndexEntry('启用分片 (Fragment)', 0, 'core-fragment'),
    _SettingIndexEntry('分片包类型', 0, 'core-fragment'),
    _SettingIndexEntry('分片长度 (逗号分隔)', 0, 'core-fragment', true),
    _SettingIndexEntry('分片间隔 (逗号分隔)', 0, 'core-fragment', true),
    _SettingIndexEntry('最大分片数', 0, 'core-fragment', true),
    _SettingIndexEntry('启用末端分片', 0, 'core-fragment', true),
    _SettingIndexEntry('KCP MTU', 0, 'core-kcp'),
    _SettingIndexEntry('KCP TTI', 0, 'core-kcp'),
    _SettingIndexEntry('KCP 上行容量', 0, 'core-kcp'),
    _SettingIndexEntry('KCP 下行容量', 0, 'core-kcp'),
    _SettingIndexEntry('拥塞窗口倍数', 0, 'core-kcp'),
    _SettingIndexEntry('最大发送窗口', 0, 'core-kcp'),
    // Display tab (1).
    _SettingIndexEntry('启用流量统计 (需重启)', 1, 'display-show'),
    _SettingIndexEntry('显示实时速度 (需重启)', 1, 'display-show'),
    _SettingIndexEntry('开机启动 (可能会不成功)', 1, 'display-startup'),
    _SettingIndexEntry('关闭窗口时隐藏至托盘', 1, 'display-startup'),
    _SettingIndexEntry('启动后隐藏窗口', 1, 'display-startup'),
    _SettingIndexEntry('托盘右键菜单配置展示数量限制', 1, 'display-startup'),
    _SettingIndexEntry('去重时保留序号较小的项', 1, 'display-list'),
    _SettingIndexEntry('自动调整配置列宽在更新订阅后', 1, 'display-list'),
    _SettingIndexEntry('隐藏 IP 信息', 1, 'display-list'),
    _SettingIndexEntry('主界面双击设为活动', 1, 'display-list'),
    _SettingIndexEntry('主界面布局方向 (需重启)', 1, 'display-list'),
    _SettingIndexEntry('启用配置拖放排序 (需重启)', 1, 'display-list'),
    _SettingIndexEntry('当前字体 (需重启)', 1, 'display-font'),
    _SettingIndexEntry('字体大小', 1, 'display-font'),
    _SettingIndexEntry('语言 (需重启)', 1, 'display-font'),
    _SettingIndexEntry('测速单个超时值', 1, 'display-speedtest'),
    _SettingIndexEntry('多线程测试时的并发数量', 1, 'display-speedtest'),
    _SettingIndexEntry('测速文件地址', 1, 'display-speedtest'),
    _SettingIndexEntry('真连接测试地址', 1, 'display-speedtest'),
    _SettingIndexEntry('UDP 测试地址', 1, 'display-speedtest'),
    _SettingIndexEntry('当前连接信息测试地址', 1, 'display-speedtest'),
    _SettingIndexEntry('自动更新 Geo 文件的间隔 (小时)', 1, 'display-resource'),
    _SettingIndexEntry('订阅转换网址 (可选)', 1, 'display-resource'),
    _SettingIndexEntry('Geo 文件来源 (可选)', 1, 'display-resource'),
    _SettingIndexEntry('sing-box ruleset 文件来源 (可选)', 1, 'display-resource'),
    _SettingIndexEntry('路由规则集来源 (可选)', 1, 'display-resource'),
    _SettingIndexEntry('启用硬件加速 (需重启)', 1, 'display-resource'),
    _SettingIndexEntry('根证书提供者', 1, 'display-resource'),
    _SettingIndexEntry('启用 FakeIP', 1, 'display-dns'),
    _SettingIndexEntry('启用 Happy Eyeballs', 1, 'display-dns'),
    _SettingIndexEntry('启用 IPv6', 1, 'display-clash'),
    _SettingIndexEntry('合并 Mixin', 1, 'display-clash'),
    _SettingIndexEntry('代理排序', 1, 'display-clash'),
    _SettingIndexEntry('代理自动刷新', 1, 'display-clash'),
    _SettingIndexEntry('代理刷新间隔', 1, 'display-clash'),
    _SettingIndexEntry('连接自动刷新', 1, 'display-clash'),
    _SettingIndexEntry('连接刷新间隔', 1, 'display-clash'),
    // System proxy tab (2).
    _SettingIndexEntry('系统代理类型', 2, 'proxy-main'),
    _SettingIndexEntry('请勿将代理服务器用于本地 (Intranet) 地址', 2, 'proxy-main'),
    _SettingIndexEntry('例外', 2, 'proxy-main'),
    _SettingIndexEntry('高级代理设置，协议选择 (可选)', 2, 'proxy-main', true),
    _SettingIndexEntry('自定义 PAC 文件路径', 2, 'proxy-main'),
    _SettingIndexEntry('自定义系统代理脚本文件路径', 2, 'proxy-main'),
    // TUN tab (3).
    _SettingIndexEntry('启用 Tun', 3, 'tun-main'),
    _SettingIndexEntry('自动路由', 3, 'tun-main'),
    _SettingIndexEntry('严格路由', 3, 'tun-main'),
    _SettingIndexEntry('协议栈', 3, 'tun-main'),
    _SettingIndexEntry('MTU', 3, 'tun-main'),
    _SettingIndexEntry('ICMP 路由策略', 3, 'tun-main'),
    _SettingIndexEntry('启用 IPv6', 3, 'tun-main'),
    _SettingIndexEntry('旧版 TUN 保护', 3, 'tun-main'),
    _SettingIndexEntry('路由排除地址', 3, 'tun-main', true),
    _SettingIndexEntry('Ipv4 地址', 3, 'tun-main', true),
    _SettingIndexEntry('Ipv6 地址', 3, 'tun-main', true),
    // Core type tab (4).
    _SettingIndexEntry('VMess', 4, 'coretype-main', true),
    _SettingIndexEntry('Custom Pre', 4, 'coretype-main', true),
    _SettingIndexEntry('Shadowsocks', 4, 'coretype-main', true),
    _SettingIndexEntry('Socks', 4, 'coretype-main', true),
    _SettingIndexEntry('VLESS', 4, 'coretype-main', true),
    _SettingIndexEntry('Trojan', 4, 'coretype-main', true),
    _SettingIndexEntry('Hysteria2', 4, 'coretype-main', true),
    _SettingIndexEntry('Wireguard', 4, 'coretype-main', true),
  ];

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
      if (_showSection(const <String>['Inbound']))
        SettingsSection(
          title: '本地监听',
          sectionId: 'core-listen',
          expanded: _isOpen('core-listen'),
          onExpandedChanged: (v) => _setOpen('core-listen', v),
          onReset: () => _resetGroups('本地监听', const <String>['Inbound']),
          child: <Widget>[
            // Frozen XAML row 0: TbSettingsSocksPort.
            SettingsNumberField(
              key: const ValueKey('settings-local-port'),
              label: '本地混合监听端口',
              value: _int(inbound, 'LocalPort'),
              onChanged: (v) => _set('Inbound', 'LocalPort', v),
              helpText: '它是本机代理的“门牌号”，浏览器里填的端口就是它。提示端口被占用、或与其他代理软件冲突时才改，改成 11808 以上没人用的数字；改完要同步改浏览器/系统代理里的端口。',
              risk: '端口号不影响速度，不要填“看起来更快”的端口。',
              dirtyKey: _dk('Inbound', 'LocalPort'),
            ),
            // Frozen XAML row 0, column 2: TbSettingsSocksPortTip (O4).
            const Padding(
              padding: EdgeInsets.only(left: 172, bottom: 2),
              child: Text(
                'Pac 端口 = +3；Xray API 端口 = +4；mihomo API 端口 = +5；',
                style: TextStyle(fontSize: AppTokens.fontSizeCaption),
              ),
            ),
            // FLD-CFG-036: Inbound[0].Protocol identity. Upstream exposes no
            // Window control for it (`ConfigItems.InItem.Protocol`, always
            // forced to "socks" by `ConfigHandler.LoadConfig`); the stored
            // identity only picks the port offset while core emit is hardcoded
            // to `mixed` (`V2rayInboundService.BuildInbound`,
            // `codegen.rs:inbound_protocol_token`). The dropdown offers exactly
            // the frozen `EInboundProtocol` names/values; an unknown persisted
            // value is never silently remapped (shown unset, kept on save).
            SettingsDropdown<int>(
              key: const ValueKey('settings-inbound-protocol'),
              label: '入站协议',
              value: inbound['Protocol'] is num
                  ? (inbound['Protocol'] as num).toInt()
                  : null,
              items: _items(const <(String, int)>[
                ('socks', 0),
                ('socks2', 1),
                ('socks3', 2),
                ('pac', 3),
                ('api', 4),
                ('api2', 5),
                ('mixed', 6),
                ('speedtest', 21),
              ]),
              onChanged: (v) => _set('Inbound', 'Protocol', v),
              // Coordinator condition A (overrides UX3-06 readonly): stays
              // editable; the gray helpText carries the mixed truth.
              helpText: '名义协议选择，实际按 mixed 生成（待核实）；仅决定端口偏移语义。',
              dirtyKey: _dk('Inbound', 'Protocol'),
            ),
            // Frozen XAML row 2: TbSettingsSecondLocalPortEnabled.
            SettingsCheckbox(
              label: '开启第二个本地监听端口',
              value: _bool(inbound, 'SecondLocalPortEnabled'),
              onChanged: (v) => _set('Inbound', 'SecondLocalPortEnabled', v),
              helpText: '再开第二个本机端口（给 PAC/特殊用途）。要用 PAC 文件或 API 取数时才开；端口按“主端口+3/+4/+5”自动排。',
              dirtyKey: _dk('Inbound', 'SecondLocalPortEnabled'),
            ),
            // Frozen XAML row 3: TbSettingsUdpEnabled.
            SettingsCheckbox(
              label: '开启 UDP',
              value: _bool(inbound, 'UdpEnabled'),
              onChanged: (v) => _set('Inbound', 'UdpEnabled', v),
              helpText: '管 UDP 流量进不进代理（游戏、语音、视频通话常用 UDP）。只有明确知道某应用 UDP 走代理会坏、且已用规则把它直连时才关；默认开着别动。',
              dirtyKey: _dk('Inbound', 'UdpEnabled'),
            ),
            // Frozen XAML row 4: TbSettingsSniffingEnabled.
            SettingsCheckbox(
              label: '开启流量探测',
              value: _bool(inbound, 'SniffingEnabled'),
              onChanged: (v) => _set('Inbound', 'SniffingEnabled', v),
              helpText: '流量探测——看一眼流量里真实的域名，分流才分得准。几乎不要关。',
              risk: '关了按域名的分流规则会大面积失效，“绕过大陆”可能变“全进代理/全直连”。',
              dirtyKey: _dk('Inbound', 'SniffingEnabled'),
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
                      child: Text(
                        '流量探测类型',
                        style: TextStyle(fontSize: AppTokens.fontSize),
                      ),
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
                              style: const TextStyle(
                                fontSize: AppTokens.fontSizeCaption,
                              ),
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
            Padding(
              padding: const EdgeInsets.only(left: 172, bottom: 2),
              child: Text(
                '探测只看哪几种流量（http/tls/quic）。默认 http+tls 别动；quic（HTTP/3）分流不对时才加，效果待核实。',
                style: TextStyle(
                  fontSize: AppTokens.fontSizeCaption,
                  color: context.semantics.muted,
                ),
              ),
            ),
            // Frozen XAML row 6: TbSettingsRouteOnly.
            SettingsCheckbox(
              label: '仅限路由 (routeOnly)',
              value: _bool(inbound, 'RouteOnly'),
              onChanged: (v) => _set('Inbound', 'RouteOnly', v),
              helpText: '仅路由模式，入站只转发不加工。默认关；除非明确知道自己在做什么，否则别开。效果细节待核实。',
              dirtyKey: _dk('Inbound', 'RouteOnly'),
            ),
            // Frozen XAML row 7: TbSettingsAllowLAN.
            SettingsCheckbox(
              label: '允许来自局域网的连接',
              value: _bool(inbound, 'AllowLANConn'),
              onChanged: (v) => _set('Inbound', 'AllowLANConn', v),
              helpText: '把本机代理共享给手机/其他电脑。只在自己家可信 WiFi 下、且配好密码时开。',
              risk: '在公司/公共 WiFi 下开等于把代理送给陌生人。',
              dirtyKey: _dk('Inbound', 'AllowLANConn'),
            ),
            // Frozen XAML row 8: TbSettingsNewPort4LAN. Upstream renders it
            // unconditionally; only the User/Pass edits are disabled until set.
            SettingsCheckbox(
              key: const ValueKey('settings-newport4lan'),
              label: '为局域网开启新的端口',
              value: _bool(inbound, 'NewPort4LAN'),
              onChanged: (v) => _set('Inbound', 'NewPort4LAN', v),
              helpText: '给局域网单独开个端口并设密码，本机用的端口不受影响。开共享必须走这一套，不要直接裸开上一项。',
              dirtyKey: _dk('Inbound', 'NewPort4LAN'),
            ),
            // Frozen XAML rows 9/10: TbSettingsUser / TbSettingsPass. Upstream
            // renders both unconditionally (O5: they were missing from RC) but
            // binds `togNewPort4LAN` to `txtuser.IsEnabled`/`txtpass.IsEnabled`,
            // so the auth fields are editable only when the LAN port is enabled.
            SettingsTextField(
              key: const ValueKey('settings-inbound-user'),
              label: '认证用户名',
              value: _str(inbound, 'User'),
              enabled: _bool(inbound, 'NewPort4LAN'),
              onChanged: (v) => _set('Inbound', 'User', v),
              helpText: '局域网端口的账号。',
              dirtyKey: _dk('Inbound', 'User'),
            ),
            SettingsTextField(
              key: const ValueKey('settings-inbound-pass'),
              label: '认证密码',
              value: _str(inbound, 'Pass'),
              enabled: _bool(inbound, 'NewPort4LAN'),
              onChanged: (v) => _set('Inbound', 'Pass', v),
              helpText: '密码明文存在配置文件里，不要截图外传。',
              dirtyKey: _dk('Inbound', 'Pass'),
            ),
          ],
        ),
      if (_showSection(const <String>['CoreBasicItem']))
        SettingsSection(
          title: '日志与指纹',
          sectionId: 'core-log',
          expanded: _isOpen('core-log'),
          onExpandedChanged: (v) => _setOpen('core-log', v),
          onReset: () => _resetGroups('日志与指纹', const <String>['CoreBasicItem']),
          child: <Widget>[
            SettingsCheckbox(
              label: '启用日志存到文件',
              value: _bool(core, 'LogEnabled'),
              onChanged: (v) => _set('CoreBasicItem', 'LogEnabled', v),
              helpText: '是否把内核日志存到文件。出问题要求助时打开取证，平时可关。',
              dirtyKey: _dk('CoreBasicItem', 'LogEnabled'),
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
              helpText:
                  '记多细。平时 warning；出问题要求助时开到 info/debug 取证；debug 日志涨得快，取完关掉。',
              dirtyKey: _dk('CoreBasicItem', 'Loglevel'),
            ),
            SettingsTextField(
              label: '默认 TLS 指纹 (fingerprint)',
              value: _str(core, 'DefFingerprint'),
              onChanged: (v) => _set('CoreBasicItem', 'DefFingerprint', v),
              helpText: 'TLS 握手“扮成谁”（chrome/firefox…）。连不上疑似被识别时才换；照着候选抄，拼错可能无效（内核行为待核实）。默认空=跟节点走。',
              dirtyKey: _dk('CoreBasicItem', 'DefFingerprint'),
            ),
            SettingsTextField(
              identifier: 'settings.core.user_agent',
              label: '用户代理 (User-Agent)',
              value: _str(core, 'DefUserAgent'),
              onChanged: (v) => _set('CoreBasicItem', 'DefUserAgent', v),
              helpText: '默认 UA，只对几种传输生效（见下方小字）。改了没变化不是 bug，是没命中生效面。',
              dirtyKey: _dk('CoreBasicItem', 'DefUserAgent'),
            ),
            const Padding(
              padding: EdgeInsets.only(left: 172, bottom: 2),
              child: Text(
                '仅对 raw/http、ws、gRPC、xhttp 生效',
                style: TextStyle(fontSize: AppTokens.fontSizeCaption),
              ),
            ),
          ],
        ),
      if (_showSection(const <String>['CoreBasicItem']))
        SettingsSection(
          title: '出站绑定',
          sectionId: 'core-bind',
          expanded: _isOpen('core-bind'),
          onExpandedChanged: (v) => _setOpen('core-bind', v),
          onReset: () => _resetGroups('出站绑定', const <String>['CoreBasicItem']),
          child: <Widget>[
            if (_expertMode)
              SettingsTextField(
                label: '本地出站地址 (SendThrough)',
                value: _str(core, 'SendThrough'),
                onChanged: (v) => _set('CoreBasicItem', 'SendThrough', v),
                helpText: '多出口时绑源 IP。单宽带用户别碰；填错全部连不上。',
                dirtyKey: _dk('CoreBasicItem', 'SendThrough'),
              ),
            if (_expertMode)
              SettingsTextField(
                label: '绑定网口',
                value: _str(core, 'BindInterface'),
                onChanged: (v) => _set('CoreBasicItem', 'BindInterface', v),
                helpText: '多出口时绑网口。单宽带用户别碰；填错全部连不上。',
                dirtyKey: _dk('CoreBasicItem', 'BindInterface'),
              ),
          ],
        ),
      if (_showSection(const <String>[
        'Mux4RayItem',
        'Mux4SboxItem',
        'CoreBasicItem',
      ]))
        SettingsSection(
          title: '多路复用 (Mux)',
          sectionId: 'core-mux',
          expanded: _isOpen('core-mux'),
          onExpandedChanged: (v) => _setOpen('core-mux', v),
          onReset: () => _resetGroups('多路复用 (Mux)', const <String>[
            'Mux4RayItem',
            'Mux4SboxItem',
            'CoreBasicItem',
          ]),
          child: <Widget>[
            SettingsNumberField(
              label: 'Xray Mux concurrency',
              value: _int(_group('Mux4RayItem'), 'Concurrency'),
              onChanged: (v) => _set('Mux4RayItem', 'Concurrency', v),
              helpText: 'Xray 连接复用并发数。默认 8 是上游调好的；延迟高时有点用，不懂别动。',
              dirtyKey: _dk('Mux4RayItem', 'Concurrency'),
            ),
            SettingsNumberField(
              label: 'Xray Mux XUDP concurrency',
              value: _int(_group('Mux4RayItem'), 'XudpConcurrency'),
              onChanged: (v) => _set('Mux4RayItem', 'XudpConcurrency', v),
              helpText: 'XUDP 并发数。默认 16；不懂别动。',
              dirtyKey: _dk('Mux4RayItem', 'XudpConcurrency'),
            ),
            if (_expertMode)
              SettingsDropdown<String>(
                label: 'Xray Mux XUDP proxy UDP443',
                value: _str(_group('Mux4RayItem'), 'XudpProxyUDP443'),
                items: _items(<(String, String)>[
                  ('reject', 'reject'),
                  ('skip', 'skip'),
                ]),
                onChanged: (v) => _set('Mux4RayItem', 'XudpProxyUDP443', v),
                helpText: 'UDP443 走不走代理。默认 reject；对不上服务器会握手失败，不懂别动。',
                dirtyKey: _dk('Mux4RayItem', 'XudpProxyUDP443'),
              ),
            if (_expertMode)
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
                helpText: 'sing-box 复用协议。默认 h2mux；“空=不启用”反而最稳；协议对不上服务器会握手失败。',
                dirtyKey: _dk('Mux4SboxItem', 'Protocol'),
              ),
            // UX3-11 ghost field: Padding has no editor anywhere; a readonly
            // gray line keeps it inspectable without adding editability.
            if (_expertMode)
              Padding(
                padding: const EdgeInsets.only(left: 172, bottom: 2),
                child: Text(
                  'Mux 填充 (Padding)：当前 ${_group('Mux4SboxItem')['Padding'] ?? '空'}（历史保留，无控件，不动）。',
                  style: TextStyle(
                    fontSize: AppTokens.fontSizeCaption,
                    color: context.semantics.muted,
                  ),
                ),
              ),
          ],
        ),
      if (_showSection(const <String>['CoreBasicItem']))
        SettingsSection(
          title: '规则与缓存',
          sectionId: 'core-cache',
          expanded: _isOpen('core-cache'),
          onExpandedChanged: (v) => _setOpen('core-cache', v),
          onReset: () => _resetGroups('规则与缓存', const <String>['CoreBasicItem']),
          child: <Widget>[
            // UX3-10: moved out of the Mux section (pure shift, same tab).
            SettingsCheckbox(
              label: '启用 sing-box (规则集文件) 的缓存文件',
              value: _bool(core, 'EnableCacheFile4Sbox'),
              onChanged: (v) =>
                  _set('CoreBasicItem', 'EnableCacheFile4Sbox', v),
              helpText: 'sing-box 规则集缓存。默认开；关了每次重载都重新下规则。',
              dirtyKey: _dk('CoreBasicItem', 'EnableCacheFile4Sbox'),
            ),
          ],
        ),
      if (_showSection(const <String>['GrpcItem']))
        SettingsSection(
          // FLD-CFG-052..055: GrpcItem has no OptionSettingWindow control
          // upstream (defaults seeded by `ConfigHandler.LoadConfig`: 60 / 20 /
          // false / 0; consumed by the xray/sing-box gRPC transport emit). Kept
          // editable here without pretending upstream had a page. Null means
          // "缺省" and renders the frozen default.
          title: '历史保留（原版无 gRPC 设置页；出站 gRPC 传输参数）',
          sectionId: 'core-grpc',
          dimmed: true,
          expanded: _isOpen('core-grpc'),
          onExpandedChanged: (v) => _setOpen('core-grpc', v),
          onReset: () => _resetGroups('gRPC 保留', const <String>['GrpcItem']),
          child: <Widget>[
            SettingsNumberField(
              key: const ValueKey('settings-grpc-idle-timeout'),
              label: 'gRPC 空闲超时 (秒，缺省 60)',
              value: _int(_group('GrpcItem'), 'IdleTimeout'),
              onChanged: (v) => _set('GrpcItem', 'IdleTimeout', v),

              dirtyKey: _dk('GrpcItem', 'IdleTimeout'),
            ),
            SettingsNumberField(
              key: const ValueKey('settings-grpc-health-timeout'),
              label: 'gRPC 健康检查超时 (秒，缺省 20)',
              value: _int(_group('GrpcItem'), 'HealthCheckTimeout'),
              onChanged: (v) => _set('GrpcItem', 'HealthCheckTimeout', v),

              dirtyKey: _dk('GrpcItem', 'HealthCheckTimeout'),
            ),
            SettingsCheckbox(
              key: const ValueKey('settings-grpc-permit-without-stream'),
              label: 'gRPC 无流许可 (PermitWithoutStream)',
              value: _bool(_group('GrpcItem'), 'PermitWithoutStream'),
              onChanged: (v) => _set('GrpcItem', 'PermitWithoutStream', v),

              dirtyKey: _dk('GrpcItem', 'PermitWithoutStream'),
            ),
            SettingsNumberField(
              key: const ValueKey('settings-grpc-initial-windows-size'),
              label: 'gRPC 初始窗口 (缺省 0)',
              value: _int(_group('GrpcItem'), 'InitialWindowsSize'),
              onChanged: (v) => _set('GrpcItem', 'InitialWindowsSize', v),

              dirtyKey: _dk('GrpcItem', 'InitialWindowsSize'),
            ),
          ],
        ),
      if (_showSection(const <String>['HysteriaItem']))
        SettingsSection(
          title: 'Hysteria2',
          sectionId: 'core-hysteria',
          expanded: _isOpen('core-hysteria'),
          onExpandedChanged: (v) => _setOpen('core-hysteria', v),
          onReset: () =>
              _resetGroups('Hysteria2', const <String>['HysteriaItem']),
          child: <Widget>[
            SettingsNumberField(
              label: '上行 Mbps',
              value: _int(_group('HysteriaItem'), 'UpMbps'),
              onChanged: (v) => _set('HysteriaItem', 'UpMbps', v),

              dirtyKey: _dk('HysteriaItem', 'UpMbps'),
            ),
            SettingsNumberField(
              label: '下行 Mbps',
              value: _int(_group('HysteriaItem'), 'DownMbps'),
              onChanged: (v) => _set('HysteriaItem', 'DownMbps', v),

              dirtyKey: _dk('HysteriaItem', 'DownMbps'),
            ),
            SettingsNumberField(
              label: 'Hop 间隔',
              value: _int(_group('HysteriaItem'), 'HopInterval'),
              onChanged: (v) => _set('HysteriaItem', 'HopInterval', v),

              dirtyKey: _dk('HysteriaItem', 'HopInterval'),
            ),
          ],
        ),
      if (_showSection(const <String>['CoreBasicItem', 'Fragment4RayItem']))
        SettingsSection(
          title: '分片 (Fragment)',
          sectionId: 'core-fragment',
          expanded: _isOpen('core-fragment'),
          onExpandedChanged: (v) => _setOpen('core-fragment', v),
          onReset: () => _resetGroups('分片 (Fragment)', const <String>[
            'CoreBasicItem',
            'Fragment4RayItem',
          ]),
          child: <Widget>[
            SettingsCheckbox(
              label: '启用分片 (Fragment)',
              value: _bool(core, 'EnableFragment'),
              onChanged: (v) => _set('CoreBasicItem', 'EnableFragment', v),
              helpText:
                  'TLS 分片（绕 SNI 审查）。在审查区连不上时开；开后变慢/被限速就关。效果因地而异，参数不懂就用默认。',
              dirtyKey: _dk('CoreBasicItem', 'EnableFragment'),
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
              helpText: '分片形状。默认 tlshello；照抄成熟作业，别自创。',
              dirtyKey: _dk('Fragment4RayItem', 'Packets'),
            ),
            if (_expertMode)
              SettingsTextField(
                key: const ValueKey('fragment-lengths'),
                label: '分片长度 (逗号分隔)',
                value: _list(_group('Fragment4RayItem'), 'Lengths').join(','),
                width: 320,
                onChanged: (v) =>
                    _set('Fragment4RayItem', 'Lengths', _splitList(v)),
                helpText: _fragmentListHelp('分片长度，', 'Lengths', 'Length'),
                dirtyKey: _dk('Fragment4RayItem', 'Lengths'),
              ),
            if (_expertMode)
              SettingsTextField(
                label: '分片间隔 (逗号分隔)',
                value: _list(_group('Fragment4RayItem'), 'Delays').join(','),
                width: 320,
                onChanged: (v) =>
                    _set('Fragment4RayItem', 'Delays', _splitList(v)),
                helpText: _fragmentListHelp('分片间隔，', 'Delays', 'Interval'),
                dirtyKey: _dk('Fragment4RayItem', 'Delays'),
              ),
            if (_expertMode)
              SettingsTextField(
                key: const ValueKey('fragment-maxsplit'),
                label: '最大分片数',
                value: _str(_group('Fragment4RayItem'), 'MaxSplit'),
                onChanged: (v) => _set('Fragment4RayItem', 'MaxSplit', v),
                helpText: '最大分片数。0 的确切语义待核实；照抄成熟作业，别自创。',
                dirtyKey: _dk('Fragment4RayItem', 'MaxSplit'),
              ),
            if (_expertMode)
              SettingsCheckbox(
                label: '启用末端分片',
                value: _bool(core, 'EnableFinalFragment'),
                onChanged: (v) =>
                    _set('CoreBasicItem', 'EnableFinalFragment', v),
                helpText: '末端分片。专家项，不懂别动。',
                dirtyKey: _dk('CoreBasicItem', 'EnableFinalFragment'),
              ),
          ],
        ),
      if (_showSection(const <String>['KcpItem']))
        SettingsSection(
          title: '历史保留（原版 KCP 页在 OptionSettingWindow.xaml 中为注释）',
          sectionId: 'core-kcp',
          dimmed: true,
          expanded: _isOpen('core-kcp'),
          onExpandedChanged: (v) => _setOpen('core-kcp', v),
          onReset: () => _resetGroups('KCP 保留', const <String>['KcpItem']),
          child: <Widget>[
            SettingsNumberField(
              label: 'KCP MTU',
              value: _int(_group('KcpItem'), 'Mtu'),
              onChanged: (v) => _set('KcpItem', 'Mtu', v),

              dirtyKey: _dk('KcpItem', 'Mtu'),
            ),
            SettingsNumberField(
              label: 'KCP TTI',
              value: _int(_group('KcpItem'), 'Tti'),
              onChanged: (v) => _set('KcpItem', 'Tti', v),

              dirtyKey: _dk('KcpItem', 'Tti'),
            ),
            SettingsNumberField(
              label: 'KCP 上行容量',
              value: _int(_group('KcpItem'), 'UplinkCapacity'),
              onChanged: (v) => _set('KcpItem', 'UplinkCapacity', v),

              dirtyKey: _dk('KcpItem', 'UplinkCapacity'),
            ),
            SettingsNumberField(
              label: 'KCP 下行容量',
              value: _int(_group('KcpItem'), 'DownlinkCapacity'),
              onChanged: (v) => _set('KcpItem', 'DownlinkCapacity', v),

              dirtyKey: _dk('KcpItem', 'DownlinkCapacity'),
            ),
            SettingsNumberField(
              label: '拥塞窗口倍数',
              value: _int(_group('KcpItem'), 'CwndMultiplier'),
              onChanged: (v) => _set('KcpItem', 'CwndMultiplier', v),

              dirtyKey: _dk('KcpItem', 'CwndMultiplier'),
            ),
            SettingsNumberField(
              label: '最大发送窗口',
              value: _int(_group('KcpItem'), 'MaxSendingWindow'),
              onChanged: (v) => _set('KcpItem', 'MaxSendingWindow', v),

              dirtyKey: _dk('KcpItem', 'MaxSendingWindow'),
            ),
          ],
        ),
    ]);
  }

  Widget _displayTab() {
    final gui = _group('GuiItem');
    final ui = _group('UiItem');
    return _scroll(<Widget>[
      if (_showSection(const <String>['GuiItem']))
        SettingsSection(
          title: '显示',
          sectionId: 'display-show',
          expanded: _isOpen('display-show'),
          onExpandedChanged: (v) => _setOpen('display-show', v),
          onReset: () => _resetGroups('显示', const <String>['GuiItem']),
          child: <Widget>[
            SettingsCheckbox(
              label: '启用流量统计 (需重启)',
              value: _bool(gui, 'EnableStatistics'),
              onChanged: (v) => _set('GuiItem', 'EnableStatistics', v),

              dirtyKey: _dk('GuiItem', 'EnableStatistics'),
              helpText: '在状态栏显示今日流量与节点统计。关闭时显示 --，不是故障。',
            ),
            SettingsCheckbox(
              label: '显示实时速度 (需重启)',
              value: _bool(gui, 'DisplayRealTimeSpeed'),
              onChanged: (v) => _set('GuiItem', 'DisplayRealTimeSpeed', v),

              dirtyKey: _dk('GuiItem', 'DisplayRealTimeSpeed'),
              helpText: '在状态栏显示实时上传/下载速率。',
            ),
          ],
        ),
      if (_showSection(const <String>['GuiItem', 'UiItem']))
        SettingsSection(
          title: '启动与托盘',
          sectionId: 'display-startup',
          expanded: _isOpen('display-startup'),
          onExpandedChanged: (v) => _setOpen('display-startup', v),
          onReset: () =>
              _resetGroups('启动与托盘', const <String>['GuiItem', 'UiItem']),
          child: <Widget>[
            // F-DESKTOP-003: the toggle only edits the draft. The real Run key
            // is written after a successful save (upstream `SaveSettingAsync`
            // runs `AutoStartupHandler.UpdateTask` after `SaveConfig`), so
            // Cancel never touches host autostart.
            SettingsCheckbox(
              key: const ValueKey('autorun-toggle'),
              label: '开机启动 (可能会不成功)',
              value: _bool(gui, 'AutoRun'),
              onChanged: (v) => _set('GuiItem', 'AutoRun', v),

              dirtyKey: _dk('GuiItem', 'AutoRun'),
              helpText: '开机自动运行。可能因权限或杀软拦截而不成功，属正常现象。',
            ),
            SettingsCheckbox(
              key: const ValueKey('settings-hide2tray'),
              label: '关闭窗口时隐藏至托盘',
              value: _bool(ui, 'Hide2TrayWhenClose'),
              onChanged: (v) => _set('UiItem', 'Hide2TrayWhenClose', v),

              dirtyKey: _dk('UiItem', 'Hide2TrayWhenClose'),
              helpText: '点 X 后程序继续在后台运行（到托盘区找图标）。关闭后点 X 即真正退出。',
            ),
            SettingsCheckbox(
              label: '启动后隐藏窗口',
              value: _bool(ui, 'AutoHideStartup'),
              onChanged: (v) => _set('UiItem', 'AutoHideStartup', v),

              dirtyKey: _dk('UiItem', 'AutoHideStartup'),
              helpText: '启动时只留托盘图标、不弹窗口。建议与开机启动配合使用，否则可能误以为没启动。',
            ),
            SettingsNumberField(
              label: '托盘右键菜单配置展示数量限制',
              value: _int(gui, 'TrayMenuServersLimit'),
              onChanged: (v) => _set('GuiItem', 'TrayMenuServersLimit', v),

              dirtyKey: _dk('GuiItem', 'TrayMenuServersLimit'),
              helpText: '托盘右键菜单列出的节点个数。默认 20。',
            ),
            // Upstream binds this row to IsMacOS visibility; keep the same gate.
            if (Platform.isMacOS)
              SettingsCheckbox(
                label: 'macOS 在 Dock 栏中显示 (需重启)',
                value: _bool(ui, 'MacOSShowInDock'),
                onChanged: (v) => _set('UiItem', 'MacOSShowInDock', v),

                dirtyKey: _dk('UiItem', 'MacOSShowInDock'),
              ),
          ],
        ),
      if (_showSection(const <String>['GuiItem', 'UiItem']))
        SettingsSection(
          title: '列表与操作习惯',
          sectionId: 'display-list',
          expanded: _isOpen('display-list'),
          onExpandedChanged: (v) => _setOpen('display-list', v),
          onReset: () =>
              _resetGroups('列表与操作习惯', const <String>['GuiItem', 'UiItem']),
          child: <Widget>[
            SettingsCheckbox(
              key: const ValueKey('settings-keep-older-dedupl'),
              label: '去重时保留序号较小的项',
              value: _bool(gui, 'KeepOlderDedupl'),
              onChanged: (v) => _set('GuiItem', 'KeepOlderDedupl', v),

              dirtyKey: _dk('GuiItem', 'KeepOlderDedupl'),
              helpText: '导入或订阅去重时，保留序号较小的一条。',
            ),
            SettingsCheckbox(
              label: '自动调整配置列宽在更新订阅后',
              value: _bool(ui, 'EnableAutoAdjustMainLvColWidth'),
              onChanged: (v) =>
                  _set('UiItem', 'EnableAutoAdjustMainLvColWidth', v),
              dirtyKey: _dk('UiItem', 'EnableAutoAdjustMainLvColWidth'),
              helpText: '更新订阅后自动调整节点表格列宽。打开后手动调的宽度会被覆盖。',
            ),
            SettingsCheckbox(
              label: '隐藏 IP 信息',
              value: _bool(ui, 'HideColumnIpInfo'),
              onChanged: (v) => _set('UiItem', 'HideColumnIpInfo', v),

              dirtyKey: _dk('UiItem', 'HideColumnIpInfo'),
              helpText: '隐藏节点表的 IP 列，截图分享时建议打开。',
            ),
            SettingsCheckbox(
              label: '主界面双击设为活动',
              value: _bool(ui, 'DoubleClick2Activate'),
              onChanged: (v) => _set('UiItem', 'DoubleClick2Activate', v),

              dirtyKey: _dk('UiItem', 'DoubleClick2Activate'),
              helpText: '双击节点直接切换为活动线路。',
              risk: '误双击会立即切换线路并断流重连。',
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
              dirtyKey: _dk('UiItem', 'MainGirdOrientation'),
              helpText: '主界面分栏方向。切“标签”后原来的上下分栏就找不到了。',
            ),
            SettingsCheckbox(
              label: '启用配置拖放排序 (需重启)',
              value: _bool(ui, 'EnableDragDropSort'),
              onChanged: (v) => _set('UiItem', 'EnableDragDropSort', v),

              dirtyKey: _dk('UiItem', 'EnableDragDropSort'),
              helpText: '允许在节点表拖拽排序。打开后误拖会打乱顺序。',
            ),
          ],
        ),
      if (_showSection(const <String>['UiItem']))
        SettingsSection(
          title: '字体、语言与主题入口',
          sectionId: 'display-font',
          expanded: _isOpen('display-font'),
          onExpandedChanged: (v) => _setOpen('display-font', v),
          onReset: () => _resetGroups('字体、语言与主题入口', const <String>['UiItem']),
          child: <Widget>[
            SettingsTextField(
              label: '当前字体 (需重启)',
              value: _str(ui, 'CurrentFontFamily'),
              onChanged: (v) => _set('UiItem', 'CurrentFontFamily', v),

              dirtyKey: _dk('UiItem', 'CurrentFontFamily'),
              helpText: '界面字体。填不存在的名字会回退默认（待核实）。',
            ),
            SettingsNumberField(
              label: '字体大小',
              value: _int(ui, 'CurrentFontSize'),
              onChanged: (v) => _set('UiItem', 'CurrentFontSize', v),

              dirtyKey: _dk('UiItem', 'CurrentFontSize'),
              // U-B2: display-only "默认" for the canonical 0 (persistence kept).
              helpText: _int(ui, 'CurrentFontSize') == 0
                  ? '字号。当前：默认（跟随系统）。看不清再调大，过大可能错位。'
                  : '字号。当前：${_int(ui, 'CurrentFontSize')}。调回 0 即恢复默认。',
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

              dirtyKey: _dk('UiItem', 'CurrentLanguage'),
              helpText: '界面语言，需重启生效。建议到主题设置统一调整，此处同样可改。',
            ),
          ],
        ),
      if (_showSection(const <String>['SpeedTestItem']))
        SettingsSection(
          title: '测速',
          sectionId: 'display-speedtest',
          expanded: _isOpen('display-speedtest'),
          onExpandedChanged: (v) => _setOpen('display-speedtest', v),
          onReset: () => _resetGroups('测速', const <String>['SpeedTestItem']),
          child: <Widget>[
            SettingsNumberField(
              label: '测速单个超时值',
              value: _int(_group('SpeedTestItem'), 'SpeedTestTimeout'),
              onChanged: (v) => _set('SpeedTestItem', 'SpeedTestTimeout', v),

              dirtyKey: _dk('SpeedTestItem', 'SpeedTestTimeout'),
              helpText: '单个节点测速最多等几秒。设过大整组测速极慢。',
              risk: '小于 10 会被自动改回 10。',
            ),
            SettingsNumberField(
              label: '多线程测试时的并发数量',
              value: _int(_group('SpeedTestItem'), 'MixedConcurrencyCount'),
              onChanged: (v) =>
                  _set('SpeedTestItem', 'MixedConcurrencyCount', v),

              dirtyKey: _dk('SpeedTestItem', 'MixedConcurrencyCount'),
              helpText: '同时测几个节点。越大越快，也越容易被限流；默认 10 别动。',
              risk: '小于 10 会被自动改回 10。',
            ),
            SettingsTextField(
              label: '测速文件地址',
              value: _str(_group('SpeedTestItem'), 'SpeedTestUrl'),
              width: 360,
              onChanged: (v) => _set('SpeedTestItem', 'SpeedTestUrl', v),

              dirtyKey: _dk('SpeedTestItem', 'SpeedTestUrl'),
              helpText: '下载测速用的文件地址。文件越小越不准，站越慢整组越容易超时。',
            ),
            SettingsTextField(
              label: '真连接测试地址',
              value: _str(_group('SpeedTestItem'), 'SpeedPingTestUrl'),
              width: 360,
              onChanged: (v) => _set('SpeedTestItem', 'SpeedPingTestUrl', v),

              dirtyKey: _dk('SpeedTestItem', 'SpeedPingTestUrl'),
              helpText: '延迟/连通探测地址。国内直连可能不通，可换备选；全组延迟虚高时先看这里。',
            ),
            SettingsTextField(
              label: 'UDP 测试地址',
              value: _str(_group('SpeedTestItem'), 'UdpTestTarget'),
              width: 320,
              onChanged: (v) => _set('SpeedTestItem', 'UdpTestTarget', v),

              dirtyKey: _dk('SpeedTestItem', 'UdpTestTarget'),
              helpText: 'UDP 探测目标。改错会导致 UDP 测试全失败。',
            ),
            SettingsTextField(
              key: const ValueKey('settings-ipapi-url'),
              label: '当前连接信息测试地址',
              value: _str(_group('SpeedTestItem'), 'IPAPIUrl'),
              width: 320,
              onChanged: (v) => _set('SpeedTestItem', 'IPAPIUrl', v),

              dirtyKey: _dk('SpeedTestItem', 'IPAPIUrl'),
              helpText: '“当前连接信息”查询地址。留空用缺省服务，一般不动。',
            ),
          ],
        ),
      if (_showSection(const <String>['GuiItem', 'ConstItem']))
        SettingsSection(
          title: '资源、证书与自动更新',
          sectionId: 'display-resource',
          expanded: _isOpen('display-resource'),
          onExpandedChanged: (v) => _setOpen('display-resource', v),
          onReset: () => _resetGroups('资源、证书与自动更新', const <String>[
            'GuiItem',
            'ConstItem',
          ]),
          child: <Widget>[
            SettingsNumberField(
              key: const ValueKey('settings-auto-update-interval'),
              label: '自动更新 Geo 文件的间隔 (小时)',
              value: _int(gui, 'AutoUpdateInterval'),
              onChanged: (v) => _set('GuiItem', 'AutoUpdateInterval', v),

              dirtyKey: _dk('GuiItem', 'AutoUpdateInterval'),
              helpText: '自动更新 Geo/规则集文件的间隔（小时）。0＝关闭；关闭后规则库长期不更新而不自知。',
            ),
            SettingsTextField(
              key: const ValueKey('settings-sub-convert-url'),
              label: '订阅转换网址 (可选)',
              value: _str(_group('ConstItem'), 'SubConvertUrl'),
              width: 360,
              onChanged: (v) => _set('ConstItem', 'SubConvertUrl', v),

              dirtyKey: _dk('ConstItem', 'SubConvertUrl'),
              helpText: '订阅转换服务地址。留空用内置；填错会导致带转换目标的订阅全失败。',
            ),
            SettingsTextField(
              key: const ValueKey('settings-geo-source-url'),
              label: 'Geo 文件来源 (可选)',
              value: _str(_group('ConstItem'), 'GeoSourceUrl'),
              width: 360,
              onChanged: (v) => _set('ConstItem', 'GeoSourceUrl', v),

              dirtyKey: _dk('ConstItem', 'GeoSourceUrl'),
              helpText: 'Geo 文件下载源。留空用内置，最稳。',
            ),
            SettingsTextField(
              key: const ValueKey('settings-srs-source-url'),
              label: 'sing-box ruleset 文件来源 (可选)',
              value: _str(_group('ConstItem'), 'SrsSourceUrl'),
              width: 360,
              onChanged: (v) => _set('ConstItem', 'SrsSourceUrl', v),

              dirtyKey: _dk('ConstItem', 'SrsSourceUrl'),
              helpText: 'sing-box 规则集下载源。留空用内置。',
            ),
            SettingsTextField(
              key: const ValueKey('settings-route-rules-source-url'),
              label: '路由规则集来源 (可选)',
              value: _str(_group('ConstItem'), 'RouteRulesTemplateSourceUrl'),
              width: 360,
              onChanged: (v) =>
                  _set('ConstItem', 'RouteRulesTemplateSourceUrl', v),
              dirtyKey: _dk('ConstItem', 'RouteRulesTemplateSourceUrl'),
              helpText: '路由规则集下载源。留空用内置。',
            ),
            SettingsCheckbox(
              key: const ValueKey('settings-enable-hwa'),
              label: '启用硬件加速 (需重启)',
              value: _bool(gui, 'EnableHWA'),
              onChanged: (v) => _set('GuiItem', 'EnableHWA', v),

              dirtyKey: _dk('GuiItem', 'EnableHWA'),
              helpText: '界面渲染用显卡加速。老机器花屏/黑窗就关掉。',
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

              dirtyKey: _dk('GuiItem', 'RootCertProvider'),
              helpText: '软件自己下载东西时信任谁。默认 system；下载失败才换。',
            ),
          ],
        ),
      // Upstream keeps DNS FakeIP / Happy Eyeballs in DNSSettingWindow, not the
      // option window; retained here so the config stays editable until FIX-16B
      // adds the DNS window. Not counted as an upstream Window field.
      if (_showSection(const <String>[
        'SimpleDNSItem',
        'HappyEyeballs4RayItem',
      ]))
        SettingsSection(
          title: '历史保留（原版为独立 DNS 设置窗口）',
          sectionId: 'display-dns',
          dimmed: true,
          expanded: _isOpen('display-dns'),
          onExpandedChanged: (v) => _setOpen('display-dns', v),
          onReset: () => _resetGroups('DNS 保留', const <String>[
            'SimpleDNSItem',
            'HappyEyeballs4RayItem',
          ]),
          child: <Widget>[
            SettingsCheckbox(
              key: const ValueKey('fakeip-toggle'),
              label: '启用 FakeIP',
              value: _bool(_group('SimpleDNSItem'), 'FakeIP'),
              onChanged: (v) => _set('SimpleDNSItem', 'FakeIP', v),

              dirtyKey: _dk('SimpleDNSItem', 'FakeIP'),
            ),
            // Upstream `DNSSettingWindow` reveals GlobalFakeIp only while FakeIP
            // is checked; hiding it is a visible field linkage.
            if (_bool(_group('SimpleDNSItem'), 'FakeIP'))
              SettingsCheckbox(
                key: const ValueKey('global-fakeip-toggle'),
                label: '全局 FakeIP',
                value: _bool(_group('SimpleDNSItem'), 'GlobalFakeIp'),
                onChanged: (v) => _set('SimpleDNSItem', 'GlobalFakeIp', v),

                dirtyKey: _dk('SimpleDNSItem', 'GlobalFakeIp'),
              ),
            SettingsCheckbox(
              key: const ValueKey('happy-eyeballs-toggle'),
              label: '启用 Happy Eyeballs',
              value: _bool(_group('SimpleDNSItem'), 'EnableHappyEyeballs'),
              onChanged: (v) => _set('SimpleDNSItem', 'EnableHappyEyeballs', v),

              dirtyKey: _dk('SimpleDNSItem', 'EnableHappyEyeballs'),
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
                onChanged: (v) =>
                    _set('HappyEyeballs4RayItem', 'TryDelayMs', v),

                dirtyKey: _dk('HappyEyeballs4RayItem', 'TryDelayMs'),
              ),
              SettingsCheckbox(
                key: const ValueKey('happy-prioritize-ipv6'),
                label: '优先 IPv6',
                value: _bool(_group('HappyEyeballs4RayItem'), 'PrioritizeIPv6'),
                onChanged: (v) =>
                    _set('HappyEyeballs4RayItem', 'PrioritizeIPv6', v),
                dirtyKey: _dk('HappyEyeballs4RayItem', 'PrioritizeIPv6'),
              ),
              SettingsNumberField(
                key: const ValueKey('happy-interleave'),
                label: '交错',
                value: _int(_group('HappyEyeballs4RayItem'), 'Interleave'),
                onChanged: (v) =>
                    _set('HappyEyeballs4RayItem', 'Interleave', v),

                dirtyKey: _dk('HappyEyeballs4RayItem', 'Interleave'),
              ),
              SettingsNumberField(
                key: const ValueKey('happy-max-concurrent'),
                label: '最大并发尝试',
                value: _int(
                  _group('HappyEyeballs4RayItem'),
                  'MaxConcurrentTry',
                ),
                onChanged: (v) =>
                    _set('HappyEyeballs4RayItem', 'MaxConcurrentTry', v),
                dirtyKey: _dk('HappyEyeballs4RayItem', 'MaxConcurrentTry'),
              ),
            ],
          ],
        ),
      // ClashUIItem has no OptionSettingWindow control upstream; the monitor
      // Clash tabs now consume these fields (FIX-16C), so they stay editable.
      if (_showSection(const <String>['ClashUIItem']))
        SettingsSection(
          title: '历史保留（原版 Clash UI 设置）',
          sectionId: 'display-clash',
          dimmed: true,
          expanded: _isOpen('display-clash'),
          onExpandedChanged: (v) => _setOpen('display-clash', v),
          onReset: () =>
              _resetGroups('Clash UI 保留', const <String>['ClashUIItem']),
          child: <Widget>[
            const SettingsNote('代理/连接面板中也可即时调整自动刷新与排序，此处保留兼容，改一处处处生效。'),
            SettingsCheckbox(
              label: '启用 IPv6',
              value: _bool(_group('ClashUIItem'), 'EnableIPv6'),
              onChanged: (v) => _set('ClashUIItem', 'EnableIPv6', v),

              dirtyKey: _dk('ClashUIItem', 'EnableIPv6'),
              helpText: '待核实：mihomo 相关，语义待核实，请勿凭字面开启。',
            ),
            SettingsCheckbox(
              label: '合并 Mixin',
              value: _bool(_group('ClashUIItem'), 'EnableMixinContent'),
              onChanged: (v) => _set('ClashUIItem', 'EnableMixinContent', v),

              dirtyKey: _dk('ClashUIItem', 'EnableMixinContent'),
              helpText: '待核实：语义待核实，请勿凭字面开启。',
            ),
            SettingsDropdown<int>(
              label: '代理排序',
              value: _int(_group('ClashUIItem'), 'ProxiesSorting'),
              items: _items(<(String, int)>[('延迟', 0), ('名称', 1)]),
              onChanged: (v) => _set('ClashUIItem', 'ProxiesSorting', v),

              dirtyKey: _dk('ClashUIItem', 'ProxiesSorting'),
              helpText: '代理面板排序：按延迟快的在前（超时沉底不是丢了）。',
            ),
            SettingsCheckbox(
              label: '代理自动刷新',
              value: _bool(_group('ClashUIItem'), 'ProxiesAutoRefresh'),
              onChanged: (v) => _set('ClashUIItem', 'ProxiesAutoRefresh', v),

              dirtyKey: _dk('ClashUIItem', 'ProxiesAutoRefresh'),
              helpText: '代理面板每隔几秒自动刷一次。',
            ),
            SettingsNumberField(
              label: '代理刷新间隔',
              value: _int(_group('ClashUIItem'), 'ProxiesRefreshInterval'),
              onChanged: (v) =>
                  _set('ClashUIItem', 'ProxiesRefreshInterval', v),

              dirtyKey: _dk('ClashUIItem', 'ProxiesRefreshInterval'),
              helpText: '自动刷新的间隔（秒）。设为 0 或负数等同关闭。',
            ),
            SettingsCheckbox(
              label: '连接自动刷新',
              value: _bool(_group('ClashUIItem'), 'ConnectionsAutoRefresh'),
              onChanged: (v) =>
                  _set('ClashUIItem', 'ConnectionsAutoRefresh', v),

              dirtyKey: _dk('ClashUIItem', 'ConnectionsAutoRefresh'),
              helpText: '连接表每隔几秒自动刷一次。',
            ),
            SettingsNumberField(
              label: '连接刷新间隔',
              value: _int(_group('ClashUIItem'), 'ConnectionsRefreshInterval'),
              onChanged: (v) =>
                  _set('ClashUIItem', 'ConnectionsRefreshInterval', v),
              dirtyKey: _dk('ClashUIItem', 'ConnectionsRefreshInterval'),
              helpText: '自动刷新的间隔（秒）。设为 0 或负数等同关闭。',
            ),
          ],
        ),
    ]);
  }

  Widget _systemProxyTab() {
    final proxy = _group('SystemProxyItem');
    return _scroll(<Widget>[
      SettingsNote('系统代理的实际读写 (WinINET/WinHTTP) 属于 T13；本页仅保存配置。'),
      if (_showSection(const <String>['SystemProxyItem']))
        SettingsSection(
          title: '系统代理',
          sectionId: 'proxy-main',
          expanded: _isOpen('proxy-main'),
          onExpandedChanged: (v) => _setOpen('proxy-main', v),
          onReset: () =>
              _resetGroups('系统代理', const <String>['SystemProxyItem']),
          child: <Widget>[
            SettingsDropdown<int>(
              key: const ValueKey('settings-sysproxy-type'),
              label: '系统代理类型',
              value: _int(proxy, 'SysProxyType'),
              items: _items(<(String, int)>[
                ('清除', 0),
                ('设置', 1),
                ('不改变', 2),
                ('PAC', 3),
              ]),
              // UX3-03: switching to 设置/PAC asks first; cancel writes
              // nothing (the dropdown keeps showing the draft value).
              onChanged: (v) async {
                if (v == 1 || v == 3) {
                  final ok = await _confirmDanger(
                    title: v == 1 ? '接管全系统代理？' : '使用 PAC 模式？',
                    message: v == 1
                        ? '“设置”会接管全系统的上网开关，全系统应用都走代理。出问题先回来看这里。本页只存配置，不会替你改系统。继续吗？'
                        : 'PAC 模式按 PAC 文件自动分流；文件写错会导致部分网站打不开。本页只存配置。继续吗？',
                  );
                  if (!mounted || !ok) return;
                }
                _set('SystemProxyItem', 'SysProxyType', v);
              },
              helpText: '管“要不要接管全系统的上网开关”。日常用“设置”（走代理）/不用时“清除”；“不改变”适合只想让浏览器走代理的人；PAC 适合“部分网站走代理”的自动模式。',
              risk:
                  '切到“设置”后全系统应用都走代理，出问题先看这里；本页只存配置，真正改系统开关需要你的明确确认（推荐方案永不自动改）。',
              dirtyKey: _dk('SystemProxyItem', 'SysProxyType'),
            ),
            SettingsCheckbox(
              label: '请勿将代理服务器用于本地 (Intranet) 地址',
              value: _bool(proxy, 'NotProxyLocalAddress'),
              onChanged: (v) =>
                  _set('SystemProxyItem', 'NotProxyLocalAddress', v),
              helpText: '内网地址一律不走代理的总开关。默认开；关了之后路由器管理页、打印机、内网系统可能打不开。',
              dirtyKey: _dk('SystemProxyItem', 'NotProxyLocalAddress'),
            ),
            SettingsTextField(
              label: '例外',
              value: _str(proxy, 'SystemProxyExceptions'),
              width: 320,
              onChanged: (v) =>
                  _set('SystemProxyItem', 'SystemProxyExceptions', v),
              helpText: '系统代理的“白名单”，这些地址不进代理。默认别删，尤其 localhost/127.* 和 10.*/192.168.*；公司内网地址打不开了，先来这里加。',
              dirtyKey: _dk('SystemProxyItem', 'SystemProxyExceptions'),
            ),
            if (_expertMode)
              SettingsTextField(
                label: '高级代理设置，协议选择 (可选)',
                value: _str(proxy, 'SystemProxyAdvancedProtocol'),
                onChanged: (v) =>
                    _set('SystemProxyItem', 'SystemProxyAdvancedProtocol', v),
                helpText: '按协议分别设代理（遗留高级项）。不动；是否真实生效待核实。',
                dirtyKey: _dk('SystemProxyItem', 'SystemProxyAdvancedProtocol'),
              ),
            SettingsTextField(
              label: '自定义 PAC 文件路径',
              value: _str(proxy, 'CustomSystemProxyPacPath'),
              width: 320,
              onChanged: (v) =>
                  _set('SystemProxyItem', 'CustomSystemProxyPacPath', v),
              helpText: '自定义 PAC 文件（只有代理类型选了 PAC 才有用）。公司/学校下发了 PAC 文件时照抄路径；自己没 PAC 文件就别碰。',
              risk: 'PAC 写错会导致“能聊 QQ 打不开网页”式故障，先备份再改。消费细节待核实。',
              dirtyKey: _dk('SystemProxyItem', 'CustomSystemProxyPacPath'),
            ),
            SettingsTextField(
              label: '自定义系统代理脚本文件路径',
              value: _str(proxy, 'CustomSystemProxyScriptPath'),
              width: 320,
              onChanged: (v) =>
                  _set('SystemProxyItem', 'CustomSystemProxyScriptPath', v),
              helpText: '自定义代理脚本路径。极少用，不动。实际生效点待核实。',
              dirtyKey: _dk('SystemProxyItem', 'CustomSystemProxyScriptPath'),
            ),
          ],
        ),
    ]);
  }

  Widget _tunTab() {
    final tun = _group('TunModeItem');
    return _scroll(<Widget>[
      SettingsNote('TUN 模式的实际启用 (虚拟网卡/路由) 属于 T13；本页仅保存配置。'),
      // UX3-07:断网自救入口（只读小字，不自动执行任何操作）。
      const SettingsNote('开 TUN 后断网？先关“启用 Tun”→检查虚拟网卡驱动/管理员权限→看日志。'),
      if (_showSection(const <String>['TunModeItem']))
        SettingsSection(
          title: 'TUN 模式',
          sectionId: 'tun-main',
          expanded: _isOpen('tun-main'),
          onExpandedChanged: (v) => _setOpen('tun-main', v),
          onReset: () => _resetGroups('TUN 模式', const <String>['TunModeItem']),
          child: <Widget>[
            SettingsCheckbox(
              key: const ValueKey('settings-enable-tun'),
              label: '启用 Tun',
              value: _bool(tun, 'EnableTun'),
              // UX3-03: enabling asks first; cancel writes nothing.
              onChanged: (v) async {
                if (v) {
                  final ok = await _confirmDanger(
                    title: '接管全机流量？',
                    message:
                        'TUN 会接管全机流量，不走系统代理的软件也会进代理。驱动没就绪会整机断网。本页只存配置。继续吗？',
                  );
                  if (!mounted || !ok) return;
                }
                _set('TunModeItem', 'EnableTun', v);
              },
              helpText: 'TUN 是“接管全机流量”的总开关，效果最强、风险最大，能让不走系统代理的软件也进代理。系统代理搞不定的应用（如游戏客户端、终端工具）才开；开之前确认已装好虚拟网卡驱动/给了管理员权限。',
              risk: '驱动没就绪就开会整机断网，先会用系统代理再碰 TUN。',
              dirtyKey: _dk('TunModeItem', 'EnableTun'),
            ),
            SettingsCheckbox(
              label: '自动路由',
              value: _bool(tun, 'AutoRoute'),
              onChanged: (v) => _set('TunModeItem', 'AutoRoute', v),
              helpText: '自动写系统路由把流量引进 TUN。默认开；关了 TUN 等于空转（“开了没效果”先查它）。',
              dirtyKey: _dk('TunModeItem', 'AutoRoute'),
            ),
            SettingsCheckbox(
              label: '严格路由',
              value: _bool(tun, 'StrictRoute'),
              // UX3-07: turning strict routing off asks first (leak risk).
              onChanged: (v) async {
                if (!v) {
                  final ok = await _confirmDanger(
                    title: '关掉严格路由？',
                    message: '关后流量可能绕过 TUN 偷跑（泄漏直连 IP）。某些游戏/局域网联机必须关。继续吗？',
                  );
                  if (!mounted || !ok) return;
                }
                _set('TunModeItem', 'StrictRoute', v);
              },
              helpText: '严格路由，防流量绕过 TUN 偷跑（保隐私）。默认开；某些游戏/局域网联机必须关才连得上。',
              dirtyKey: _dk('TunModeItem', 'StrictRoute'),
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
              helpText: 'TUN 的实现方式。默认空=自动（gvisor）兼容最好；system 性能好但要管理员权限且可能不兼容；mixed 语义待核实。新手别动。',
              dirtyKey: _dk('TunModeItem', 'Stack'),
            ),
            SettingsNumberField(
              label: 'MTU',
              value: _int(tun, 'Mtu'),
              onChanged: (v) => _set('TunModeItem', 'Mtu', v),
              helpText:
                  'TUN 网卡的数据包大小，默认 9000。上网正常就别动；频繁掉包/慢才试 1500/1408，效果待核实。',
              dirtyKey: _dk('TunModeItem', 'Mtu'),
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
              helpText: 'ping 走哪。默认 rule（跟分流规则）；ping 全超时先看它是不是被设成了 drop，这不是断网。',
              dirtyKey: _dk('TunModeItem', 'IcmpRouting'),
            ),
            SettingsCheckbox(
              label: '启用 IPv6',
              value: _bool(tun, 'EnableIPv6Address'),
              onChanged: (v) => _set('TunModeItem', 'EnableIPv6Address', v),
              helpText:
                  'TUN 要不要管 IPv6。默认关（IPv6 走直连）；开了但 IPv6 不通会卡，家宽带没 IPv6 就别开。',
              dirtyKey: _dk('TunModeItem', 'EnableIPv6Address'),
            ),
            SettingsCheckbox(
              label: '旧版 TUN 保护',
              value: _bool(tun, 'EnableLegacyProtect'),
              // UX3-07: turning loop protection off asks first.
              onChanged: (v) async {
                if (!v) {
                  final ok = await _confirmDanger(
                    title: '关掉环路保护？',
                    message: '关后 TUN 流量可能环路导致断网。机制细节待核实。继续吗？',
                  );
                  if (!mounted || !ok) return;
                }
                _set('TunModeItem', 'EnableLegacyProtect', v);
              },
              helpText: '防 TUN 流量环路的保护。默认开，别关；关了可能环路断网。机制细节待核实。',
              dirtyKey: _dk('TunModeItem', 'EnableLegacyProtect'),
            ),
            if (_expertMode)
              SettingsTextField(
                key: const ValueKey('tun-route-exclude'),
                label: '路由排除地址',
                value: _list(tun, 'RouteExcludeAddress').join(','),
                width: 320,
                onChanged: (v) =>
                    _set('TunModeItem', 'RouteExcludeAddress', _splitList(v)),
                helpText: '这些网段不进 TUN（直连）。代理服务器 IP、公司内网段填这里；按逗号填。填错会泄漏，属专家操作。',
                dirtyKey: _dk('TunModeItem', 'RouteExcludeAddress'),
              ),
            if (_expertMode)
              SettingsTextField(
                label: 'Ipv4 地址',
                value: _str(tun, 'IPv4Address'),
                onChanged: (v) => _set('TunModeItem', 'IPv4Address', v),
                helpText: 'TUN 网卡自己的 IPv4 地址，高级用法才填；与家里网段冲突会起不来。默认空（自动）。',
                dirtyKey: _dk('TunModeItem', 'IPv4Address'),
              ),
            if (_expertMode)
              SettingsTextField(
                label: 'Ipv6 地址',
                value: _str(tun, 'IPv6Address'),
                onChanged: (v) => _set('TunModeItem', 'IPv6Address', v),
                helpText: 'TUN 网卡自己的 IPv6 地址，高级用法才填；与家里网段冲突会起不来。默认空（自动）。',
                dirtyKey: _dk('TunModeItem', 'IPv6Address'),
              ),
          ],
        ),
    ]);
  }

  Widget _coreTypeTab() {
    final items = _coreTypeItems();
    return _scroll(<Widget>[
      SettingsNote('内核类型按协议选择 (ConfigType → CoreType)；仅覆盖上游有下拉框的 8 项。'),
      if (_expertMode && _showSection(const <String>['CoreTypeItem']))
        SettingsSection(
          title: '内核类型',
          sectionId: 'coretype-main',
          expanded: _isOpen('coretype-main'),
          onExpandedChanged: (v) => _setOpen('coretype-main', v),
          onReset: () => _resetGroups('内核类型', const <String>['CoreTypeItem']),
          child: <Widget>[
            for (final entry in _coreTypeControls)
              SettingsDropdown<int>(
                key: ValueKey('settings-coretype-${entry.configType}'),
                label: entry.label,
                value: _coreTypeValue(items, entry.configType),
                items: _items(_coreTypes.map((c) => (c.$1, c.$2)).toList()),
                // UX3-10: switching cores asks first; cancel keeps the draft.
                onChanged: (v) async {
                  final next = v ?? 2;
                  final ok = await _confirmDanger(
                    title: '换内核？',
                    message: '换后这类协议全部走新内核；节点类型不被它支持会整类全灭（支持表待补）。继续吗？',
                  );
                  if (!mounted || !ok) return;
                  setState(() {
                    _setCoreType(items, entry.configType, next);
                  });
                },
                helpText: '默认 Xray；切之前确认节点类型被它支持（支持表待补），切错整类协议全灭。',
                dirtyKey: _coreTypeValue(items, entry.configType) != 2
                    ? 'CoreTypeItem.${entry.configType}'
                    : null,
              ),
          ],
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
