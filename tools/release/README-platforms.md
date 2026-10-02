# Building on macOS / Linux (not verified)

This document describes the steps required to build `v2rayN-R` on macOS and
Linux. **Nothing here has been executed or verified.** No macOS/Linux build or
runtime support is claimed. Treat this as a starting checklist only.

The shared Rust backend and Flutter UI are cross-platform, but the packaged
release scripts in this directory currently target Windows only. The Windows
release script is `tools/release/build_windows.ps1`.

---

## Common prerequisites

| Tool | Notes |
|---|---|
| Flutter | 3.47.5 stable (Dart 3.13.4); must match `apps/desktop/pubspec.yaml` |
| Rust | 1.98.1 stable; the workspace pins `Cargo.lock` (`--locked`) |
| flutter_rust_bridge | 2.13.0 (`flutter_rust_bridge_codegen`, same version in Dart/Rust/codegen) |
| C/C++ toolchain | clang/LLVM + platform SDK |
| `net_host` + `privileged_helper` | built from this workspace; must sit next to the app binary |

Build order (repository root):

```bash
cargo build --workspace --release --locked
(cd apps/desktop && flutter build <platform> --release)
```

Package assembly must mirror `build_windows.ps1`: copy the Flutter Release
bundle, then add `net_host` and `privileged_helper` from `target/release/`, plus
`LICENSE`, `NOTICE.md`, `README.md`. Do **not** bundle proxy cores.

---

## macOS

- Requires Xcode command-line tools and CocoaPods.
- Flutter target: `flutter build macos --release`.
- The Rust plugin is wired through `apps/desktop/rust_builder/macos`
  (`bridge_api.podspec`) using cargokit.
- `net_host` / `privileged_helper` are currently Windows-centric; the TUN helper
  path on macOS uses a kill-sudo-style flow (see upstream baseline and
  `docs/evidence/T14-*.md`). This needs a dedicated port and verification.
- App signing / notarization / entitlements are **not** covered.

```bash
cargo build --workspace --release --locked
cd apps/desktop && flutter build macos --release
```

## Linux

- Requires clang, cmake, ninja, GTK 3 development headers, and `pkg-config`.
- Flutter target: `flutter build linux --release`.
- The Rust plugin is wired through `apps/desktop/rust_builder/linux/CMakeLists.txt`
  using cargokit.
- TUN/privilege handling, autostart (`.desktop`), and system-proxy behavior
  differ from Windows and are **unverified**.
- Packaging (`.deb` / `.rpm`) is not implemented here.

```bash
cargo build --workspace --release --locked
cd apps/desktop && flutter build linux --release
```

---

## Not done / not claimed

- No macOS or Linux build has been produced or run in this project.
- No installer, code signing, notarization, or package format.
- No platform-specific TUN/privilege/autostart verification.
- Windows ARM64 is also unsupported by the pinned Flutter CLI
  (`flutter build windows` has no `--target-platform`); see
  `docs/evidence/T20.md`.

Do not represent any of the above as supported until a real build and runtime
smoke exist, following the same evidence standard as
`tools/release/smoke_windows.ps1`.
