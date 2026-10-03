#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <cstdio>
#include <string>

#include "flutter_window.h"
#include "runner_messages.h"
#include "utils.h"

namespace {

const wchar_t kWindowTitle[] = L"v2rayN-R (T01)";
const wchar_t kStateFileName[] = L"v2raynr_window_state.ini";
constexpr UINT kDefaultWidth = 1200;
constexpr UINT kDefaultHeight = 800;

// Per-executable instance mutex name. Anchoring on the executable path keeps
// separate installs / isolated test data dirs independent, matching the
// upstream single-instance lock which is scoped to the running app.
std::wstring InstanceMutexName() {
  wchar_t path[MAX_PATH];
  const DWORD length = ::GetModuleFileNameW(nullptr, path, MAX_PATH);
  const std::wstring full(path, length);
  unsigned long long hash = 1469598103934665603ULL;
  for (const wchar_t c : full) {
    hash ^= static_cast<unsigned long long>(c);
    hash *= 1099511628211ULL;
  }
  wchar_t buffer[64];
  ::swprintf(buffer, 64, L"Global\\v2rayN-R-SingleInstance-%llX", hash);
  return std::wstring(buffer);
}

// Wake the first instance: locate its window by class and post the show
// message (ROOT-06 / ACT-WIN-012).
void ActivateExistingInstance() {
  HWND existing = ::FindWindowW(kFlutterWindowClassName, nullptr);
  if (existing != nullptr) {
    ::PostMessageW(existing, kShowWindowMessage, 0, 0);
  }
}

// Draft T01 window-size persistence next to the executable; the final
// AppSettings/WindowSizeItem contract is a later task (FLD-CFG-156..158).
std::wstring StateFilePath() {
  wchar_t path[MAX_PATH];
  DWORD length = ::GetModuleFileNameW(nullptr, path, MAX_PATH);
  std::wstring full(path, length);
  const size_t slash = full.find_last_of(L"\\/");
  if (slash != std::wstring::npos) {
    full = full.substr(0, slash + 1);
  }
  return full + kStateFileName;
}

Win32Window::Size LoadWindowSize() {
  const std::wstring file = StateFilePath();
  UINT width = ::GetPrivateProfileIntW(L"window", L"width", kDefaultWidth,
                                       file.c_str());
  UINT height = ::GetPrivateProfileIntW(L"window", L"height", kDefaultHeight,
                                        file.c_str());
  if (width < 400) width = kDefaultWidth;
  if (height < 300) height = kDefaultHeight;
  return Win32Window::Size(width, height);
}

void SaveWindowSize(HWND handle) {
  RECT rect;
  if (handle == nullptr || !::GetWindowRect(handle, &rect)) {
    return;
  }
  const int width = rect.right - rect.left;
  const int height = rect.bottom - rect.top;
  const std::wstring file = StateFilePath();
  ::WritePrivateProfileStringW(L"window", L"width",
                               std::to_wstring(width).c_str(), file.c_str());
  ::WritePrivateProfileStringW(L"window", L"height",
                               std::to_wstring(height).c_str(), file.c_str());
}

}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  // Upstream startup argument: `App.xaml.cs` treats `Global.RebootAs`
  // ("rebootas") as a self-restart. The replacement process (often elevated)
  // must start normally instead of being mistaken for a second instance and
  // exiting. It is the only argument the upstream app parses from `e.Args`.
  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();
  bool reboot_as = false;
  for (const std::string& arg : command_line_arguments) {
    if (arg == "rebootas") {
      reboot_as = true;
      break;
    }
  }

  // Single instance (ROOT-06): a second launch wakes the running window and
  // exits; the lock is held for the process lifetime. `rebootas` bypasses the
  // exit so the elevated replacement can take over.
  HANDLE instance_mutex =
      ::CreateMutexW(nullptr, FALSE, InstanceMutexName().c_str());
  if (!reboot_as && instance_mutex != nullptr &&
      ::GetLastError() == ERROR_ALREADY_EXISTS) {
    ActivateExistingInstance();
    ::CloseHandle(instance_mutex);
    ::CoUninitialize();
    return EXIT_SUCCESS;
  }

  flutter::DartProject project(L"data");

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size = LoadWindowSize();
  if (!window.Create(kWindowTitle, origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  SaveWindowSize(window.GetHandle());
  if (instance_mutex != nullptr) {
    ::ReleaseMutex(instance_mutex);
    ::CloseHandle(instance_mutex);
  }
  ::CoUninitialize();
  return EXIT_SUCCESS;
}
