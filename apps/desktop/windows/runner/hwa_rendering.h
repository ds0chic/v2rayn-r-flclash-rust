#ifndef RUNNER_HWA_RENDERING_H_
#define RUNNER_HWA_RENDERING_H_

#include <string>
#include <vector>

namespace flutter {
class DartProject;
class FlutterEngine;
}  // namespace flutter

// SP-26: maps the persisted EnableHWA switch (upstream default false,
// process-wide, restart-to-apply) onto the Flutter Windows embedder's
// renderer selection. Honest downgrade: the embedder exposes no SoftwareOnly
// equivalent, so OFF means low-power GPU + non-Impeller path, NOT software
// rendering. See docs/decisions/SP-26-hwa-runner.md.
namespace hwa {

// Effective switch. Precedence: explicit CLI flags, then the
// V2RAYNR_ENABLE_HWA environment variable, then the
// <exe-dir>\v2raynr_hwa.ini flag file (contract for the settings pipeline,
// see ADR), else false (upstream default).
bool ResolveHardwareAcceleration(const std::vector<std::string>& args);

// Removes HWA-only CLI flags so they are not forwarded to Dart.
void StripHwaFlags(std::vector<std::string>& args);

// Applies the mapping to |project| before engine creation and logs it.
// Never aborts startup; unknown inputs fall back to OFF.
void ApplyHardwareAcceleration(flutter::DartProject& project, bool enable_hwa);

// Records the DXGI adapter actually used for rendering. Call after the
// engine/view exists. Never fails startup; logs failures instead.
void LogGraphicsAdapter(flutter::FlutterEngine* engine);

}  // namespace hwa

#endif  // RUNNER_HWA_RENDERING_H_
