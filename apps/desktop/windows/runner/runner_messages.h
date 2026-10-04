#ifndef RUNNER_RUNNER_MESSAGES_H_
#define RUNNER_RUNNER_MESSAGES_H_

#include <windows.h>

// Posted to the first instance when a second instance starts, so the running
// window is restored and brought to the foreground (ROOT-06 / ACT-WIN-012).
constexpr UINT kShowWindowMessage = WM_APP + 0x51;

// Class name the Flutter embedder registers for the top-level window. Used to
// locate the first instance without depending on its (mutable) title.
constexpr wchar_t kFlutterWindowClassName[] =
    L"FLUTTER_RUNNER_WIN32_WINDOW";

// Posted by the option-window host to the settings window to make it destroy
// itself once the "close" method call has been answered.
constexpr UINT kSettingsCloseMessage = WM_APP + 0x52;

// Posted by the settings window to the main window once its HWND is gone, so
// the host can release it and re-enable the main window.
constexpr UINT kSettingsClosedMessage = WM_APP + 0x53;

#endif  // RUNNER_RUNNER_MESSAGES_H_
