import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';

/// Open the backup-and-restore window (ACT-WIN-007, F-BACKUP-001..004).
Future<void> showBackupAndRestoreWindow(BuildContext context, WidgetRef ref) {
  return showDialog<void>(
    context: context,
    barrierDismissible: false,
    builder: (_) => const BackupAndRestoreView(),
  );
}

class BackupAndRestoreView extends ConsumerStatefulWidget {
  const BackupAndRestoreView({super.key});

  @override
  ConsumerState<BackupAndRestoreView> createState() =>
      _BackupAndRestoreViewState();
}

class _BackupAndRestoreViewState extends ConsumerState<BackupAndRestoreView> {
  final _destDir = TextEditingController();
  final _archivePath = TextEditingController();
  final _bundleParent = TextEditingController();
  final _url = TextEditingController();
  final _userName = TextEditingController();
  final _password = TextEditingController();
  final _dirName = TextEditingController();

  @override
  void initState() {
    super.initState();
    final config = ref.read(backupControllerProvider).webdav;
    _url.text = config.url;
    _userName.text = config.userName;
    _password.text = config.password;
    _dirName.text = config.dirName;
  }

  @override
  void dispose() {
    _destDir.dispose();
    _archivePath.dispose();
    _bundleParent.dispose();
    _url.dispose();
    _userName.dispose();
    _password.dispose();
    _dirName.dispose();
    super.dispose();
  }

  c.WebDavConfigDto get _webdav => c.WebDavConfigDto(
    url: _url.text.trim(),
    userName: _userName.text.trim(),
    password: _password.text,
    dirName: _dirName.text.trim(),
  );

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(backupControllerProvider);
    final controller = ref.read(backupControllerProvider.notifier);
    return AlertDialog(
      key: const ValueKey('backup-window'),
      title: const Text('备份与还原', style: TextStyle(fontSize: 15)),
      contentPadding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
      content: SizedBox(
        width: 760,
        height: 560,
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(bottom: 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              _section('本地备份 / 恢复'),
              _pathRow(
                label: '保存到目录',
                fieldKey: 'backup-dest-field',
                controller: _destDir,
                buttonKey: 'backup-local-btn',
                buttonLabel: '本地备份',
                onPressed: () => controller.localBackup(_destDir.text),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 3),
                child: Row(
                  children: <Widget>[
                    const SizedBox(
                      width: 96,
                      child: Text('本地恢复', style: TextStyle(fontSize: 12)),
                    ),
                    OutlinedButton(
                      key: const ValueKey('backup-restore-zip-btn'),
                      onPressed: state.busy
                          ? null
                          : () => controller.restoreFromArchive(),
                      child: const Text(
                        '选择备份 ZIP 恢复',
                        style: TextStyle(fontSize: 12),
                      ),
                    ),
                    const SizedBox(width: 8),
                    OutlinedButton(
                      key: const ValueKey('backup-restore-dir-btn'),
                      onPressed: state.busy
                          ? null
                          : () => controller.restoreFromDirectory(),
                      child: const Text(
                        '选择备份包目录恢复',
                        style: TextStyle(fontSize: 12),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 6),
              _pathRow(
                label: '原版 ZIP 路径',
                fieldKey: 'backup-archive-field',
                controller: _archivePath,
                buttonKey: 'backup-recognize-btn',
                buttonLabel: '识别原版 ZIP',
                onPressed: () => controller.recognize(_archivePath.text),
              ),
              Row(
                children: <Widget>[
                  const Spacer(),
                  OutlinedButton(
                    key: const ValueKey('backup-import-btn'),
                    onPressed: state.busy
                        ? null
                        : () => controller.importUpstream(_archivePath.text),
                    child: const Text(
                      '导入原版 ZIP',
                      style: TextStyle(fontSize: 12),
                    ),
                  ),
                ],
              ),
              _pathRow(
                label: '备份列表目录',
                fieldKey: 'backup-list-field',
                controller: _bundleParent,
                buttonKey: 'backup-list-btn',
                buttonLabel: '列出备份',
                onPressed: () => controller.reloadBundles(_bundleParent.text),
              ),
              if (state.bundles.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      Text(
                        '已发现 ${state.bundles.length} 个备份包',
                        style: const TextStyle(fontSize: 12),
                      ),
                      for (var i = 0; i < state.bundles.length; i++)
                        Row(
                          key: ValueKey('backup-list-item-$i'),
                          children: <Widget>[
                            Expanded(
                              child: Text(
                                _bundleLabel(state.bundles[i]),
                                style: const TextStyle(fontSize: 12),
                              ),
                            ),
                            OutlinedButton(
                              key: ValueKey('backup-list-restore-$i'),
                              onPressed: state.busy
                                  ? null
                                  : () => controller.restoreListed(
                                      state.bundles[i],
                                    ),
                              child: const Text(
                                '恢复',
                                style: TextStyle(fontSize: 12),
                              ),
                            ),
                          ],
                        ),
                    ],
                  ),
                ),
              const Divider(height: 24),
              _section('WebDAV 远程备份 / 恢复'),
              _fieldRow('地址', 'webdav-url', _url),
              _fieldRow('用户名', 'webdav-user', _userName),
              _fieldRow('密码', 'webdav-pass', _password, obscure: true),
              _fieldRow('远程目录', 'webdav-dir', _dirName),
              Wrap(
                spacing: 8,
                runSpacing: 4,
                children: <Widget>[
                  OutlinedButton(
                    key: const ValueKey('webdav-save-btn'),
                    onPressed: state.busy
                        ? null
                        : () => controller.saveWebdav(_webdav),
                    child: const Text('保存配置', style: TextStyle(fontSize: 12)),
                  ),
                  OutlinedButton(
                    key: const ValueKey('webdav-check-btn'),
                    onPressed: state.busy
                        ? null
                        : () => controller.webdavCheck(_webdav),
                    child: const Text('检查连接', style: TextStyle(fontSize: 12)),
                  ),
                  OutlinedButton(
                    key: const ValueKey('webdav-list-btn'),
                    onPressed: state.busy
                        ? null
                        : () => controller.webdavList(_webdav),
                    child: const Text('列出远程', style: TextStyle(fontSize: 12)),
                  ),
                  OutlinedButton(
                    key: const ValueKey('webdav-backup-btn'),
                    onPressed: state.busy
                        ? null
                        : () => controller.webdavBackup(_webdav),
                    child: const Text('远程备份', style: TextStyle(fontSize: 12)),
                  ),
                  OutlinedButton(
                    key: const ValueKey('webdav-restore-btn'),
                    onPressed: state.busy
                        ? null
                        : () => controller.webdavRestore(_webdav),
                    child: const Text('远程恢复', style: TextStyle(fontSize: 12)),
                  ),
                ],
              ),
              if (state.remotes.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: Text(
                    '远程文件：${state.remotes.map((e) => e.href.split('/').last).join(', ')}',
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
              const Divider(height: 24),
              _section('维护'),
              Row(
                children: <Widget>[
                  OutlinedButton(
                    key: const ValueKey('backup-open-dir-btn'),
                    onPressed: controller.openConfigDir,
                    child: const Text('打开配置目录', style: TextStyle(fontSize: 12)),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton(
                    key: const ValueKey('backup-cleanup-btn'),
                    onPressed: controller.cleanupLogsTmp,
                    child: const Text(
                      '清理日志/临时文件',
                      style: TextStyle(fontSize: 12),
                    ),
                  ),
                ],
              ),
              if (state.coreVersions.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: Text(
                    '已安装内核：${state.coreVersions.map((e) => '${e.core} ${e.version}').join('，')}',
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
              const SizedBox(height: 8),
              _StatusArea(
                key: const ValueKey('backup-status'),
                busy: state.busy,
                status: state.status,
              ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: const ValueKey('backup-close'),
          onPressed: () => Navigator.pop(context),
          child: const Text('关闭'),
        ),
      ],
    );
  }

  String _bundleLabel(c.BackupManifestDto manifest) {
    final time = _formatEpoch(manifest.createdAt);
    final where = manifest.root ?? '(未提供路径)';
    return '$time · ${manifest.resourceCount} 项资源 · $where';
  }

  String _formatEpoch(Object? value) {
    final int seconds;
    if (value is BigInt) {
      seconds = value.toInt();
    } else if (value is num) {
      seconds = value.toInt();
    } else {
      return '未知时间';
    }
    if (seconds <= 0) return '未知时间';
    final local = DateTime.fromMillisecondsSinceEpoch(
      seconds * 1000,
      isUtc: true,
    ).toLocal();
    String two(int n) => n.toString().padLeft(2, '0');
    return '${local.year}-${two(local.month)}-${two(local.day)} '
        '${two(local.hour)}:${two(local.minute)}:${two(local.second)}';
  }

  Widget _section(String title) => Padding(
    padding: const EdgeInsets.only(bottom: 6),
    child: Text(
      title,
      style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
    ),
  );

  Widget _pathRow({
    required String label,
    required String fieldKey,
    required TextEditingController controller,
    required String buttonKey,
    required String buttonLabel,
    required VoidCallback onPressed,
  }) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 3),
    child: Row(
      children: <Widget>[
        SizedBox(
          width: 96,
          child: Text(label, style: const TextStyle(fontSize: 12)),
        ),
        Expanded(
          child: TextField(
            key: ValueKey(fieldKey),
            controller: controller,
            style: const TextStyle(fontSize: 12),
            decoration: const InputDecoration(
              isDense: true,
              border: OutlineInputBorder(),
            ),
          ),
        ),
        const SizedBox(width: 8),
        OutlinedButton(
          key: ValueKey(buttonKey),
          onPressed: onPressed,
          child: Text(buttonLabel, style: const TextStyle(fontSize: 12)),
        ),
      ],
    ),
  );

  Widget _fieldRow(
    String label,
    String fieldKey,
    TextEditingController controller, {
    bool obscure = false,
  }) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 3),
    child: Row(
      children: <Widget>[
        SizedBox(
          width: 96,
          child: Text(label, style: const TextStyle(fontSize: 12)),
        ),
        Expanded(
          child: TextField(
            key: ValueKey(fieldKey),
            controller: controller,
            obscureText: obscure,
            style: const TextStyle(fontSize: 12),
            decoration: const InputDecoration(
              isDense: true,
              border: OutlineInputBorder(),
            ),
          ),
        ),
      ],
    ),
  );
}

class _StatusArea extends StatelessWidget {
  const _StatusArea({super.key, required this.busy, required this.status});

  final bool busy;
  final BackupStatus? status;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final message = busy
        ? '处理中…'
        : status == null
        ? '就绪'
        : status!.detail == null
        ? status!.message
        : '${status!.message} — ${status!.detail}';
    final color = status?.isError == true
        ? scheme.error
        : status?.isSuccess == true
        ? scheme.primary
        : scheme.onSurfaceVariant;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      color: scheme.surfaceContainerHighest,
      child: Text(message, style: TextStyle(fontSize: 12, color: color)),
    );
  }
}
