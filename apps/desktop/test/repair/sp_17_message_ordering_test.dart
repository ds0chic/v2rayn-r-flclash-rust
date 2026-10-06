import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';

/// SP-17 red contract: message ordering on the status bar. A current runtime
/// failure is the headline; an older platform message must not cover it and is
/// demoted to the secondary slot. With no runtime failure the platform message
/// is the headline again. Availability notices carry time/severity/op.
void main() {
  RuntimeErrorView runtimeError() => const RuntimeErrorView(
    code: 'E_CORE_START_FAILED',
    messageKey: 'error.core_start_failed',
    operationId: 'op-b-1',
  );

  test(
    'current runtime failure is the headline, stale platform msg demoted',
    () {
      final resolved = resolveStatusMessages(
        runtimeError: runtimeError(),
        runtimeNotice: null,
        platformErrorText: null,
        platformMessage: '旧平台消息：系统代理已切换',
        shellMessage: '旧shell消息',
      );

      expect(resolved.headlineKind, StatusHeadlineKind.runtimeError);
      expect(resolved.headlineText, contains('core_start_failed'));
      expect(resolved.headlineText, isNot(contains('旧平台消息')));
      expect(resolved.secondaryTexts.join('\n'), contains('旧平台消息'));
      // A failure headline always offers retry, never a bare label.
      expect(resolved.headlineRetryable, isTrue);
    },
  );

  test('platform message is the headline when no runtime failure exists', () {
    final resolved = resolveStatusMessages(
      runtimeError: null,
      runtimeNotice: null,
      platformErrorText: null,
      platformMessage: '系统代理: 直接连接',
      shellMessage: 'shell消息',
    );

    expect(resolved.headlineKind, StatusHeadlineKind.info);
    expect(resolved.headlineText, contains('系统代理'));
  });

  test('availability notice carries time/severity/operation identity', () {
    const notice = RuntimeNotice(
      text: '可用性 127.0.0.1:11911 可达 3ms',
      severity: RuntimeNoticeSeverity.info,
      atMs: 1728288000000,
      operationId: 'avail-1',
    );
    final resolved = resolveStatusMessages(
      runtimeError: null,
      runtimeNotice: notice,
      platformErrorText: null,
      platformMessage: null,
      shellMessage: null,
    );

    expect(resolved.headlineKind, StatusHeadlineKind.info);
    expect(resolved.headlineText, contains('11911'));
    expect(notice.atMs, 1728288000000);
    expect(notice.operationId, 'avail-1');
    expect(notice.severity, RuntimeNoticeSeverity.info);
  });

  test('platform error stays visible next to a runtime failure', () {
    final resolved = resolveStatusMessages(
      runtimeError: runtimeError(),
      runtimeNotice: null,
      platformErrorText: 'error.platform_backend',
      platformMessage: '旧平台消息',
      shellMessage: null,
    );

    expect(resolved.headlineKind, StatusHeadlineKind.runtimeError);
    expect(
      resolved.secondaryTexts.join('\n'),
      contains('error.platform_backend'),
    );
  });
}
