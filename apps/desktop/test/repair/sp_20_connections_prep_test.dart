// SP-20 准备范围（identified）：连接表列布局/右键关闭重开保持的独立部分。
//
// 原版对照（v2rayN 7.25.4 / 7d6a967）：
// `v2rayN/Views/ClashConnectionsView.xaml` 默认列宽
// Host=300 / Chain=500 / Network=80 / Type=160 / ProcessPath=100 / Elapsed=100；
// `ClashConnectionsView.xaml.cs` RestoreUI 按 ConnectionsColumnItem.Index 排序恢复
// （Width>0 才覆盖），StorageUI 在退出时按 Name/ActualWidth/DisplayIndex 回写；
// `ServiceLib/ViewModels/ClashConnectionsViewModel.cs` 右键关闭冻结
// SelectedSource.Id（空 Id 不可执行），关闭全部走独立空 id 路径，关闭后重查。
// 会话 generation 语义（换 session 迟到不误关）完整实现依赖 SP-17（A08 在途），
// 本卡只做请求冻结/过期丢弃的纯合同，不触达真实网络与宿主代理。
//
// 合成数据：全部使用 `*.example.invalid` 合成 host，不做任何 socket/HTTP 调用。
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/monitor/connections_columns.dart';

void main() {
  group('SP-20 prep: upstream column defaults', () {
    test('canonical columns match ClashConnectionsView.xaml widths', () {
      final defaults = defaultConnectionColumns();
      expect(defaults.map((c) => c.name).toList(), <String>[
        'Host',
        'Chain',
        'Network',
        'Type',
        'ProcessPath',
        'Elapsed',
      ]);
      expect(defaults.map((c) => c.width).toList(), <int>[
        300,
        500,
        80,
        160,
        100,
        100,
      ]);
      expect(defaults.map((c) => c.index).toList(), <int>[0, 1, 2, 3, 4, 5]);
    });
  });

  group('SP-20 prep: layout normalize/persist roundtrip', () {
    test('orders by Index, drops unknown, clamps width, fills missing', () {
      final resolved = resolveVisibleColumns(<Map<String, dynamic>>[
        {'Name': 'Elapsed', 'Width': 120, 'Index': 0},
        {'Name': 'Nope', 'Width': 50, 'Index': 1},
        {'Name': 'Host', 'Width': 0, 'Index': 5},
        {'Name': 'Chain', 'Width': -3, 'Index': 2},
      ]);
      // Elapsed first (Index 0); Chain keeps canonical width on bad input;
      // Host keeps canonical width on Width<=0; missing Network/Type/
      // ProcessPath appended in canonical order.
      expect(resolved.map((c) => c.name).toList(), <String>[
        'Elapsed',
        'Chain',
        'Host',
        'Network',
        'Type',
        'ProcessPath',
      ]);
      expect(resolved.map((c) => c.index).toList(), <int>[0, 1, 2, 3, 4, 5]);
      final byName = {for (final c in resolved) c.name: c.width};
      expect(byName['Elapsed'], 120);
      expect(byName['Chain'], 500);
      expect(byName['Host'], 300);
    });

    test('storage roundtrip preserves order for independent reopen', () {
      final resolved = resolveVisibleColumns(<Map<String, dynamic>>[
        {'Name': 'Type', 'Width': 170, 'Index': 0},
        {'Name': 'Host', 'Width': 310, 'Index': 1},
      ]);
      final stored = connectionColumnsToStorage(resolved);
      expect(stored[0]['Name'], 'Type');
      expect(stored[0]['Width'], 170);
      expect(stored[0]['Index'], 0);
      // Simulate an independent reopen: feed the stored rows back.
      final reopened = resolveVisibleColumns(stored);
      expect(
        reopened.map((c) => c.name).toList(),
        resolved.map((c) => c.name).toList(),
      );
      expect(
        reopened.map((c) => c.width).toList(),
        resolved.map((c) => c.width).toList(),
      );
    });

    test('empty persisted layout falls back to upstream defaults', () {
      final resolved = resolveVisibleColumns(const <Map<String, dynamic>>[]);
      expect(
        resolved.map((c) => c.name).toList(),
        defaultConnectionColumns().map((c) => c.name).toList(),
      );
    });
  });

  group('SP-20 prep: close target freeze (no network)', () {
    test('empty id never freezes a single-close request', () {
      expect(freezeConnectionClose('', 7), isNull);
      expect(freezeConnectionClose(null, 7), isNull);
    });

    test('late response from an older session is dropped', () {
      final request = freezeConnectionClose('conn-1', 7)!;
      expect(request.id, 'conn-1');
      expect(request.generation, 7);
      expect(closeResponseIsCurrent(request.generation, 7), isTrue);
      // Session switched while the close was in flight: do not apply.
      expect(closeResponseIsCurrent(request.generation, 8), isFalse);
    });

    test('close-all uses its own path, never an empty single id', () {
      expect(isCloseAllRequest(closeAll: true, id: ''), isTrue);
      expect(isCloseAllRequest(closeAll: false, id: ''), isFalse);
      expect(isCloseAllRequest(closeAll: false, id: 'conn-1'), isFalse);
    });
  });

  group('SP-20 prep: synthetic connection fixture (no sockets)', () {
    test('1k synthetic rows are unique and host-only synthetic', () {
      final rows = syntheticConnections(1000);
      expect(rows.length, 1000);
      expect(rows.map((r) => r.id).toSet().length, 1000);
      for (final row in rows) {
        expect(row.host, contains('.example.invalid'));
      }
      // Layout + filter path handles the full synthetic page without network.
      final visible = resolveVisibleColumns(const <Map<String, dynamic>>[]);
      expect(visible.length, 6);
    });

    test('10k synthetic rows keep unique ids', () {
      final rows = syntheticConnections(10000);
      expect(rows.length, 10000);
      expect(rows.map((r) => r.id).toSet().length, 10000);
    });
  });
}
