import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

/// Column visibility/order editor. Mirrors upstream LAY-PROFILES-003
/// (`UiItem.MainColumnItem`: Name / Width / DisplayIndex). Persisted through
/// `ui_state.json`; no backend involved.
Future<void> showColumnSettingsDialog(BuildContext context, WidgetRef ref) {
  final controller = ref.read(profilesControllerProvider.notifier);
  return showDialog<void>(
    context: context,
    builder: (context) {
      return Consumer(
        builder: (context, ref, _) {
          final columns = ref.watch(profilesControllerProvider).columns;
          return AlertDialog(
            key: const ValueKey<String>('column-settings-dialog'),
            title: const Text('显示列设置', style: TextStyle(fontSize: 15)),
            content: SizedBox(
              width: 420,
              height: 420,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  const Padding(
                    padding: EdgeInsets.only(bottom: 6),
                    child: Text(
                      '顺序 / 显隐 / 宽度会保存到 ui_state.json（与 Clash 连接列同一状态源，迁移与备份不丢）',
                      style: TextStyle(fontSize: 11),
                    ),
                  ),
                  Expanded(
                    child: ListView.builder(
                      itemCount: columns.length,
                      itemBuilder: (context, index) {
                        final column = columns[index];
                        return Row(
                          children: <Widget>[
                            Checkbox(
                              key: ValueKey<String>('colvis-${column.key}'),
                              value: column.visible,
                              onChanged: (_) =>
                                  controller.toggleColumnVisibility(column.key),
                            ),
                            Expanded(
                              child: Text(
                                column.title,
                                style: const TextStyle(fontSize: 12),
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            IconButton(
                              key: ValueKey<String>(
                                'colwidthdec-${column.key}',
                              ),
                              tooltip: '减小列宽',
                              iconSize: 14,
                              visualDensity: VisualDensity.compact,
                              onPressed: column.width <= 40
                                  ? null
                                  : () => controller.resizeColumn(
                                      column.key,
                                      -10,
                                    ),
                              icon: const Icon(Icons.remove),
                            ),
                            SizedBox(
                              width: 40,
                              child: Text(
                                '${column.width.round()}',
                                key: ValueKey<String>('colwidth-${column.key}'),
                                textAlign: TextAlign.center,
                                style: const TextStyle(fontSize: 11),
                              ),
                            ),
                            IconButton(
                              key: ValueKey<String>(
                                'colwidthinc-${column.key}',
                              ),
                              tooltip: '增大列宽',
                              iconSize: 14,
                              visualDensity: VisualDensity.compact,
                              onPressed: column.width >= 600
                                  ? null
                                  : () =>
                                        controller.resizeColumn(column.key, 10),
                              icon: const Icon(Icons.add),
                            ),
                            IconButton(
                              key: ValueKey<String>('colup-${column.key}'),
                              tooltip: '上移',
                              iconSize: 16,
                              onPressed: index == 0
                                  ? null
                                  : () => controller.moveColumn(column.key, -1),
                              icon: const Icon(Icons.arrow_upward),
                            ),
                            IconButton(
                              key: ValueKey<String>('coldown-${column.key}'),
                              tooltip: '下移',
                              iconSize: 16,
                              onPressed: index == columns.length - 1
                                  ? null
                                  : () => controller.moveColumn(column.key, 1),
                              icon: const Icon(Icons.arrow_downward),
                            ),
                          ],
                        );
                      },
                    ),
                  ),
                ],
              ),
            ),
            actions: <Widget>[
              TextButton(
                key: const ValueKey<String>('column-settings-close'),
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('关闭'),
              ),
            ],
          );
        },
      );
    },
  );
}
