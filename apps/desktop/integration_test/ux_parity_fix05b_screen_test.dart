// FIX-05B (ACT-MAIN-017): real-window integration for the screen QR scan.
//
// This does not use the injectable fakes: it captures the real primary monitor
// and exercises the real `window_manager`-backed hide/show control. It needs a
// real Windows desktop session (`flutter test integration_test/... -d windows`)
// and is expected to be run by the root agent when the environment is stable.
import 'package:flutter_test/flutter_test.dart';
import 'package:image/image.dart' as img;
import 'package:integration_test/integration_test.dart';
import 'package:v2rayn_desktop/features/subs/scan_screen_qr.dart';
import 'package:window_manager/window_manager.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('real screen capture returns a decodable PNG', (tester) async {
    final bytes = await capturePrimaryScreenPng();
    expect(bytes, isNotEmpty);
    final decoded = img.decodeImage(bytes);
    expect(decoded, isNotNull);
    expect(decoded!.width, greaterThan(0));
    expect(decoded.height, greaterThan(0));
  });

  testWidgets('window control hides then restores the real window', (
    tester,
  ) async {
    await windowManager.ensureInitialized();
    const window = WindowManagerScanWindowControl();
    await window.hide();
    expect(await windowManager.isVisible(), isFalse);
    await window.show();
    expect(await windowManager.isVisible(), isTrue);
  });
}
