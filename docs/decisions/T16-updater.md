# T16 更新管线决策记录（crates/updater）

- 关联方案：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §15（更新链）、§18（T16）。
- 上游只读来源（commit `7d6a967`）：
  - `ServiceLib/Services/UpdateService.cs`
  - `ServiceLib/Manager/CoreInfoManager.cs`
  - `ServiceLib/Global.cs`（`CoreUrls`、`GithubApiUrl`）
  - `ServiceLib/Models/Dto/GitHubRelease.cs`、`.../SemanticVersion.cs`、`Models/CoreConfigs/CoreInfo.cs`
  - `AmazTool/UpgradeApp.cs`
  - `tools/cores/cores.lock.json`（sha256 与 `.dgst` 资产）
- 关联台账/夹具：`tools/cores/cores.lock.json`（`read_lock_expected_sha256` 的输入格式）。

## 1) 决策 D1 —— 版本解析与比较忠实复刻上游

上游 `SemanticVersion` 的语义有两个非标准点，本实现刻意保留：

- **解析失败回退 `0.0.0` 而不报错**：`SemanticVersion(string)` 在任何异常时回到 `0.0.0`。`Semver::parse` 同此；另提供 `try_parse_strict` 供需要区分“真 0.0.0”与“非法”的调用方。
- **预发布分段比较**：上游 `CompareSegment`：`(false,true)=>1`、`(true,false)=>-1`、`(false,false)=>字符串序`。即数字标识符低于字母数字标识符（符合 SemVer 2.0.0 §11.4.3），字母数字按序数比较。

差异标注：本实现 `Eq`/`Hash` 忽略 `raw` 与 build 元数据（上游 `Equals` 也忽略 build），build 不参与排序（SemVer §10）。

## 2) 决策 D2 —— 预发布选择对齐 GitHub 排序

上游 `GetRemoteVersion`：
- `preRelease == true` → `gitHubReleases?.First()`（**不看该条自身的 prerelease 标志**，依赖 GitHub 返回“最新在前”）。
- 否则 → `First(r => r.Prerelease == false)`。

`ReleasesClient::pick` 精确复刻：`prerelease=true` 返回 `releases.first()`；`false` 返回第一个非预发布。测试夹具必须显式按“最新在前”排列才能反映真实行为（已在用例中注明并同时覆盖两种排列）。

`pick_with_limit` 复刻 `LockedMaxVersion` 分支：当频道头版本超过锁定上限时，回退到 `≤ 上限` 的最高版本（`MaxBy`），无可用版本则 `NoRelease`。

## 3) 决策 D3 —— 内置更新目标与渠道策略

`IsCheckUpdateSupported` 仅 xray/mihomo/sing_box/v2rayN；`v2rayN` 在打包安装（`IsPackagedInstall`）时为 false。`GetCheckPreRelease` 仅 v2rayN 与 Xray 跟随用户的预发布偏好，其余恒为 stable。sing-box 锁定上限 `1.14.x`（`SemanticVersion(1,14,int.MaxValue)`）。

这些以纯函数形式实现（`is_check_update_supported`、`check_pre_release`、`spec_version_in_range`），便于测试，不读取全局状态。

## 4) 决策 D4 —— 校验链分级，签名“未决”显式化

方案 §15 要求“仅比较同源哈希不等于真实性验证”。因此：
- sha256 通过 `digest`/lock 期望值完成**完整性**校验；
- 真实性交给 `SignatureVerifier` 钩子。由于方案未固定方案与公钥，默认 `UnsupportedSignatureVerifier` **显式失败**（`SignatureUnsupported`，`is_available()==false`），绝不返回成功。这样上层可以区分“已验证”与“签名待定”，符合“不冒充验证成功”。

`PrefixHashVerifier` 仅为测试控制流存在，命名与文档明确标注为非加密。

## 5) 决策 D5 —— 不引入 `tar` 依赖，自带最小 USTAR 读取器

工作区锁文件未包含 `tar` crate，离线环境无法新增。为不引入网络依赖且保持安全边界，`safe_unpack_targz` 内置最小 USTAR/GNU 表头读取器：
- 只接受普通文件（typeflag `0`/NUL）与目录（`5`）；
- 符号链接（`2`）、硬链接（`1`）、设备/FIFO 等一律拒绝；
- Pax/GNU 扩展头按 size 跳过其 payload；
- 写入时用 `take(max+1)` 独立钳制，防声明长度撒谎。

zip 侧直接使用工作区既有 `zip` 4.6.1（`deflate`），用 `is_symlink()`/`unix_mode()` 检测符号链接。

## 6) 决策 D6 —— 原子替换只做同卷 rename，不碰活文件

`apply_atomic` 不复制、不删除旧版直到新版就位；顺序为 `current→keep`、`staged→current`，第二步失败即回滚第一步。全部在调用方给定的 `root` 内，越界路径在 `validate()` 拒绝。跨卷场景未实现（记为未决）。

`UpgradeCoordinator` 只产出 `ExternalUpgradeSpec`，不 spawn。对应上游 `AmazTool.UpgradeApp`：Windows 运行中的 exe 不能自替换，需外部进程等待宿主退出后替换，本任务只冻结接口。

崩溃注入通过 `FailPoint` + `apply_atomic_inject`（`#[doc(hidden)]`）实现，生产路径 `apply_atomic` 恒传 `None`，不污染稳定 API。

## 7) 决策 D7 —— 下载客户端永不读环境代理

对齐 T15 决策 D5：`reqwest::Client::builder().no_proxy()`（除非显式传入 `proxy`）。loopback mock 与真实请求都不得被用户系统代理（10808）截获。测试端口一律 ≥ 11808。

## 8) 决策 D8 —— 取消覆盖整个下载生命周期

初版将取消只放在 chunk 循环内，但初始 `send().await` 不可取消，导致取消请求在等待响应头期间无效。改为把整个 `download_inner` future `pin!` 后与 `wait_for_cancel` 一起 `select!`，取消时连带中止进行中的请求并清理 `.partial`。这是测试中发现并修正的真实缺陷（见 evidence）。

## 9) 决策 D9 —— mock 服务器 `Drop` 不 join

`tiny_http` 的 `respond` 向已被客户端关闭的连接写大 body 时，Windows 环回可能长时间阻塞；`Drop` 中 `join()` 会让测试挂死。改为只置 `running=false` 并 detach 线程：线程在两次请求之间观测标志退出；`pick_port` 只返回当前可 bind 的端口，故残留线程不会导致端口复用。此为测试基础设施权衡，不影响生产代码。
