import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

/// UI-state backing for the first-run wizard and the "don't ask again"
/// suppressions (U-B1). All keys live in `ui_state.json`, isolated from the
/// business settings document: resetting hints never touches persisted
/// proxy configuration, layout or density.
class OnboardingHints {
  const OnboardingHints._();

  /// Completion flag of the first-run wizard.
  static const String onboardingSection = 'onboarding';

  /// Per-dialog "don't ask again" suppressions, keyed by dialog id.
  static const String confirmSection = 'confirm_hints';

  static bool isCompleted(UiStateStore store) =>
      store.loadSection(onboardingSection)?['completed'] == true;

  static void setCompleted(UiStateStore store, bool completed) {
    final next = Map<String, dynamic>.of(
      store.loadSection(onboardingSection) ?? const <String, dynamic>{},
    );
    next['completed'] = completed;
    store.saveSection(onboardingSection, next);
  }

  static bool isSuppressed(UiStateStore store, String key) =>
      store.loadSection(confirmSection)?[key] == true;

  static void setSuppressed(UiStateStore store, String key, bool suppressed) {
    final next = Map<String, dynamic>.of(
      store.loadSection(confirmSection) ?? const <String, dynamic>{},
    );
    next[key] = suppressed;
    store.saveSection(confirmSection, next);
  }

  /// Reset hint-class state only: confirm suppressions are cleared and the
  /// wizard becomes eligible to auto-show again. Density, layout, theme and
  /// every business setting are left untouched.
  static void resetHints(UiStateStore store) {
    store.saveSection(confirmSection, <String, dynamic>{});
    setCompleted(store, false);
  }
}
