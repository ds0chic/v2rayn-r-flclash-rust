#include "hwa_rendering.h"

#include <dxgi.h>
#include <windows.h>

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <flutter/dart_project.h>
#include <flutter/flutter_engine.h>
#include <fstream>

#include "utils.h"

namespace hwa {
namespace {

// Recognized CLI flags. `--hwa` alone means on; `--no-hwa`,
// `--disable-hwa` mean off; `--enable-hwa` means on.
constexpr char kEnvName[] = "V2RAYNR_ENABLE_HWA";
constexpr wchar_t kFlagFileName[] = L"v2raynr_hwa.ini";
constexpr wchar_t kLogFileName[] = L"v2raynr-hwa.log";

bool ParseBool(const std::string& raw, bool& out) {
  std::string v = raw;
  std::transform(v.begin(), v.end(), v.begin(),
                 [](unsigned char c) { return (char)::tolower(c); });
  if (v == "1" || v == "true" || v == "on" || v == "yes" || v == "enable" ||
      v == "enabled") {
    out = true;
    return true;
  }
  if (v == "0" || v == "false" || v == "off" || v == "no" || v == "disable" ||
      v == "disabled") {
    out = false;
    return true;
  }
  return false;
}

// Returns true when |arg| is an HWA-only flag; sets |out| when it carries a
// decision (bare `--hwa` = on).
bool MatchCliFlag(const std::string& arg, bool& out) {
  if (arg == "--hwa" || arg == "--enable-hwa") {
    out = true;
    return true;
  }
  if (arg == "--no-hwa" || arg == "--disable-hwa") {
    out = false;
    return true;
  }
  constexpr char kPrefix[] = "--hwa=";
  if (arg.compare(0, sizeof(kPrefix) - 1, kPrefix) == 0) {
    return ParseBool(arg.substr(sizeof(kPrefix) - 1), out);
  }
  return false;
}

std::wstring ExeDir() {
  wchar_t path[MAX_PATH];
  const DWORD length = ::GetModuleFileNameW(nullptr, path, MAX_PATH);
  std::wstring full(path, length);
  const size_t slash = full.find_last_of(L"\\/");
  if (slash != std::wstring::npos) {
    full.resize(slash + 1);
  }
  return full;
}

bool ReadEnv(bool& out) {
  wchar_t buf[16];
  const DWORD n =
      ::GetEnvironmentVariableW(L"V2RAYNR_ENABLE_HWA", buf, 16);
  if (n == 0 || n >= 16) {
    return false;
  }
  return ParseBool(Utf8FromUtf16(buf), out);
}

// Flag file contract for the settings pipeline (ADR): next to the
// executable, `[rendering] enable_hwa=1|0`. Absent/unparsable = no opinion.
bool ReadFlagFile(bool& out) {
  const std::wstring file = ExeDir() + kFlagFileName;
  wchar_t buf[16];
  ::GetPrivateProfileStringW(L"rendering", L"enable_hwa", L"", buf, 16,
                             file.c_str());
  if (buf[0] == L'\0') {
    return false;
  }
  return ParseBool(Utf8FromUtf16(buf), out);
}

void EmitLine(const std::string& line) {
  char stamp[32];
  SYSTEMTIME st;
  ::GetLocalTime(&st);
  ::snprintf(stamp, sizeof(stamp), "%04u-%02u-%02u %02u:%02u:%02u ", st.wYear,
             st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond);
  std::string full = std::string(stamp) + "[v2raynr-hwa pid=" +
                     std::to_string(::GetCurrentProcessId()) + "] " + line +
                     "\n";

  std::wstring wfull(full.begin(), full.end());
  ::OutputDebugStringW(wfull.c_str());
  ::fputs(full.c_str(), stderr);

  wchar_t tmp[MAX_PATH];
  const DWORD n =
      ::GetEnvironmentVariableW(L"TEMP", tmp, MAX_PATH);
  std::wstring log = (n > 0 && n < MAX_PATH)
                         ? std::wstring(tmp) + L"\\" + kLogFileName
                         : ExeDir() + kLogFileName;
  std::ofstream f(log, std::ios::app | std::ios::binary);
  if (f) {
    f.write(full.data(), (std::streamsize)full.size());
  }
}

}  // namespace

bool ResolveHardwareAcceleration(const std::vector<std::string>& args) {
  bool value = false;
  bool saw_cli = false;
  for (const std::string& arg : args) {
    bool v = false;
    if (MatchCliFlag(arg, v)) {
      saw_cli = true;
      value = v;
    }
  }
  // CLI wins only when present; otherwise env, then flag file, else default.
  if (saw_cli) {
    return value;
  }
  if (ReadEnv(value)) {
    return value;
  }
  if (ReadFlagFile(value)) {
    return value;
  }
  return false;
}

void StripHwaFlags(std::vector<std::string>& args) {
  args.erase(std::remove_if(args.begin(), args.end(),
                            [](const std::string& arg) {
                              bool v = false;
                              return MatchCliFlag(arg, v);
                            }),
             args.end());
}

void ApplyHardwareAcceleration(flutter::DartProject& project, bool enable_hwa) {
  if (enable_hwa) {
    project.set_impeller_switch(flutter::ImpellerSwitch::Default);
    project.set_gpu_preference(flutter::GpuPreference::HighPerformancePreference);
    EmitLine(
        "mode=ON impeller=Default gpu=HighPerformance (NOT SoftwareOnly)");
  } else {
    project.set_impeller_switch(flutter::ImpellerSwitch::Disabled);
    project.set_gpu_preference(flutter::GpuPreference::LowPowerPreference);
    EmitLine(
        "mode=OFF impeller=Disabled gpu=LowPower (NOT SoftwareOnly: "
        "ANGLE/Skia GPU fallback, low-power preference only)");
  }
}

void LogGraphicsAdapter(flutter::FlutterEngine* engine) {
  if (engine == nullptr) {
    EmitLine("adapter=unavailable(no-engine)");
    return;
  }
  IDXGIAdapter* adapter = nullptr;
  if (!engine->GetGraphicsAdapter(&adapter) || adapter == nullptr) {
    EmitLine("adapter=unavailable(GetGraphicsAdapter-failed)");
    return;
  }
  DXGI_ADAPTER_DESC desc{};
  char line[512];
  if (SUCCEEDED(adapter->GetDesc(&desc))) {
    ::snprintf(line, sizeof(line),
               "adapter desc=\"%s\" vendor=0x%04X device=0x%04X vram_mb=%llu",
               Utf8FromUtf16(desc.Description).c_str(), desc.VendorId,
               desc.DeviceId,
               (unsigned long long)desc.DedicatedVideoMemory / (1024 * 1024));
  } else {
    ::snprintf(line, sizeof(line), "adapter=unavailable(GetDesc-failed)");
  }
  adapter->Release();
  EmitLine(line);
}

}  // namespace hwa
