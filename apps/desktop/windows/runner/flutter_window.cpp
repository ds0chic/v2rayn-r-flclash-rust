#include "flutter_window.h"

#include <optional>

#include "flutter/generated_plugin_registrant.h"
#include "option_window_host.h"
#include "routing_window_host.h"
#include "runner_messages.h"

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  // Bind the option-settings window host to this engine so the menu can spawn
  // the independent top-level settings window.
  OptionWindowHost::Instance().Attach(flutter_controller_->engine(),
                                      GetHandle());
  // Bind the routing-settings window host for the independent top-level
  // routing window.
  RoutingWindowHost::Instance().Attach(flutter_controller_->engine(),
                                       GetHandle());
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case kShowWindowMessage:
      // Second instance asked to restore the window (ROOT-06 / ACT-WIN-012).
      ShowWindow(GetHandle(), SW_RESTORE);
      SetForegroundWindow(GetHandle());
      return 0;
    case kSettingsClosedMessage:
      // The settings window finished destroying itself; release it and
      // re-enable this (owner) window.
      OptionWindowHost::Instance().OnSettingsWindowDestroyed();
      return 0;
    case kRoutingClosedMessage:
      // The routing window finished destroying itself; release it and
      // re-enable this (owner) window.
      RoutingWindowHost::Instance().OnRoutingWindowDestroyed();
      return 0;
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
