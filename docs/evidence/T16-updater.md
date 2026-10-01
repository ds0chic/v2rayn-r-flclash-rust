# T16 更新管线证据（crates/updater）

- 任务：T16-updater（应用与内核更新管线基础设施）
- 范围：仅 `crates/updater/**`、`docs/evidence/T16-updater.md`、`docs/decisions/T16-updater.md`。
  未改 `apps/`、`compat/`、`work/`、`outputs/`，未改其他 crate 源码；未 commit。
- 锁文件例外：`Cargo.lock` 由 cargo 自动更新（updater 新增 domain/sha2/hex/zip/flate2/reqwest/tokio/futures-util/thiserror/tiny_http/tempfile 依赖项）。
  这是 `--locked` 门禁能通过的前提。工作区中同时存在其他任务对 `Cargo.lock` 的改动（application/privileged_helper），非本回合产生。
- 环境：Windows 11 x64，Rust 1.98.1（MSVC）。本机用户代理端口 10808 未被触碰；全部 mock 与测试端口 ≥ 11808。
- **未执行真实替换**：本回合不更新、不替换任何本机文件。`apply_atomic` 仅在临时目录内以 rename 交换；外部 updater 只返回 `ExternalUpgradeSpec`，不启动进程。

## 1) 管线概览

对齐方案 §15 的更新链：

```
UpdateChannel/版本约束 → ReleasesClient.fetch → ReleaseInfo.pick / select_for_target
  → FileDownloader.download（暂存 + 流式 sha256）
  → verify：①sha256（.dgst/GitHub digest/本地 lock） ②SignatureVerifier 钩子 ③parse_binary_arch（PE/ELF/Mach-O）
  → safe_unpack_zip / safe_unpack_targz
  → apply_atomic（同卷 rename 交换 + 回滚）/ UpgradeCoordinator.external_upgrade_spec
  → InstallManifest（版本 + 逐文件 sha256）
```

## 2) API 概览

### 2.1 元数据 `metadata`
- `ReleaseAsset { name, size, browser_download_url, digest? }`，`ReleaseAsset::sha256()` 解析 `sha256:<hex>`。
- `ReleaseInfo { tag_name, name?, prerelease, published_at?, assets[] }`；`version()`、`select_asset(needles)`、`select_for_target(core, target)`。
- `parse_releases(json)`；`ReleasesClient { repo, api_base }` + `releases_url()` + `pick(releases, prerelease)` + `pick_with_limit(releases, prerelease, locked_max)`。
- `pick` 复刻上游：`prerelease=true` 取 GitHub 返回的第一个（最新）；否则取第一个非预发布。
- `asset_needles(core, target)`：按上游 `CoreInfoManager` 模板给出资产名片段（xray/sing-box/mihomo/v2rayN × windows/linux/macos × x64/x86/arm64）；未知 core 返回空 → `select_for_target` 报 `NoMatchingAsset`。

### 2.2 版本 `semver`
- `Semver::parse` / `try_parse_strict`：可选 `v`/`V` 前缀、2/3/4 段数字、`-prerelease`、`+build`；非法输入回退 `0.0.0`（对齐上游 catch）。
- `Ord` 按 主.次.补 → 预发布列表比较；数字标识符比字母数字标识符低（SemVer 2.0.0 §11.4.3）；build 元数据不参与比较。
- `Eq`/`Hash` 忽略 `raw` 与 build；`to_standard_string(prefix)`。

### 2.3 平台架构 `arch`
- `Os`、`PlatformArch`、`HostTarget`、`detect_target()`、`parse_triple()`。
- `parse_binary_arch(bytes)` 只读解析 PE（`MZ`+`PE\0\0`+machine）、ELF64-LE（e_machine）、Mach-O（magic+cputype）；`BinaryArch::matches(target)`/`implied_target()`；`binary_matches`。
- PE machine：x64=0x8664、x86=0x014c、arm64=0xaa64；ELF：62/3/183；Mach-O：0x01000007/7/0x0100000c。

### 2.4 下载 `download`
- `DownloaderOptions`（headers/user_agent/proxy/connect_timeout/timeout/max_bytes/expected_size），默认 `no_proxy()`。
- `FileDownloader::download(request, cancellation)`：整体套在取消竞速内（**含初始 `send()`**），流式写 `.partial` → 校验大小/期望长度 → rename 到目标；失败清理部分文件。
- 错误：`Timeout`、`Incomplete`（0 字节 / 提前断开 / 长度不符）、`TooLarge`、`Cancelled`、`Download`。
- `sha256_of`、`sha256_matches`、`sha256_file`。

### 2.5 签名 `signature`
- `trait SignatureVerifier { scheme(); verify(artifact, signature); is_available() }`。
- `UnsupportedSignatureVerifier`：默认实现，`is_available()==false`，`verify` 返回 `SignatureUnsupported`（不冒充已验证）。
- `PrefixHashVerifier`：仅测试用控制流验证器（非加密方案）。

### 2.6 安全解包 `unpack`
- `UnpackLimits { max_total_bytes, max_entry_bytes, max_entries }`。
- `safe_join(root, member)`：拒绝绝对路径、`..`、Windows 盘符与保留名（CON/PRN/…/LPT9），归一化 `\`→`/`。
- `safe_unpack_zip`：拒绝目录穿越、符号链接（`is_symlink`/unix mode 0o120000）、单条目/总大小/条目数炸弹；写入时独立 `take(max+1)` 兜底。
- `safe_unpack_targz`：内置最小 USTAR/GNU 表头读取器（不引入 tar 依赖）；只接受普通文件与目录，符号/硬链接/设备/其他特殊类型一律拒绝，含钳制与 padding 处理。

### 2.7 替换与回滚 `install`
- `InstallManifest { version, previous_version?, files[{relative_path, sha256, size}] }`；`scan_directory`、`verify_manifest`。
- `InstallPlan { root, current_dir, staged_dir, keep_name, version, discard_previous }`；`validate()` 拒绝越界路径与含分隔符的 keep 名。
- `apply_atomic(plan)`：`current → keep` 后 `staged → current`；第二步失败回滚第一步；旧版本在新版本就位前不删除。
- `FailPoint { None, StageRename, CommitRename }` + `apply_atomic_inject`（`#[doc(hidden)]`）用于崩溃注入。
- `UpgradeCoordinator` + `external_upgrade_spec()`：返回 `ExternalUpgradeSpec { helper_exe, source, install_root, wait_for_pid, args }`，**不启动**外部进程（对齐上游 `AmazTool/UpgradeApp.cs` 的“等待退出后替换”设计）。

### 2.8 渠道 `channel`
- `UpdateChannel { Stable, Prerelease }`。
- `CORE_URLS`（= 上游 `Global.CoreUrls` 等价表）；`core_url_slug`、`download_template`、`core_spec`。
- `is_check_update_supported(core, packaged_install)`：内置仅 xray/mihomo/sing_box/v2rayN；v2rayN 打包安装时为 false（对齐 `IsCheckUpdateSupported`）。
- `check_pre_release(core, requested)`：仅 v2rayN 与 Xray 跟随预发布（对齐 `GetCheckPreRelease`）。
- `LOCKED_MAX_SING_BOX = (1,14,u32::MAX)`；`max_allowed_version`、`spec_version_in_range`。
- `read_lock_expected_sha256(lock_json, asset)`：读取 `tools/cores/cores.lock.json` 的期望哈希（小写）。
- `fetch::CoreReleaseApi`：reqwest + `no_proxy()`，`GET {api_base}/{repo}/releases`。

## 3) 测试统计（真实运行）

命令：`cargo test -p updater --locked`（另加 fmt/clippy 门禁）。

| 测试文件 | 用例数 | 覆盖点 |
|---|---|---|
| `src/**`（单元，46） | 46 | semver/arch/metadata/channel/install/download/signature/unpack 内联用例 |
| `tests/semver.rs` | 10 | v 前缀、2/4 段、非法回退、预发布比较边界、build 忽略、sing-box 上限边界 |
| `tests/arch.rs` | 9 | PE x64/x86/arm64、未知 machine、ELF64-LE、大端不映射、Mach-O、非法/截断 |
| `tests/channel.rs` | 11 | 内置目标、打包 v2rayN、预发布策略、1.14 上限、slug、模板、lock 读取 |
| `tests/download.rs` | 10 | 流式 sha256、Content-Length/流式超限、0 字节、长度不符、连接/整体超时、404、取消、非 http |
| `tests/install.rs` | 14 | 交换保留旧版、manifest 校验、丢弃旧版、缺 staged、keep 冲突、崩溃注入（stage/commit）、越界、篡改检测、外部 spec 不启动 |
| `tests/metadata.rs` | 14 | 模型解析、缺省字段、非法 JSON、stable/prerelease 选择、各平台资产、锁定上限回退、URL |
| `tests/pipeline.rs` | 3 | loopback mock GitHub API 全链（fetch→选择→下载→sha256→arch→解包→装→manifest）、上限阻断、哈希不符 |
| `tests/signature.rs` | 5 | Unsupported 不冒充成功、正确/篡改签名与工件 |
| `tests/unpack.rs` | 13 | 有效解包、zip/tar.gz 穿越/绝对/符号链接/单条与总炸弹/条目数 |
| **合计** | **135 通过 / 0 失败** | |

- mock：`tests/common/mod.rs` 的 `tiny_http` HTTP 服务器，绑定 `127.0.0.1`，端口 `11808..13000`；`pick_port` 先探测可用；`Drop` 不 join（避免对已关闭连接写大 body 时阻塞）。
- 全部使用 `tempfile` 临时目录；测试后无残留。

## 4) 门禁结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p updater -- --check` | 通过（exit 0） |
| `cargo clippy -p updater --all-targets --locked -- -D warnings` | 通过（无警告） |
| `cargo test -p updater --locked` | 135 通过 / 0 失败 |

## 5) 未执行清单

- **未做任何真实更新/替换**：没有下载真实 release、没有替换本机任何 exe/内核/数据库。
- **未启动外部 updater 进程**：只构造并断言 `ExternalUpgradeSpec`。
- **未做真实签名校验**：默认 verifier 返回 Unsupported，未接入 GPG/minisign 与可信公钥。
- **未对真实内核二进制做架构校验**：仅用合成 PE/ELF/Mach-O 头。
- **未联网访问 GitHub**：元数据全部来自 loopback mock。

## 6) 未决项

1. **签名方案未定（GPG/minisign）**：`SignatureVerifier` 钩子已就位，但默认 `UnsupportedSignatureVerifier` 不验证。需选定方案、内置可信公钥、接入 `.sig` 资产下载与校验。→ identified/未验证。
2. **外部 updater 实跑未验证**：`ExternalUpgradeSpec` 只给出可测试描述；Windows“运行中 exe 自替换 → 外部进程等待退出后执行”尚未实跑（对齐上游 `AmazTool`）。→ identified/未验证。
3. **`.dgst` 资产解析未接入**：`sha256` 目前取自 GitHub `digest` 或本地 `cores.lock.json`；磁盘上 `.dgst`（`Xray-*.zip.dgst`，含 MD5/SHA1/SHA2-256/SHA2-512）的下载与解析尚未实现。
4. **解压炸弹的压缩比维度未跟踪**：当前按解压后字节数与条目数钳制，未按压缩比（compressed/uncompressed）判定。
5. **mihomo `.gz`（非 tar.gz）单文件归档**未提供专用解包路径；`download_template` 已给出，但 `safe_unpack_*` 只处理 zip 与 tar.gz。
6. **跨卷/容器/权限场景**：`apply_atomic` 依赖同卷 rename；跨卷需先复制再交换，未实现。
7. **HTTP 代理经代理更新**：`DownloaderOptions.proxy` 支持显式代理，但“经当前运行会话代理更新”的接线由上层（net_host）负责，本 crate 未接。
