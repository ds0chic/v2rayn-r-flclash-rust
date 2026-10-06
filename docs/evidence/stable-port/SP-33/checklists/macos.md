# SP-33 实例 O03/O04：macOS x64 / ARM64 —— 验收清单（blocked）

阻塞：无 Apple 硬件/Xcode SDK；Windows 本机无法产出 macOS 产物（平台矩阵 §6.1）。

解除条件：
1. 隔离 Mac（二选一）：Intel Mac（O03）/ Apple Silicon（O04），或 macOS CI runner；
2. `xcode-select --install` + CocoaPods + pinned Flutter 3.47.5 macOS 侧可用；
3. Rust 对应 target（x64：`x86_64-apple-darwin`；ARM64：`aarch64-apple-darwin`）。

解除后执行项（与 O01 同标准，不借 O01 证据）：
- `cargo build --workspace --release --locked` + `flutter build macos --release`
 （见 `tools/release/README-platforms.md`；cargokit 经 `bridge_api.podspec`）；
- P0 包身份（macOS 包布局：`.app` bundle + `net_host`/helper 对应物 +
  LICENSE/NOTICE/README；**不捆绑内核**；`.dmg`/`.zip` 命名对照上游
  `v2rayN-macos-<arch>.zip/.dmg`，不宣称等价）；
- P1–P7 全步，另加 macOS 特有分支实测：Dock accessory 策略与 `MacOSShowInDock`
  切换、`~/Library/LaunchAgents` 自启 plist 写/删（隔离机）、非 Windows 代理分支、
  热键设置项不可见、二维码截屏返回 null（平台矩阵 §4）；
- 签名/公证/entitlements：未覆盖，登记未验证；在配齐 Apple 签名身份前
  “未签名包仅隔离验收，不发布”。

隔离机要求：同 Linux —— 真实 OS 副作用仅授权隔离环境。
