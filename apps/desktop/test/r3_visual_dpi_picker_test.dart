// R3-VISUAL-DPI-TRAY: the node picker (upstream ProfilesSelectWindow) must not
// overflow/truncate across the 100/125/150/200% DPI matrix. Synthetic nodes
// only; no native bridge, no network.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';

import 'support/dpi_assertions.dart';
import 'support/node_picker_harness.dart';

void main() {
  testWidgets('node picker survives the 100-200% DPI matrix', (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    applyDpi(tester, 1.0, const Size(1400, 1024));

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) => Center(
              child: ElevatedButton(
                onPressed: () async {
                  await showNodePicker(
                    context,
                    candidates: <c.ProfileDto>[
                      pickerNode('c1', remarks: 'HK-1', address: '192.0.2.1'),
                      pickerNode(
                        'c2',
                        remarks: 'US-1',
                        type: ConfigType.vmess,
                        subid: 'sub-2',
                        address: '192.0.2.2',
                        port: 80,
                        network: 'ws',
                      ),
                    ],
                    subItems: <c.SubItemDto>[
                      pickerSub('sub-1', 'A组'),
                      pickerSub('sub-2', 'B组'),
                    ],
                  );
                },
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-picker')), findsOneWidget);

    for (final scale in kDpiScales) {
      for (final logical in kLogicalSizes) {
        applyDpi(tester, scale, logical);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 40));
        expectNoLayoutProblems(
          tester,
          'node picker ${dpiLabel(scale, logical)}',
        );
      }
    }
  });
}
