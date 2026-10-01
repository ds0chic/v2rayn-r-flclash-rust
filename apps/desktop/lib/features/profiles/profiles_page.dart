import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/profiles/column_settings_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';

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
              const _GroupPlaceholder(),
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
            _toolbarButton('添加', () => controller.emitAction('add')),
            _toolbarButton('编辑', () => controller.emitAction('edit')),
            _toolbarButton('删除', () => controller.emitAction('delete')),
            _toolbarButton('Tcping', controller.pingSelected),
            _toolbarButton('测试延迟', () => controller.runBlockingProbe(400)),
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

class _GroupPlaceholder extends StatelessWidget {
  const _GroupPlaceholder();

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 150,
      child: ListView(
        padding: const EdgeInsets.all(8),
        children: const <Widget>[
          Text('Groups', style: TextStyle(fontWeight: FontWeight.bold)),
          SizedBox(height: 8),
          ListTile(
            dense: true,
            title: Text('All', style: TextStyle(fontSize: 12)),
          ),
          ListTile(
            dense: true,
            title: Text('sub-000', style: TextStyle(fontSize: 12)),
          ),
          ListTile(
            dense: true,
            title: Text('sub-001', style: TextStyle(fontSize: 12)),
          ),
        ],
      ),
    );
  }
}
