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

#endif  // RUNNER_RUNNER_MESSAGES_H_
