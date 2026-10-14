// D-LEFT2: export/save failure toasts must show human text, never a raw key.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/subs/subs_actions.dart';

import 'support/subs_harness.dart';

class _FailExportBridge extends SyntheticBridgePort {
  _FailExportBridge() : super(count: 1);

  @override
  Future<c.ShareExportResult> exportProfiles(
    List<String> ids,
    String kind,
  ) async {
    return const c.ShareExportResult(
      ok: false,
      text: '',
      count: 0,
      error: c.ErrorDto(
        code: 'E_NOT_FOUND',
        messageKey: 'error.no_profiles_selected',
        retryable: false,
      ),
    );
  }
}

void main() {
  testWidgets('export failure toast shows human text, not the raw key', (
    tester,
  ) async {
    final container = makeSubsContainer(bridge: _FailExportBridge());
    addTearDown(container.dispose);
    container.read(profilesControllerProvider.notifier).selectRow('x');
    late BuildContext ctx;
    late WidgetRef ref;
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: RefProbe(
            onRef: (c, r) {
              ctx = c;
              ref = r;
            },
          ),
        ),
      ),
    );

    await exportProfiles(ctx, ref, kind: 'share');

    final message = container.read(uiShellControllerProvider).message;
    expect(message, contains('导出失败'));
    expect(message, isNot(contains('error.')));
  });
}
