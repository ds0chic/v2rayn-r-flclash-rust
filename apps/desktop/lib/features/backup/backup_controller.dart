import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/backup/backup_picker.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

final backupControllerProvider =
    NotifierProvider<BackupController, BackupState>(BackupController.new);

const c.WebDavConfigDto emptyWebDavConfig = c.WebDavConfigDto(
  url: '',
  userName: '',
  password: '',
  dirName: 'v2rayN_backup',
);

/// A structured message for the backup window.
class BackupStatus {
  const BackupStatus({required this.kind, required this.message, this.detail});

  /// `info` / `success` / `error`.
  final String kind;
  final String message;
  final String? detail;

  bool get isError => kind == 'error';
  bool get isSuccess => kind == 'success';
}

class BackupState {
  const BackupState({
    this.busy = false,
    this.status,
    this.bundles = const <c.BackupManifestDto>[],
    this.webdav = emptyWebDavConfig,
    this.webdavRevision = 0,
    this.remotes = const <c.WebDavEntryDto>[],
    this.coreVersions = const <c.InstalledCoreDto>[],
  });

  final bool busy;
  final BackupStatus? status;
  final List<c.BackupManifestDto> bundles;
  final c.WebDavConfigDto webdav;
  final int webdavRevision;
  final List<c.WebDavEntryDto> remotes;
  final List<c.InstalledCoreDto> coreVersions;

  BackupState copyWith({
    bool? busy,
    BackupStatus? status,
    List<c.BackupManifestDto>? bundles,
    c.WebDavConfigDto? webdav,
    int? webdavRevision,
    List<c.WebDavEntryDto>? remotes,
    List<c.InstalledCoreDto>? coreVersions,
  }) => BackupState(
    busy: busy ?? this.busy,
    status: status ?? this.status,
    bundles: bundles ?? this.bundles,
    webdav: webdav ?? this.webdav,
    webdavRevision: webdavRevision ?? this.webdavRevision,
    remotes: remotes ?? this.remotes,
    coreVersions: coreVersions ?? this.coreVersions,
  );
}

/// Backup / restore / WebDAV window controller (F-BACKUP-001..004).
class BackupController extends Notifier<BackupState> {
  @override
  BackupState build() {
    final bridge = ref.read(bridgePortProvider);
    final config = bridge.t16WebdavConfigGet();
    return BackupState(
      webdav: config.config ?? emptyWebDavConfig,
      webdavRevision: config.revision.toInt(),
      coreVersions: bridge.t16GetCoreVersions().items,
    );
  }

  void _status(String kind, String message, {String? detail}) {
    state = state.copyWith(
      status: BackupStatus(kind: kind, message: message, detail: detail),
    );
  }

  String _detail(c.ErrorDto? error) {
    if (error == null) return 'unknown';
    return error.detail == null
        ? '${error.code} / ${error.messageKey}'
        : '${error.code} / ${error.messageKey}: ${error.detail}';
  }

  /// WebDAV failures are mapped to a concrete branch (401/403, 404, timeout,
  /// unreachable). The code is kept so the branch stays diagnosable; remote
  /// status text never carries credentials.
  String _webdavDetail(c.ErrorDto? error) {
    if (error == null) return 'unknown';
    final branch = switch (error.code) {
      'E_PERMISSION_DENIED' => '认证失败（401/403），请检查用户名/密码',
      'E_NOT_FOUND' => '远端路径不存在（404）',
      'E_TIMEOUT' => '连接超时',
      'E_UNAVAILABLE' => '无法连接远端（网络或代理不可达）',
      _ => null,
    };
    final base = '${error.code} / ${error.messageKey}';
    if (branch == null) {
      return error.detail == null ? base : '$base: ${error.detail}';
    }
    return '$base: $branch';
  }

  void reloadBundles(String parent) {
    final result = ref.read(bridgePortProvider).t16BackupList(parent);
    if (result.error != null) {
      _status('error', '列出备份失败', detail: _detail(result.error));
      return;
    }
    state = state.copyWith(bundles: result.items);
  }

  void localBackup(String destRoot) {
    if (destRoot.trim().isEmpty) {
      _status('error', '请输入备份目标目录');
      return;
    }
    final result = ref.read(bridgePortProvider).t16BackupLocal(destRoot.trim());
    if (!result.ok) {
      _status('error', '本地备份失败', detail: _detail(result.error));
      return;
    }
    state = state.copyWith(
      bundles: result.manifest == null
          ? state.bundles
          : <c.BackupManifestDto>[result.manifest!, ...state.bundles],
    );
    _status('success', '本地备份完成：${result.root ?? destRoot}');
  }

  /// Re-read engine-owned settings so the window never keeps serving stale
  /// state after a restore swapped the on-disk config (`quiesce`/`reopen`).
  void _refreshAfterRestore() {
    final bridge = ref.read(bridgePortProvider);
    final config = bridge.t16WebdavConfigGet();
    final cores = bridge.t16GetCoreVersions();
    state = state.copyWith(
      webdav: config.config ?? state.webdav,
      webdavRevision: config.revision.toInt(),
      coreVersions: cores.items,
    );
  }

  bool _restoreBundlePath(String bundleDir) {
    final result = ref.read(bridgePortProvider).t16BackupRestore(bundleDir);
    if (!result.ok) {
      _status('error', '本地恢复失败（已保留现有配置）', detail: _detail(result.error));
      return false;
    }
    _refreshAfterRestore();
    _status('success', '本地恢复完成（配置与资源已重载，请重开窗口查看）：${result.message}');
    return true;
  }

  void restoreBundle(String bundleDir) {
    if (bundleDir.trim().isEmpty) {
      _status('error', '请选择备份包目录');
      return;
    }
    _restoreBundlePath(bundleDir.trim());
  }

  /// Pick a local `backup_*.zip` with the native dialog and restore it. A
  /// cancelled picker is a no-op. An upstream ZIP is imported through the
  /// candidate + activation flow (`quiesce` -> activate -> `reopen`), so the
  /// restored settings/active node take effect on the next load.
  Future<void> restoreFromArchive() async {
    final path = await ref.read(backupPickerProvider).pickArchive();
    if (path == null || path.trim().isEmpty) return;
    _restoreArchivePath(path.trim());
  }

  void _restoreArchivePath(String path) {
    final bridge = ref.read(bridgePortProvider);
    final recognition = bridge.t16BackupRecognize(path);
    if (recognition.error != null) {
      _status('error', '无法读取备份文件（现有配置未修改）', detail: _detail(recognition.error));
      return;
    }
    if (!recognition.isUpstream) {
      _status('error', '不是可恢复的备份文件（缺少配置或数据库，现有配置未修改）');
      return;
    }
    final result = bridge.t16BackupImportUpstream(path);
    if (!result.ok) {
      _status('error', '本地恢复失败（已保留现有配置）', detail: _detail(result.error));
      return;
    }
    _refreshAfterRestore();
    _status('success', '本地恢复完成（配置与资源已重载，请重开窗口查看）：${result.status}');
  }

  /// Pick a project bundle directory with the native dialog and restore it via
  /// the same quiesce -> swap -> reopen path as FIX-14.
  Future<void> restoreFromDirectory() async {
    final path = await ref.read(backupPickerProvider).pickDirectory();
    if (path == null || path.trim().isEmpty) return;
    restoreBundle(path.trim());
  }

  /// Restore one bundle from the discovered list. Real `backup_list` items may
  /// not carry their root path yet; those are surfaced as an error instead of
  /// guessing a directory.
  void restoreListed(c.BackupManifestDto manifest) {
    final root = manifest.root;
    if (root == null || root.trim().isEmpty) {
      _status('error', '该备份未提供路径，无法直接恢复');
      return;
    }
    restoreBundle(root);
  }

  void recognize(String path) {
    if (path.trim().isEmpty) {
      _status('error', '请输入要识别的 ZIP 路径');
      return;
    }
    final result = ref.read(bridgePortProvider).t16BackupRecognize(path.trim());
    if (result.error != null) {
      _status('error', '识别失败', detail: _detail(result.error));
      return;
    }
    _status(
      'info',
      result.isUpstream
          ? '识别为原版备份（${result.layout}），配置=${result.hasConfig}，数据库=${result.hasDb}'
          : '不是可识别的原版备份',
    );
  }

  void importUpstream(String path) {
    if (path.trim().isEmpty) {
      _status('error', '请输入要导入的 ZIP 路径');
      return;
    }
    final result = ref
        .read(bridgePortProvider)
        .t16BackupImportUpstream(path.trim());
    if (!result.ok) {
      _status('error', '导入失败', detail: _detail(result.error));
      return;
    }
    _status(
      'success',
      '导入完成：${result.status}，导入 ${result.importedRows} 行，原版设置/活动节点已激活',
    );
  }

  void saveWebdav(c.WebDavConfigDto config) {
    final result = ref
        .read(bridgePortProvider)
        .t16WebdavConfigSave(config, state.webdavRevision);
    if (!result.ok) {
      _status('error', '保存 WebDAV 配置失败', detail: _detail(result.error));
      return;
    }
    state = state.copyWith(
      webdav: result.config ?? config,
      webdavRevision: result.revision.toInt(),
    );
    _status('success', 'WebDAV 配置已保存');
  }

  Future<void> webdavCheck(c.WebDavConfigDto config) async {
    state = state.copyWith(busy: true);
    final result = await ref.read(bridgePortProvider).t16WebdavCheck(config);
    state = state.copyWith(busy: false);
    if (!result.ok) {
      _status('error', 'WebDAV 连接失败', detail: _webdavDetail(result.error));
      return;
    }
    _status('success', result.createdDir ? 'WebDAV 已连接（已创建目录）' : 'WebDAV 连接正常');
  }

  Future<void> webdavList(c.WebDavConfigDto config) async {
    state = state.copyWith(busy: true);
    final result = await ref.read(bridgePortProvider).t16WebdavList(config);
    state = state.copyWith(busy: false);
    if (!result.ok) {
      _status('error', '列出远程目录失败', detail: _webdavDetail(result.error));
      return;
    }
    state = state.copyWith(remotes: result.items);
    _status('success', '远程目录：${result.items.length} 项');
  }

  Future<void> webdavBackup(c.WebDavConfigDto config) async {
    state = state.copyWith(busy: true);
    final bridge = ref.read(bridgePortProvider);
    final result = await bridge.t16WebdavBackup(config);
    if (!result.ok) {
      state = state.copyWith(busy: false);
      _status('error', '远程备份失败', detail: _webdavDetail(result.error));
      return;
    }
    // The upload only succeeds when the PUT was accepted; surface the file that
    // is now actually on the remote so "上传 -> 远端出现" is visible.
    final remote = await bridge.t16WebdavList(config);
    final entries = remote.ok ? remote.items : state.remotes;
    state = state.copyWith(busy: false, remotes: entries);
    _status('success', '远程备份完成：${result.bytes} 字节');
  }

  Future<void> webdavRestore(c.WebDavConfigDto config) async {
    state = state.copyWith(busy: true);
    final result = await ref.read(bridgePortProvider).t16WebdavRestore(config);
    state = state.copyWith(busy: false);
    if (!result.ok) {
      _status('error', '远程恢复失败（已保留现有配置）', detail: _webdavDetail(result.error));
      return;
    }
    _refreshAfterRestore();
    _status('success', '远程恢复完成（配置与资源已重载，请重开窗口查看）：${result.message}');
  }

  void openConfigDir() {
    final result = ref.read(bridgePortProvider).t16OpenConfigDir();
    _status(
      result.ok ? 'success' : 'error',
      result.ok ? '已打开配置目录' : '无法打开配置目录',
      detail: result.ok ? null : _detail(result.error),
    );
  }

  void cleanupLogsTmp() {
    final result = ref.read(bridgePortProvider).t16CleanupLogsTmp();
    if (!result.ok) {
      _status('error', '清理失败', detail: _detail(result.error));
      return;
    }
    _status(
      'success',
      '已清理 ${result.deleted} 个文件（${result.bytes} 字节），跳过 ${result.skipped} 个',
    );
  }
}
