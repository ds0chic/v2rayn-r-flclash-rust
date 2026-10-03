import 'package:file_selector/file_selector.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Native file dialogs for the backup window (ACT-WIN-007, F-BACKUP-001/002).
///
/// Upstream opens a `SaveFileDialog`/`OpenFileDialog` for `backup_*.zip`; this
/// seam keeps the same behaviour and stays injectable so widget tests never
/// touch the platform dialog. A `null` return means the user cancelled and the
/// caller must leave the current state untouched.
abstract class BackupPicker {
  /// Pick a local `backup_*.zip` archive to restore.
  Future<String?> pickArchive();

  /// Pick a project bundle directory to restore.
  Future<String?> pickDirectory();
}

class FileSelectorBackupPicker implements BackupPicker {
  const FileSelectorBackupPicker();

  @override
  Future<String?> pickArchive() async {
    const group = XTypeGroup(label: '备份文件', extensions: <String>['zip']);
    try {
      final file = await openFile(acceptedTypeGroups: <XTypeGroup>[group]);
      return file?.path;
    } catch (_) {
      return null;
    }
  }

  @override
  Future<String?> pickDirectory() async {
    try {
      return await getDirectoryPath(confirmButtonText: '选择');
    } catch (_) {
      return null;
    }
  }
}

final backupPickerProvider = Provider<BackupPicker>(
  (ref) => const FileSelectorBackupPicker(),
);
