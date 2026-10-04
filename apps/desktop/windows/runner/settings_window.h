#ifndef RUNNER_SETTINGS_WINDOW_H_
#define RUNNER_SETTINGS_WINDOW_H_

#include <flutter/flutter_engine.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <memory>
#include <string>

#include "win32_window.h"

// A second, independent top-level window hosting its own Flutter engine that
// runs the `settingsWindowMain` Dart entrypoint. Owned by the main window so
// it stays above it and does not get a taskbar button.
class SettingsWindow : public Win32Window {
 public:
  SettingsWindow() = default;
  ~SettingsWindow() override = default;

  flutter::FlutterEngine* engine() const {
    return flutter_controller_ ? flutter_controller_->engine() : nullptr;
  }

 protected:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT const message, WPARAM const wparam,
                         LPARAM const lparam) noexcept override;

 private:
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;
};

#endif  // RUNNER_SETTINGS_WINDOW_H_
