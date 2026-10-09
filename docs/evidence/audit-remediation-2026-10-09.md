# 2026-10-09 审计整改记录

本记录对应本地审计中的 H1–H22 项。修改按实际故障路径收敛，没有新增通用防御层。测试仅覆盖改动边界；没有执行真实系统代理写入、真实 TUN 路由或内核真机流量。

## 已修改

- 配置生成：修正 Xray 代理链 detour、模板自定义出站顺序、FakeDNS 嗅探字段和规则类型；修正 sing-box DNS 规则字段。TUN 计划带入 IPv6 路由状态和内核直连保护路径；测速会话使用完整应用配置，并关闭临时会话的 TUN。
- 持久化和导入：节点保存、导入、删除在配置文件持久化失败时回滚数据行与 revision；导入失败时保留仍被数据库引用的资源文件。
- 测速和备份恢复：取消测速不再被记为失败延迟；备份恢复后取消旧测速并重载测速覆盖层。
- 特权 helper：校验配置 token；删除路由和停止进程要求对应会话拥有该资源。helper token 改由临时文件传递，不出现在命令行参数中。
- 系统代理事务：后续字段写入或通知失败时回滚本次已写字段，并返回原始错误及回滚错误。验证仅使用假后端。
- 退出和设置应用：停止内核、PAC、订阅调度器或全局热键清理失败时，普通退出会留在前台并显示失败。应用更新交接清理失败时也不再退出；保存模板后若内核正在运行则重新应用。
- 更新和核心启动：Xray 启动环境传递资产目录；Mihomo `.gz` 可按限制解压，Unix 解压出的内核补执行位；替换核心时保留旧版本，运行中的当前核心更新后重建并应用运行计划。
- 备份体积：本地备份排除核心、日志和临时目录；WebDAV 上游 ZIP 直接流式写入文件，上传和下载逐块传输，不把整份归档同时放在内存中。
- 意外核心退出：先清理 sidecar，再释放所属 TUN 租约；清理失败时保留租约日志供下次启动恢复，并记录错误。

## 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test -p config_codegen --locked`：通过。
- `cargo test -p application --lib codegen --locked`：30 项通过。
- `cargo test -p application --test t16_backup local_backup_roundtrip_and_restore_is_clean --locked`：通过。
- `cargo test -p application --test t16_webdav check_list_upload_download_roundtrip --locked`：流式 WebDAV 上传/下载通过。
- `cargo test -p application --test wave_b_webdav waveb_e2e_save_upload_list_download_restore_reopen --locked`：流式 ZIP、上传、恢复和重开通过。
- `cargo test -p net_host r304_defers_helper_until_the_core_created_interface_is_discovered --locked -- --nocapture`：项目管理的测试内核退出后，假 helper 确认 TUN 租约清理通过。
- `flutter analyze`：无问题。
- `flutter test test/r4_05_contract_test.dart test/t16_update_test.dart`：14 项通过。
- 其余定向回归已分别覆盖节点持久化回滚、helper token/资源归属、代理部分写入回滚、gzip 解压及运行时参数；没有重跑全量 Rust/Flutter 测试。

## 未闭环与限制

- H19：系统代理所有权台账仍只在进程内。把恢复前的系统代理值持久化可能把带凭据的代理 URI 写入明文文件；在确定受保护存储方案前不落盘，因此进程异常终止后的自动恢复仍未解决。
- 真实 Windows 系统代理、开机自启和 TUN 路由未在宿主上执行；所有新增测试使用假后端或项目自有测试内核。
- WebDAV 文件流和 ZIP 兼容性由本地回环测试验证，未对真实 WebDAV 服务做远端/TLS 实测。
- 全量 `cargo test --workspace`、全量 `flutter test` 和 Windows release 构建本轮未运行；已有环境基线失败仍需沿用此前证据逐项判定。
