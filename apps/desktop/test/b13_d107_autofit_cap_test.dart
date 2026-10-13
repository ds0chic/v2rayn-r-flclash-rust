// B13 D1-07: the node-picker autofit must sample rows like the main table
// (500) instead of measuring every candidate on the UI thread.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';

void main() {
  test('capAutofitRows keeps the first 500 rows', () {
    final rows = List<int>.generate(10000, (i) => i);
    final sampled = capAutofitRows(rows);
    expect(sampled, hasLength(500));
    expect(sampled.first, 0);
    expect(sampled.last, 499);
  });

  test('capAutofitRows keeps small lists whole', () {
    final rows = List<int>.generate(37, (i) => i);
    expect(capAutofitRows(rows), hasLength(37));
  });
}
