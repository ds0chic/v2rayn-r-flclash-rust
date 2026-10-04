import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';

void main() {
  final rows = SyntheticBridgePort().generate(1000);

  test('default column set matches LAY-PROFILES-002', () {
    final columns = defaultProfileColumns();
    // Stable internal ExName keys drive persistence/sort/FRB.
    expect(columns.map((c) => c.key).toList(), <String>[
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
    // Display labels come from ResUI.zh-Hans.resx, never used as storage keys.
    expect(columns.map((c) => c.title).toList(), <String>[
      '类型',
      '别名',
      '地址',
      '端口',
      '传输协议',
      'TLS',
      '订阅分组',
      '延迟 (ms)',
      '速度 (MB/s)',
      '今日上传',
      'IP 信息',
      '今日下载',
      '总上传',
      '总下载',
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

  test('SortSpec toggles the same column two-way, resets on a new column', () {
    var spec = const SortSpec();
    spec = spec.next('Port');
    expect(spec.direction, SortDirection.ascending);
    spec = spec.next('Port');
    expect(spec.direction, SortDirection.descending);
    spec = spec.next('Port');
    expect(
      spec.direction,
      SortDirection.ascending,
      reason: 'same-column repeat flips two-way (upstream SortServer)',
    );
    // A different column starts fresh at ascending.
    spec = spec.next('Remarks');
    expect(spec.columnKey, 'Remarks');
    expect(spec.direction, SortDirection.ascending);
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
