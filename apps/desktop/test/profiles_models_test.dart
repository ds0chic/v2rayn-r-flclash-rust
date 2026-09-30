import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';

void main() {
  final rows = SyntheticBridgePort().generate(1000);

  test('default column set matches LAY-PROFILES-002', () {
    final columns = defaultProfileColumns();
    expect(columns.map((c) => c.title).toList(), <String>[
      'ConfigType',
      'Remarks',
      'Address',
      'Port',
      'Network',
      'StreamSecurity',
      'SubRemarks',
      'DelayVal',
      'SpeedVal',
      'TodayUp',
      'IpInfo',
      'TodayDown',
      'TotalUp',
      'TotalDown',
    ]);
    expect(columns.length, 14);
  });

  test('filter is instant and matches remarks/address', () {
    final filtered = applyFilter(rows, 'Synthetic-00001');
    expect(filtered, isNotEmpty);
    expect(
      filtered.every((r) => r.remarks.contains('Synthetic-00001')),
      isTrue,
    );
  });

  test('filter with empty query returns all rows', () {
    expect(applyFilter(rows, '  ').length, rows.length);
  });

  test('sort by delay ascending then descending', () {
    final columns = defaultProfileColumns();
    final asc = applySort(
      rows,
      columns,
      const SortSpec(columnKey: 'DelayVal', direction: SortDirection.ascending),
    );
    final desc = applySort(
      rows,
      columns,
      const SortSpec(
        columnKey: 'DelayVal',
        direction: SortDirection.descending,
      ),
    );
    expect(asc.first.delay <= asc.last.delay, isTrue);
    expect(desc.first.delay >= desc.last.delay, isTrue);
  });

  test('SortSpec cycles asc -> desc -> none', () {
    var spec = const SortSpec();
    spec = spec.next('Port');
    expect(spec.direction, SortDirection.ascending);
    spec = spec.next('Port');
    expect(spec.direction, SortDirection.descending);
    spec = spec.next('Port');
    expect(spec.direction, SortDirection.none);
  });

  test('formatBytes renders human readable units', () {
    expect(formatBytes(BigInt.zero), '0 B');
    expect(formatBytes(BigInt.from(1073741824)), '1.0 GB');
  });

  test('synthetic source yields 10k rows with reserved ranges', () {
    final tenK = SyntheticBridgePort().generate(10000);
    expect(tenK.length, 10000);
    expect(tenK.first.id, 'syn-000000');
    expect(tenK.last.id, 'syn-009999');
    for (final row in tenK) {
      final addressOk =
          row.address.startsWith('192.0.2.') ||
          row.address.startsWith('198.51.100.') ||
          row.address.startsWith('203.0.113.') ||
          row.address.contains('.example.com') ||
          row.address.contains('.example.org') ||
          row.address.contains('.example.net');
      expect(addressOk, isTrue, reason: 'unexpected address ${row.address}');
    }
  });
}
