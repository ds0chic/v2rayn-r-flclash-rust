// R3-SET-06: the main-menu subscription toast must reuse the real per-group
// report so a partial failure is never summarized as an all-success.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';

c.SubUpdateEntryDto _entry(String remarks, String status, {String? code}) =>
    c.SubUpdateEntryDto(
      subId: 'sub-$remarks',
      remarks: remarks,
      status: status,
      code: code,
    );

void main() {
  test('partial failure is summarized per group, not as all success', () {
    final result = c.SubUpdateResult(
      ok: true,
      success: 1,
      cancelled: false,
      entries: <c.SubUpdateEntryDto>[
        _entry('A', 'updated'),
        _entry('B', 'failed', code: 'E_TIMEOUT'),
        _entry('C', 'preserved_empty'),
        _entry('D', 'skipped'),
      ],
    );

    final summary = subsUpdateSummary(result, viaProxy: false);
    expect(summary, contains('成功 1'));
    expect(summary, contains('保留 1'));
    expect(summary, contains('失败 1'));
    expect(summary, contains('跳过 1'));
    expect(hasSubUpdateFailures(result), isTrue);
  });

  test('cancelled and proxy-unavailable paths stay explicit', () {
    final cancelled = c.SubUpdateResult(
      ok: false,
      success: 0,
      cancelled: true,
      entries: const <c.SubUpdateEntryDto>[],
    );
    expect(subsUpdateSummary(cancelled, viaProxy: false), '订阅更新已取消');

    final proxy = c.SubUpdateResult(
      ok: false,
      success: 0,
      cancelled: false,
      entries: const <c.SubUpdateEntryDto>[],
      error: const c.ErrorDto(
        code: 'E_PROXY_UNAVAILABLE',
        messageKey: 'error.proxy_unavailable',
        retryable: false,
      ),
    );
    expect(
      subsUpdateSummary(proxy, viaProxy: true),
      contains('E_PROXY_UNAVAILABLE'),
    );
    expect(hasSubUpdateFailures(proxy), isFalse);
  });
}
