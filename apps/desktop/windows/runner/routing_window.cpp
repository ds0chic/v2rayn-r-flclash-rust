#include "routing_window.h"

#include <flutter/dart_project.h>

#include <optional>

#include "runner_messages.h"

bool RoutingWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  flutter::DartProject project(L"data");
  project.set_dart_entrypoint("routingWindowMain");

  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project);
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  // The routing UI is pure Flutter and talks to the main engine over a channel;
  // it must not touch the Rust bridge, so no plugins are registered here.
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() { this->Show(); });
  flutter_controller_->ForceRedraw();
  return true;
}

void RoutingWindow::OnDestroy() {
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }
  Win32Window::OnDestroy();
}

LRESULT
RoutingWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  if (flutter_controller_) {
    std::optional<LRESULT> result = flutter_controller_->HandleTopLevelWindowProc(
        hwnd, message, wparam, lparam);
    if (result) {
      return *result;
    }
  }

  if (message == kRoutingCloseMessage) {
    // Posted by the host so the window destroys itself outside of a method
    // channel handler (destroying the engine from within its own handler is
    // unsafe).
    DestroyWindow(hwnd);
    return 0;
  }

  HWND owner = GetWindow(hwnd, GW_OWNER);
  LRESULT handled = Win32Window::MessageHandler(hwnd, message, wparam, lparam);
  if (message == WM_DESTROY && owner != nullptr) {
    // Notify the host once the HWND is gone; it owns this object and will
    // release it, re-enabling the main window.
    ::PostMessage(owner, kRoutingClosedMessage, 0, 0);
  }
  return handled;
}
