# 宿主代理误清事件（2026-10-05）

## 事实与时间线
- 21:01 前：宿主 WinINET 代理为开启状态：`ProxyEnable=1`、`ProxyServer=127.0.0.1:20808`
  （`AutoConfigURL` 空；`ProxyOverride` 当时未读取，原值未知）。
- 21:03:12：根代理启动发布包实例 PID 5152（数据目录为真实 `%LOCALAPPDATA%\v2rayN-R\data`，
  当时该目录配置 `SystemProxyItem.SysProxyType=0`，即“清除系统代理”）。
- 21:03:13：`HKCU\...\Internet Settings` 注册表键 LastWriteTime 与该实例启动同秒；
  该键被写入：`ProxyEnable=0`，`ProxyServer`/`ProxyOverride`/`AutoConfigURL` 值被删除。
  即：启动恢复路径 `restoreAppliedModeOnLaunch` -> `forcedClear` 应用，清空了宿主代理。

## 根因
- 应用启动恢复（上游 `LoadCore` -> `UpdateSysProxy` 语义）会按持久化模式应用系统代理；
  旧配置为 `SysProxyType=0`（forcedClear）时，启动即清空宿主代理。
- 根代理在改配置（切 `SysProxyType=2`）之前先启动了 PID 5152，属操作顺序失误；
  违反 AGENTS 硬约束“禁止修改宿主系统代理”。

## 处置
- 21:12：停止 5152；将配置改为 `SysProxyType=2`（“不改变系统代理”）并备份原文件为
  `guiNConfig.json.bak-prefullstart`；同时将 `Inbound[0].localPort` 由 10808 改为 11808
  （遵守禁占 10808 约束）。内核从 `tools/cores` 锁版本复制到 `<data>/cores`
  （xray v26.3.27、singbox v1.14.2、mihomo v1.19.32）。
- 21:12 起运行实例 PID 8068 为 mode 2，不再写宿主代理；Xray PID 21144 监听 127.0.0.1:11808。
- 经用户批准恢复宿主代理：写回 `ProxyEnable=1`、`ProxyServer=127.0.0.1:20808`，并调用
  WinINET `INTERNET_OPTION_SETTINGS_CHANGED` + `INTERNET_OPTION_REFRESH`。
  `ProxyOverride` 原值未知，未猜测恢复，需由用户代理工具重新应用或提供原值。

## 后续约束
- 任何对真实数据目录的发布包启动前，必须先确认 `SysProxyType=2`；不得在 mode 0/1/3 下启动。
- 测试/验收一律使用隔离 `V2RAYN_R_DATA_DIR`，避免触碰用户配置与宿主代理。
