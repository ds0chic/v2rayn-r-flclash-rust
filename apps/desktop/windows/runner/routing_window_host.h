#ifndef RUNNER_ROUTING_WINDOW_HOST_H_
#define RUNNER_ROUTING_WINDOW_HOST_H_

#include <flutter/encodable_value.h>
#include <flutter/flutter_engine.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>
#include <windows.h>

#include <memory>
#include <string>

#include "routing_window.h"

// Owns the independent top-level "路由设置" (routing settings) window and the
// method channels that let the two Flutter engines exchange the routing
// snapshot and the edited draft. Mirrors OptionWindowHost (Wave L).
//
// Protocol (channel "v2rayn/routing_window", per engine messenger):
//   main engine  -> native : open(snapshotJson) -> bool
//   main engine  -> native : reportOutcome({id, ok, message})
//   native       -> main   : applyDraft({id, draft})
//   routing eng  -> native : ready() -> snapshotJson
//   routing eng  -> native : saveDraft({id, draft})
//   routing eng  -> native : close()
//   native       -> routing: saveOutcome({id, ok, message})
class RoutingWindowHost {
 public:
  static RoutingWindowHost& Instance();

  // Binds the main engine's channel and remembers the owner HWND. Called once
  // from FlutterWindow::OnCreate.
  void Attach(flutter::FlutterEngine* main_engine, HWND main_hwnd);

  // Called on the main window's thread when the routing window has finished
  // destroying itself.
  void OnRoutingWindowDestroyed();

  HWND main_hwnd() const { return main_hwnd_; }

 private:
  RoutingWindowHost() = default;
  RoutingWindowHost(const RoutingWindowHost&) = delete;
  RoutingWindowHost& operator=(const RoutingWindowHost&) = delete;

  void HandleMainCall(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);
  void HandleRoutingCall(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  void OpenRoutingWindow(const std::string& snapshot);

  flutter::FlutterEngine* main_engine_ = nullptr;
  HWND main_hwnd_ = nullptr;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> main_channel_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      routing_channel_;
  std::unique_ptr<RoutingWindow> routing_window_;
  std::string snapshot_ = "{}";
};

#endif  // RUNNER_ROUTING_WINDOW_HOST_H_
