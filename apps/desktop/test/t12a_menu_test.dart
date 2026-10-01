import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';

void main() {
  test('settings menu carries the T12a actions', () {
    final settings = mainMenuModel.firstWhere(
      (e) => e.actionId == 'UI-GROUP-SETTINGS',
    );
    final ids = <String>{
      for (final item in settings.submenu)
        if (item.actionId != null) item.actionId!,
    };
    for (final id in <String>[
      'ACT-MAIN-024', // 参数设置
      'ACT-MAIN-028', // 全局热键设置
      'UI-THEME-WINDOW',
      'ACT-MAIN-029', // 以管理员身份重启 (未接入)
      'ACT-WIN-004', // UWP 回环 (未接入)
    ]) {
      expect(ids, contains(id), reason: 'missing $id');
    }
  });
}
