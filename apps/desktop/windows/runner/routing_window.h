#ifndef RUNNER_ROUTING_WINDOW_H_
#define RUNNER_ROUTING_WINDOW_H_

#include <flutter/flutter_engine.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <memory>
#include <string>

#include "win32_window.h"

// A second, independent top-level window hosting its own Flutter engine that
// runs the `routingWindowMain` Dart entrypoint. Owned by the main window so it
// stays above it and does not get a taskbar button. Mirrors `SettingsWindow`
// (Wave L) for the routing settings window (R3-WPF-ROUTING-WINDOW).
class RoutingWindow : public Win32Window {
 public:
  RoutingWindow() = default;
  ~RoutingWindow() override = default;

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

#endif  // RUNNER_ROUTING_WINDOW_H_
