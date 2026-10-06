# SP-33 实例 O05/O06：Linux x64 / ARM64 —— 验收清单（blocked）

阻塞（实测依据）：本机无 WSL 分发、无 docker（T21-A：`wsl --status` exit 50；
平台矩阵 §6.1）。O06 另缺 ARM64 工具链。

解除条件：
1. 隔离 Linux 机（二选一）：`wsl --install`（管理员+重启）后发行版可用，或
   Docker Desktop / 原生 Linux x64 主机；O06 另需 `gcc-aarch64-linux-gnu` 或
   ARM64 Linux 主机；
2. 发行版内：`build-essential`、`pkg-config`、GTK3 dev 头、cmake/ninja、
   对应 `rustup target`（x64：`x86_64-unknown-linux-gnu`；ARM64：`aarch64-unknown-linux-gnu`）；
3. pinned Flutter 3.47.5 Linux 侧可用（`flutter build linux --release`）。

解除后执行项（与 O01 同标准，不借 O01 证据）：
- `cargo build --workspace --release --locked` + `flutter build linux --release`
 （见 `tools/release/README-platforms.md`；cargokit 经 `rust_builder/linux/CMakeLists.txt`）；
- P0 包身份（Linux 包布局：Flutter bundle + `net_host`/`privileged_helper` +
  LICENSE/NOTICE/README；**不捆绑内核**；`.deb`/`.rpm` 打包未实现——缺失则登记未验证）；
- P1–P7 全步，另加 Linux 特有分支实测：`.desktop` 自启写/删（隔离机）、
  sudo 密码流、TUN 权限路径、非 Windows 系统代理分支与自定义脚本路径、
  `export` 前缀复制命令、截屏取码返回 null 行为（平台矩阵 §4）；
- 签名：`.deb`/`.rpm` 签名未实现，登记未验证，不宣称发行就绪。

隔离机要求：真实 OS 副作用（代理/路由/TUN/DNS/自启）仅授权隔离环境；
宿主日常机不执行。
