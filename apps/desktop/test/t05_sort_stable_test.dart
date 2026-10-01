import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';

void main() {
  test('applySort is stable for equal keys (selection/anchor safety)', () {
    final rows = SyntheticBridgePort().generate(500);
    final originalIndex = <String, int>{
      for (var i = 0; i < rows.length; i++) rows[i].id: i,
    };
    final columns = defaultProfileColumns();

    final sorted = applySort(
      rows,
      columns,
      const SortSpec(
        columnKey: 'ConfigType',
        direction: SortDirection.ascending,
      ),
    );

    expect(sorted.length, rows.length);
    for (var i = 1; i < sorted.length; i++) {
      if (sorted[i].configType == sorted[i - 1].configType) {
        expect(
          originalIndex[sorted[i].id]! > originalIndex[sorted[i - 1].id]!,
          isTrue,
          reason: 'stable order broken at $i',
        );
      }
    }
  });
}
