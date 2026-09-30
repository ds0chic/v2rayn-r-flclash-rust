#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <string>

#include "flutter_window.h"
#include "utils.h"

namespace {

const wchar_t kWindowTitle[] = L"v2rayN-R (T01)";
const wchar_t kStateFileName[] = L"v2raynr_window_state.ini";
constexpr UINT kDefaultWidth = 1200;
constexpr UINT kDefaultHeight = 800;

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

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

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
  ::CoUninitialize();
  return EXIT_SUCCESS;
}
