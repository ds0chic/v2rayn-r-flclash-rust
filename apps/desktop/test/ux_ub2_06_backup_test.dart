import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';

import 't16_backup_test.dart' show makeContainer, pumpBackup;

void main() {
  testWidgets('UB2-06: destructive entries warn backup-first', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    // Restore (zip/dir/remote) + upstream-ZIP import all warn first.
    expect(find.textContaining('先本地备份'), findsNWidgets(3));
  });

  testWidgets('UB2-06: webdav secret and dir carry risk notes', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    expect(find.textContaining('明文保存'), findsOneWidget);
    expect(find.textContaining('待核实'), findsOneWidget);
  });
}
