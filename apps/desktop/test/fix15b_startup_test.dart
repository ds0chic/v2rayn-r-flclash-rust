import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/desktop_integration.dart';

/// FIX-15B: AutoHideStartup decision used by `DesktopIntegration.start()`.
///
/// Upstream (`MainWindow.xaml.cs:146/321`) reads `_config.UiItem.AutoHideStartup`
/// once: the ctor minimizes the window and `OnLoaded` calls
/// `ShowHideWindow(false)`. The clone must translate that into "do not surface
/// the window on launch, only the tray".
void main() {
  group('FIX-15B AutoHideStartup startup decision', () {
    test('hides on startup only when UiItem.AutoHideStartup is true', () {
      expect(
        DesktopIntegration.shouldHideOnStartup(<String, dynamic>{
          'UiItem': <String, dynamic>{'AutoHideStartup': true},
        }),
        isTrue,
      );
      expect(
        DesktopIntegration.shouldHideOnStartup(<String, dynamic>{
          'UiItem': <String, dynamic>{'AutoHideStartup': false},
        }),
        isFalse,
      );
    });

    test('normal launch (absent/malformed field) does not hide', () {
      expect(DesktopIntegration.shouldHideOnStartup(const {}), isFalse);
      expect(
        DesktopIntegration.shouldHideOnStartup(const {'UiItem': 'not-a-map'}),
        isFalse,
      );
      expect(
        DesktopIntegration.shouldHideOnStartup(const {
          'UiItem': <String, dynamic>{},
        }),
        isFalse,
      );
      // Non-bool JSON values must not be coerced.
      expect(
        DesktopIntegration.shouldHideOnStartup(const {
          'UiItem': <String, dynamic>{'AutoHideStartup': 1},
        }),
        isFalse,
      );
    });

    test('close flags stay independent', () {
      final behavior = CloseBehavior.fromDocument(const {
        'UiItem': <String, dynamic>{
          'Hide2TrayWhenClose': true,
          'AutoHideStartup': true,
        },
      });
      expect(behavior.hide2TrayWhenClose, isTrue);
      expect(behavior.autoHideStartup, isTrue);
    });
  });
}
