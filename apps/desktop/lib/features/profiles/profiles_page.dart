import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/profiles/column_settings_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';
import 'package:v2rayn_desktop/features/profiles/table_actions.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';
import 'package:v2rayn_desktop/perf/perf_harness.dart';
import 'package:v2rayn_desktop/perf/t18_bench.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';
import 'package:v2rayn_desktop/shared/widgets/adaptive_toolbar.dart';
import 'package:v2rayn_desktop/shared/widgets/empty_state.dart';

/// Profiles panel: top toolbar (LAY-PROFILES-001) + virtualized node table.
///
/// The top row restores the upstream `ProfilesView.xaml:24..99` WrapPanel:
/// subscription-group choice chips -> edit/add subscription icons -> 200px
/// filter box -> autofit-columns / fast-realping / mixed-test icons. The node
/// table fills the whole width below it (no left group sidebar).
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
  void initState() {
    super.initState();
    if (T18Bench.enabled && T18Bench.scenario == 'scroll') {
      WidgetsBinding.instance.addPostFrameCallback((_) => _runT18Scroll());
    }
  }

  Future<void> _runT18Scroll() async {
    // Let the first frame settle so the table has a real maxScrollExtent.
    await Future<void>.delayed(const Duration(milliseconds: 800));
    if (!mounted) return;
    await PerfHarness.run(
      vertical: _vertical,
      rowCount: T18Bench.rows,
      scrollSteps: T18Bench.scrollSteps,
      outputFile:
          '${T18Bench.dir}${Platform.pathSeparator}scroll_${T18Bench.rows}'
          '${T18Bench.tag.isEmpty ? '' : '_${T18Bench.tag}'}.json',
      label: 'T18',
    );
    exit(0);
  }

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
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _Toolbar(filterController: _filterController),
        Expanded(
          child: ProfilesTable(
            verticalController: _vertical,
            horizontalController: _horizontal,
          ),
        ),
      ],
    );
  }
}

/// Top WrapPanel row (LAY-PROFILES-001). Everything flows through one [Wrap]
/// so the original controls wrap as a unit at narrow widths instead of being
/// clipped or pushed into a second text-command row.
class _Toolbar extends ConsumerWidget {
  const _Toolbar({required this.filterController});

  final TextEditingController filterController;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.read(profilesControllerProvider.notifier);
    final state = ref.watch(profilesControllerProvider);
    final subs = controller.subItems();
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainer,
      child: AdaptiveToolbar(
        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
        spacing: AppTokens.toolbarIconGap,
        runSpacing: 4,
        children: <Widget>[
          _GroupChip(
            keyId: 'group-filter-all',
            label: '全部',
            selected: state.groupSubId == null,
            onTap: () => controller.setGroupSubId(null),
          ),
          for (final sub in subs)
            _GroupChip(
              keyId: 'group-filter-${sub.id}',
              label: sub.remarks.isEmpty ? sub.id : sub.remarks,
              selected: state.groupSubId == sub.id,
              onTap: () => controller.setGroupSubId(sub.id),
            ),
          _IconTool(
            keyId: 'toolbar-sub-edit',
            tooltip: '编辑订阅',
            icon: Icons.edit_outlined,
            onPressed: () => openSubSettings(context, ref),
          ),
          _IconTool(
            keyId: 'toolbar-sub-add',
            tooltip: '新增订阅',
            icon: Icons.add,
            onPressed: () => openSubSettings(context, ref),
          ),
          const SizedBox(
            width: AppTokens.toolbarGroupFilterGap - AppTokens.toolbarIconGap,
          ),
          _FilterField(controller: filterController),
          _IconTool(
            keyId: 'toolbar-自动列宽',
            tooltip: '自动调整列宽',
            icon: Icons.vertical_split_outlined,
            onPressed: () => controller.emitAction('autofit-columns'),
          ),
          _IconTool(
            keyId: 'toolbar-快速真延迟',
            tooltip: '一键测试真连接延迟',
            icon: Icons.bolt_outlined,
            onPressed: () => controller.emitAction(ProfileAction.fastRealping),
          ),
          _IconTool(
            keyId: 'toolbar-混合',
            tooltip: '一键多线程测试延迟和速度 (Ctrl+E)',
            icon: Icons.speed_outlined,
            onPressed: () => controller.emitAction(ProfileAction.mixedTest),
          ),
          // Actions with no upstream top-row slot but no other reachable entry
          // in the current app yet (registered in the UX-SPACE-01 evidence).
          _IconTool(
            keyId: 'toolbar-备注',
            tooltip: '编辑备注',
            icon: Icons.drive_file_rename_outline,
            onPressed: () => renameSelectedProfile(context, ref),
          ),
          _IconTool(
            keyId: 'column-settings-button',
            tooltip: '显示列设置',
            icon: Icons.view_column_outlined,
            onPressed: () => showColumnSettingsDialog(context, ref),
          ),
          if (state.speedTestRunning) ...<Widget>[
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 6),
              child: StageIndicator(
                key: const ValueKey('speedtest-stage'),
                stage: state.speedTestStage,
              ),
            ),
            _IconTool(
              keyId: 'toolbar-停止测试',
              tooltip: '停止测试 (Esc)',
              icon: Icons.stop_circle_outlined,
              onPressed: controller.cancelSpeedTest,
            ),
          ],
          if (state.speedTestMessage != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 6),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 420),
                child: Text(
                  state.speedTestMessage!,
                  key: const ValueKey('speedtest-message'),
                  style: TextStyle(
                    fontSize: 12,
                    color: Theme.of(context).colorScheme.primary,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
          if (state.orderMessage != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 6),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 420),
                child: Text(
                  state.orderMessage!,
                  key: const ValueKey('order-message'),
                  style: TextStyle(
                    fontSize: 12,
                    color: Theme.of(context).colorScheme.error,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// One subscription-group choice chip. Long Chinese group names wrap inside a
/// capped width instead of being clipped horizontally.
class _GroupChip extends StatelessWidget {
  const _GroupChip({
    required this.keyId,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String keyId;
  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 360),
      child: ChoiceChip(
        key: ValueKey(keyId),
        label: Text(label, style: const TextStyle(fontSize: 12)),
        selected: selected,
        onSelected: (_) => onTap(),
        visualDensity: VisualDensity.compact,
        materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
        labelPadding: const EdgeInsets.symmetric(horizontal: 4),
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      ),
    );
  }
}

/// 30x30 upstream icon control (adjacent outer edges 8px apart via the
/// surrounding Wrap spacing).
class _IconTool extends StatelessWidget {
  const _IconTool({
    required this.keyId,
    required this.tooltip,
    required this.icon,
    required this.onPressed,
  });

  final String keyId;
  final String tooltip;
  final IconData icon;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: AppTokens.toolbarIconButton,
      height: AppTokens.toolbarIconButton,
      child: IconButton(
        key: ValueKey(keyId),
        tooltip: tooltip,
        onPressed: onPressed,
        icon: Icon(icon),
        iconSize: AppTokens.iconSizeToolbar,
        padding: EdgeInsets.zero,
        constraints: const BoxConstraints(),
        style: IconButton.styleFrom(
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          minimumSize: const Size(
            AppTokens.toolbarIconButton,
            AppTokens.toolbarIconButton,
          ),
          padding: EdgeInsets.zero,
        ),
      ),
    );
  }
}

/// Upstream `txtServerFilter`: 200 logical px, hint `ResUI.MsgServerTitle`.
class _FilterField extends ConsumerWidget {
  const _FilterField({required this.controller});

  final TextEditingController controller;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final profiles = ref.read(profilesControllerProvider.notifier);
    return SizedBox(
      width: AppTokens.toolbarFilterWidth,
      height: AppTokens.toolbarIconButton,
      child: TextField(
        key: const ValueKey('filter-field'),
        controller: controller,
        decoration: const InputDecoration(
          isDense: true,
          hintText: '过滤器，按回车执行',
          prefixIcon: Icon(Icons.search, size: 18),
          prefixIconConstraints: BoxConstraints(minWidth: 30, minHeight: 30),
          border: OutlineInputBorder(),
          contentPadding: EdgeInsets.symmetric(horizontal: 6, vertical: 6),
        ),
        onChanged: profiles.setFilter,
        onSubmitted: (_) => profiles.submitFilter(),
      ),
    );
  }
}
