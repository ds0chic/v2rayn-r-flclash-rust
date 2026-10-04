#include "routing_window_host.h"

#include <variant>

#include "runner_messages.h"

namespace {

// U+8DEF U+7531 U+8BBE U+7F6E = 路由设置.
constexpr wchar_t kRoutingTitle[] = L"\u8DEF\u7531\u8BBE\u7F6E";
constexpr UINT kRoutingOriginX = 120;
constexpr UINT kRoutingOriginY = 120;
constexpr UINT kRoutingWidth = 1000;
constexpr UINT kRoutingHeight = 700;

}  // namespace

RoutingWindowHost& RoutingWindowHost::Instance() {
  static RoutingWindowHost instance;
  return instance;
}

void RoutingWindowHost::Attach(flutter::FlutterEngine* main_engine,
                               HWND main_hwnd) {
  main_engine_ = main_engine;
  main_hwnd_ = main_hwnd;
  if (main_engine_ == nullptr) {
    return;
  }
  main_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          main_engine_->messenger(), "v2rayn/routing_window",
          &flutter::StandardMethodCodec::GetInstance());
  main_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) { HandleMainCall(call, std::move(result)); });
}

void RoutingWindowHost::HandleMainCall(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  const std::string& method = call.method_name();
  if (method == "open") {
    std::string snapshot = "{}";
    if (call.arguments() != nullptr) {
      if (const std::string* value = std::get_if<std::string>(call.arguments());
          value != nullptr) {
        snapshot = *value;
      }
    }
    OpenRoutingWindow(snapshot);
    result->Success(flutter::EncodableValue(routing_window_ != nullptr));
  } else if (method == "reportOutcome") {
    if (routing_channel_ != nullptr && call.arguments() != nullptr) {
      routing_channel_->InvokeMethod(
          "saveOutcome",
          std::make_unique<flutter::EncodableValue>(*call.arguments()));
    }
    result->Success();
  } else {
    result->NotImplemented();
  }
}

void RoutingWindowHost::HandleRoutingCall(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  const std::string& method = call.method_name();
  if (method == "ready") {
    result->Success(flutter::EncodableValue(snapshot_));
  } else if (method == "saveDraft") {
    if (main_channel_ != nullptr && call.arguments() != nullptr) {
      main_channel_->InvokeMethod(
          "applyDraft",
          std::make_unique<flutter::EncodableValue>(*call.arguments()));
    }
    result->Success();
  } else if (method == "close") {
    result->Success();
    if (routing_window_ != nullptr) {
      HWND handle = routing_window_->GetHandle();
      if (handle != nullptr) {
        ::PostMessage(handle, kRoutingCloseMessage, 0, 0);
      }
    }
  } else {
    result->NotImplemented();
  }
}

void RoutingWindowHost::OpenRoutingWindow(const std::string& snapshot) {
  if (routing_window_ != nullptr) {
    HWND existing = routing_window_->GetHandle();
    if (existing != nullptr && ::IsWindow(existing) != 0) {
      // Second open while the window is up: raise the existing window and keep
      // the in-progress draft rather than creating a duplicate.
      ::SetForegroundWindow(existing);
      return;
    }
    routing_channel_.reset();
    routing_window_.reset();
  }

  snapshot_ = snapshot;
  auto window = std::make_unique<RoutingWindow>();
  Win32Window::Point origin(kRoutingOriginX, kRoutingOriginY);
  Win32Window::Size size(kRoutingWidth, kRoutingHeight);
  if (!window->Create(kRoutingTitle, origin, size, main_hwnd_)) {
    return;
  }
  window->SetQuitOnClose(false);

  flutter::FlutterEngine* engine = window->engine();
  if (engine == nullptr) {
    return;
  }
  routing_window_ = std::move(window);
  routing_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          engine->messenger(), "v2rayn/routing_window",
          &flutter::StandardMethodCodec::GetInstance());
  routing_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) { HandleRoutingCall(call, std::move(result)); });

  // Upstream opens the window with `ShowDialog()`, which disables the owner.
  if (main_hwnd_ != nullptr) {
    ::EnableWindow(main_hwnd_, FALSE);
  }
}

void RoutingWindowHost::OnRoutingWindowDestroyed() {
  routing_channel_.reset();
  routing_window_.reset();
  if (main_hwnd_ != nullptr && ::IsWindow(main_hwnd_) != 0) {
    ::EnableWindow(main_hwnd_, TRUE);
    ::SetForegroundWindow(main_hwnd_);
  }
}
