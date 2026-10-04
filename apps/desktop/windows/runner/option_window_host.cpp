#include "option_window_host.h"

#include <variant>

#include "runner_messages.h"

namespace {

constexpr wchar_t kSettingsTitle[] = L"\u8BBE\u7F6E";
constexpr UINT kSettingsOriginX = 120;
constexpr UINT kSettingsOriginY = 120;
constexpr UINT kSettingsWidth = 1000;
constexpr UINT kSettingsHeight = 700;

}  // namespace

OptionWindowHost& OptionWindowHost::Instance() {
  static OptionWindowHost instance;
  return instance;
}

void OptionWindowHost::Attach(flutter::FlutterEngine* main_engine,
                              HWND main_hwnd) {
  main_engine_ = main_engine;
  main_hwnd_ = main_hwnd;
  if (main_engine_ == nullptr) {
    return;
  }
  main_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          main_engine_->messenger(), "v2rayn/option_window",
          &flutter::StandardMethodCodec::GetInstance());
  main_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) { HandleMainCall(call, std::move(result)); });
}

void OptionWindowHost::HandleMainCall(
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
    OpenSettingsWindow(snapshot);
    result->Success(flutter::EncodableValue(settings_window_ != nullptr));
  } else if (method == "reportOutcome") {
    if (settings_channel_ != nullptr && call.arguments() != nullptr) {
      settings_channel_->InvokeMethod(
          "saveOutcome",
          std::make_unique<flutter::EncodableValue>(*call.arguments()));
    }
    result->Success();
  } else {
    result->NotImplemented();
  }
}

void OptionWindowHost::HandleSettingsCall(
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
    if (settings_window_ != nullptr) {
      HWND handle = settings_window_->GetHandle();
      if (handle != nullptr) {
        ::PostMessage(handle, kSettingsCloseMessage, 0, 0);
      }
    }
  } else {
    result->NotImplemented();
  }
}

void OptionWindowHost::OpenSettingsWindow(const std::string& snapshot) {
  if (settings_window_ != nullptr) {
    HWND existing = settings_window_->GetHandle();
    if (existing != nullptr && ::IsWindow(existing) != 0) {
      // Second open while the window is up: raise the existing window and keep
      // the in-progress draft rather than creating a duplicate.
      ::SetForegroundWindow(existing);
      return;
    }
    settings_channel_.reset();
    settings_window_.reset();
  }

  snapshot_ = snapshot;
  auto window = std::make_unique<SettingsWindow>();
  Win32Window::Point origin(kSettingsOriginX, kSettingsOriginY);
  Win32Window::Size size(kSettingsWidth, kSettingsHeight);
  if (!window->Create(kSettingsTitle, origin, size, main_hwnd_)) {
    return;
  }
  window->SetQuitOnClose(false);

  flutter::FlutterEngine* engine = window->engine();
  if (engine == nullptr) {
    return;
  }
  settings_window_ = std::move(window);
  settings_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          engine->messenger(), "v2rayn/option_window",
          &flutter::StandardMethodCodec::GetInstance());
  settings_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) { HandleSettingsCall(call, std::move(result)); });

  // Upstream opens the window with `ShowDialog()`, which disables the owner.
  // Emulate that so the main window is modal while the settings window exists.
  if (main_hwnd_ != nullptr) {
    ::EnableWindow(main_hwnd_, FALSE);
  }
}

void OptionWindowHost::OnSettingsWindowDestroyed() {
  settings_channel_.reset();
  settings_window_.reset();
  if (main_hwnd_ != nullptr && ::IsWindow(main_hwnd_) != 0) {
    ::EnableWindow(main_hwnd_, TRUE);
    ::SetForegroundWindow(main_hwnd_);
  }
}
