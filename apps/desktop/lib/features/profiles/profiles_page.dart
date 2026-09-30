import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';
import 'package:v2rayn_desktop/perf/perf_harness.dart';

class ProfilesPage extends ConsumerStatefulWidget {
  const ProfilesPage({super.key});

  @override
  ConsumerState<ProfilesPage> createState() => _ProfilesPageState();
}

class _ProfilesPageState extends ConsumerState<ProfilesPage> {
  final TextEditingController _filterController = TextEditingController();
  final ScrollController _vertical = ScrollController();
  final ScrollController _horizontal = ScrollController();
  bool _benchStarted = false;

  @override
  void dispose() {
    _filterController.dispose();
    _vertical.dispose();
    _horizontal.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(profilesControllerProvider);
    final controller = ref.read(profilesControllerProvider.notifier);
    _maybeStartBench(state.totalCount);

    return Scaffold(
      body: Column(
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
          _StatusBar(
            total: state.totalCount,
            visible: state.visible.length,
            selected: state.selectedCount,
            rustCount: state.rustCount,
            lastAction: state.lastEvent?.action ?? '-',
            lastDetail: state.lastEvent?.detail ?? '',
            ackSeq: state.lastAckSeq,
            blockingBusy: state.blockingBusy,
            onRunBlocking: () => controller.runBlockingProbe(400),
          ),
        ],
      ),
    );
  }

  void _maybeStartBench(int rowCount) {
    if (_benchStarted || !PerfHarness.enabled) return;
    _benchStarted = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await PerfHarness.run(vertical: _vertical, rowCount: rowCount);
      exit(0);
    });
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
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Row(
          children: <Widget>[
            const Text(
              'v2rayN-R (T01)',
              key: ValueKey('app-title'),
              style: TextStyle(fontWeight: FontWeight.bold, fontSize: 14),
            ),
            const SizedBox(width: 16),
            SizedBox(
              width: 280,
              height: 32,
              child: TextField(
                key: const ValueKey('filter-field'),
                controller: filterController,
                decoration: const InputDecoration(
                  isDense: true,
                  hintText: 'Filter nodes (Enter to refresh)',
                  prefixIcon: Icon(Icons.search, size: 18),
                  border: OutlineInputBorder(),
                ),
                onChanged: controller.setFilter,
                onSubmitted: (_) => controller.submitFilter(),
              ),
            ),
            const SizedBox(width: 8),
            _toolbarButton('Add', () => controller.emitAction('add')),
            _toolbarButton('Edit', () => controller.emitAction('edit')),
            _toolbarButton('Delete', () => controller.emitAction('delete')),
            _toolbarButton('Tcping', controller.pingSelected),
            _toolbarButton(
              'Test Blocking',
              () => controller.runBlockingProbe(400),
            ),
            const SizedBox(width: 16),
            const Text('DoubleClick2Activate', style: TextStyle(fontSize: 12)),
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
      width: 160,
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

class _StatusBar extends StatelessWidget {
  const _StatusBar({
    required this.total,
    required this.visible,
    required this.selected,
    required this.rustCount,
    required this.lastAction,
    required this.lastDetail,
    required this.ackSeq,
    required this.blockingBusy,
    required this.onRunBlocking,
  });

  final int total;
  final int visible;
  final int selected;
  final int rustCount;
  final String lastAction;
  final String lastDetail;
  final int? ackSeq;
  final bool blockingBusy;
  final VoidCallback onRunBlocking;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: SizedBox(
        height: 28,
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(
            children: <Widget>[
              Text(
                'total=$total visible=$visible selected=$selected',
                key: const ValueKey('status-counts'),
                style: const TextStyle(fontSize: 12),
              ),
              const SizedBox(width: 16),
              Text(
                'rust=$rustCount ack=$ackSeq',
                key: const ValueKey('status-rust'),
                style: const TextStyle(fontSize: 12),
              ),
              const SizedBox(width: 16),
              Text(
                'last=[$lastAction] $lastDetail',
                key: const ValueKey('status-last-event'),
                style: const TextStyle(fontSize: 12),
              ),
              const SizedBox(width: 16),
              if (blockingBusy)
                const Padding(
                  padding: EdgeInsets.symmetric(horizontal: 8),
                  child: Text('blocking...', style: TextStyle(fontSize: 12)),
                ),
              TextButton(
                key: const ValueKey('status-blocking'),
                onPressed: blockingBusy ? null : onRunBlocking,
                child: const Text(
                  'Blocking probe',
                  style: TextStyle(fontSize: 12),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
