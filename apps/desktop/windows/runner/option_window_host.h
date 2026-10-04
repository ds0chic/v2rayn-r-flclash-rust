#ifndef RUNNER_OPTION_WINDOW_HOST_H_
#define RUNNER_OPTION_WINDOW_HOST_H_

#include <flutter/encodable_value.h>
#include <flutter/flutter_engine.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>
#include <windows.h>

#include <memory>
#include <string>

#include "settings_window.h"

// Owns the independent top-level "设置" (option settings) window and the
// method channels that let the two Flutter engines exchange the settings
// snapshot and the edited draft. The main engine never has to know how the
// second window is implemented.
//
// Protocol (channel "v2rayn/option_window", per engine messenger):
//   main engine  -> native : open(snapshotJson) -> bool
//   main engine  -> native : reportOutcome({id, ok, message})
//   native       -> main   : applyDraft({id, draft})
//   settings eng -> native : ready() -> snapshotJson
//   settings eng -> native : saveDraft({id, draft})
//   settings eng -> native : close()
//   native       -> settings: saveOutcome({id, ok, message})
class OptionWindowHost {
 public:
  static OptionWindowHost& Instance();

  // Binds the main engine's channel and remembers the owner HWND. Called once
  // from FlutterWindow::OnCreate.
  void Attach(flutter::FlutterEngine* main_engine, HWND main_hwnd);

  // Called on the main window's thread when the settings window has finished
  // destroying itself.
  void OnSettingsWindowDestroyed();

  HWND main_hwnd() const { return main_hwnd_; }

 private:
  OptionWindowHost() = default;
  OptionWindowHost(const OptionWindowHost&) = delete;
  OptionWindowHost& operator=(const OptionWindowHost&) = delete;

  void HandleMainCall(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
  void HandleSettingsCall(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  void OpenSettingsWindow(const std::string& snapshot);

  flutter::FlutterEngine* main_engine_ = nullptr;
  HWND main_hwnd_ = nullptr;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> main_channel_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      settings_channel_;
  std::unique_ptr<SettingsWindow> settings_window_;
  std::string snapshot_ = "{}";
};

#endif  // RUNNER_OPTION_WINDOW_HOST_H_
