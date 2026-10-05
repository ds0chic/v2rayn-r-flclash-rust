// R3-VISUAL-DPI-TRAY: main shell at 200% DPI scaling.
import 'package:flutter_test/flutter_test.dart';

import 'support/dpi_assertions.dart';

void main() {
  testWidgets('main shell survives 200% DPI scaling', (tester) async {
    await runShellDpiCase(tester, 2.0);
  });
}
