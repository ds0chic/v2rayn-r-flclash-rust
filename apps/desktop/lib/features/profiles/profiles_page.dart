import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/column_settings_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

/// Profiles panel: top toolbar (LAY-PROFILES-001) + virtualized node table.
/// The bottom status bar and menu bar live in the shell, not here.
class ProfilesPage extends ConsumerStatefulWidget {
  const ProfilesPage({super.key});

  @override
  ConsumerState<ProfilesPage> createState() => _ProfilesPageState();
}

class _ProfilesPageState extends ConsumerState<ProfilesPage> {
  final TextEditingController _filterController = TextEditingController();
  final ScrollController _vertical = ScrollController();
  final ScrollController _horizontal = ScrollController();

  @override
  void dispose() {
    _filterController.dispose();
    _vertical.dispose();
    _horizontal.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: <Widget>[
        _Toolbar(filterController: _filterController),
        Expanded(
          child: Row(
            children: <Widget>[
              const GroupsPanel(),
              const VerticalDivider(width: 1),
              Expanded(
                child: ProfilesTable(
                  verticalController: _vertical,
                  horizontalController: _horizontal,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _Toolbar extends ConsumerWidget {
  const _Toolbar({required this.filterController});

  final TextEditingController filterController;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.read(profilesControllerProvider.notifier);
    final state = ref.watch(profilesControllerProvider);
    // Horizontally scrollable so the toolbar never overflows at narrow widths.
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainer,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
        child: Row(
          children: <Widget>[
            SizedBox(
              width: 260,
              height: 30,
              child: TextField(
                key: const ValueKey('filter-field'),
                controller: filterController,
                decoration: const InputDecoration(
                  isDense: true,
                  hintText: '过滤 (Enter 刷新)',
                  prefixIcon: Icon(Icons.search, size: 18),
                  border: OutlineInputBorder(),
                ),
                onChanged: controller.setFilter,
                onSubmitted: (_) => controller.submitFilter(),
              ),
            ),
            const SizedBox(width: 8),
            _AddButton(),
            _toolbarButton('编辑', () => editSelectedProfile(context, ref)),
            _toolbarButton('删除', () => deleteSelectedProfiles(context, ref)),
            _toolbarButton('复制', () => copySelectedProfiles(ref)),
            _toolbarButton('备注', () => renameSelectedProfile(context, ref)),
            _toolbarButton('启用/停用', () => toggleActiveSelected(ref)),
            _toolbarButton(
              'TCPing',
              () => controller.emitAction(ProfileAction.tcping),
            ),
            _toolbarButton(
              '真延迟',
              () => controller.emitAction(ProfileAction.realping),
            ),
            _toolbarButton(
              '测速',
              () => controller.emitAction(ProfileAction.speedtest),
            ),
            _toolbarButton(
              '混合',
              () => controller.emitAction(ProfileAction.mixedTest),
            ),
            _toolbarButton(
              '快速真延迟',
              () => controller.emitAction(ProfileAction.fastRealping),
            ),
            _toolbarButton('停止测试', controller.cancelSpeedTest),
            _toolbarButton(
              '移除无效',
              () => controller.emitAction(ProfileAction.removeInvalid),
            ),
            if (state.speedTestRunning)
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6),
                child: StageIndicator(
                  key: const ValueKey('speedtest-stage'),
                  stage: state.speedTestStage,
                ),
              ),
            _toolbarButton(
              '自动列宽',
              () => controller.emitAction('autofit-columns'),
            ),
            const SizedBox(width: 8),
            IconButton(
              key: const ValueKey('column-settings-button'),
              tooltip: '显示列设置',
              iconSize: 18,
              onPressed: () => showColumnSettingsDialog(context, ref),
              icon: const Icon(Icons.view_column_outlined),
            ),
            const SizedBox(width: 8),
            const Text('双击激活', style: TextStyle(fontSize: 12)),
            Switch(
              key: const ValueKey('double-click-switch'),
              value: state.doubleClick2Activate,
              onChanged: (_) => controller.toggleDoubleClick2Activate(),
            ),
          ],
        ),
      ),
    );
  }

  Widget _toolbarButton(String label, VoidCallback onPressed) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2),
      child: TextButton(
        key: ValueKey('toolbar-$label'),
        onPressed: onPressed,
        child: Text(label, style: const TextStyle(fontSize: 12)),
      ),
    );
  }
}

/// "添加" toolbar button with the 11 basic-protocol submenu.
class _AddButton extends ConsumerWidget {
  static const List<ConfigType> _types = <ConfigType>[
    ConfigType.vmess,
    ConfigType.vless,
    ConfigType.shadowsocks,
    ConfigType.socks,
    ConfigType.http,
    ConfigType.trojan,
    ConfigType.hysteria2,
    ConfigType.tuic,
    ConfigType.wireGuard,
    ConfigType.anytls,
    ConfigType.naive,
  ];

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return PopupMenuButton<ConfigType>(
      key: const ValueKey('toolbar-添加'),
      tooltip: '添加节点',
      onSelected: (type) => startAddProfile(context, ref, type),
      itemBuilder: (context) => <PopupMenuEntry<ConfigType>>[
        for (final type in _types)
          PopupMenuItem<ConfigType>(
            key: ValueKey('add-${type.name}'),
            value: type,
            child: Text(type.name, style: const TextStyle(fontSize: 12)),
          ),
      ],
      child: const Padding(
        padding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(Icons.add, size: 16),
            SizedBox(width: 4),
            Text('添加', style: TextStyle(fontSize: 12)),
          ],
        ),
      ),
    );
  }
}

/// Left subscription-groups panel: All + one row per SubItem with node
/// counts. Selecting a row filters the node table by `subid`
/// (`ProfilesController.setGroupSubId`); the two buttons generate policy
/// groups for the selected subscription.
class GroupsPanel extends ConsumerWidget {
  const GroupsPanel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.read(profilesControllerProvider.notifier);
    final state = ref.watch(profilesControllerProvider);
    final subs = controller.subItems();
    final counts = <String, int>{};
    for (final p in state.profiles) {
      counts[p.subid] = (counts[p.subid] ?? 0) + 1;
    }
    final selected = state.groupSubId;
    return SizedBox(
      width: 170,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const Padding(
            padding: EdgeInsets.all(8),
            child: Text(
              '分组',
              key: ValueKey('groups-title'),
              style: TextStyle(fontWeight: FontWeight.bold, fontSize: 13),
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.symmetric(horizontal: 4),
              children: <Widget>[
                _row(
                  key: 'group-filter-all',
                  label: '全部 (${state.profiles.length})',
                  selected: selected == null,
                  onTap: () => controller.setGroupSubId(null),
                ),
                for (final sub in subs)
                  _row(
                    key: 'group-filter-${sub.id}',
                    label:
                        '${sub.remarks.isEmpty ? sub.id : sub.remarks} (${counts[sub.id] ?? 0})',
                    selected: selected == sub.id,
                    onTap: () => controller.setGroupSubId(sub.id),
                  ),
                if (subs.isEmpty)
                  const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 8, vertical: 12),
                    child: Text(
                      '暂无订阅分组',
                      key: ValueKey('groups-empty'),
                      style: TextStyle(fontSize: 11.5, color: Colors.grey),
                    ),
                  ),
              ],
            ),
          ),
          const Divider(height: 1),
          Padding(
            padding: const EdgeInsets.all(6),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                OutlinedButton(
                  key: const ValueKey('group-gen-all'),
                  onPressed: selected == null
                      ? null
                      : () => controller.genGroupAll(selected),
                  child: const Text('生成全部组', style: TextStyle(fontSize: 12)),
                ),
                const SizedBox(height: 4),
                OutlinedButton(
                  key: const ValueKey('group-gen-region'),
                  onPressed: selected == null
                      ? null
                      : () => controller.genGroupRegion(selected),
                  child: const Text('生成地区组', style: TextStyle(fontSize: 12)),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _row({
    required String key,
    required String label,
    required bool selected,
    required VoidCallback onTap,
  }) {
    return ListTile(
      key: ValueKey(key),
      dense: true,
      selected: selected,
      title: Text(label, style: const TextStyle(fontSize: 12)),
      onTap: onTap,
    );
  }
}
