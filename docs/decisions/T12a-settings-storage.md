# ADR T12a — 设置存储、默认值三层语义与桥接形状

## 背景

上游 `guiNConfig.json` 是 `Config` 根 + 25 个 `ConfigItems` 子类的 PascalCase JSON，枚举以整数序列化，
`SaveConfig` 以 `DefaultIgnoreCondition.Never` 写出 null；`LoadConfig` 在对象缺失/为空时补三层默认
（属性初始化器 → LoadConfig 对象级合成/校正 → 运行时回退）。T12a 需要全量存储、桥接与设置界面，同时
把 `apply_timing` 反馈给 UI。

## 决策

1. **类型即持久化形状**：`AppSettings` 及各组直接以 PascalCase + 整数枚举序列化；缩写字段（`IPv4Address`、
   `FakeIP`、`BlockAAAAQuery`、`SimpleDNSItem`、`MsgUIItem`、`ClashUIItem`、`XudpProxyUDP443`、`MacOSShowInDock`
   等）显式 `#[serde(rename)]`。未知键经每级 `#[serde(flatten)] extra` 保留。
2. **null/空串可区分**：上游可为 null 的字符串一律 `Option<String>`；数值/布尔仅在上游为 `?` 时用 `Option<T>`。
3. **默认值用逐字段 serde default，而不是容器 Default**：serde 的容器 `#[serde(default)]` 会以整struct 的
   `Default` 作为缺失字段基底，会掩盖“对象存在但字段缺失”的 CLR 语义。因此每个字段单独标注
   `#[serde(default)]` / `#[serde(default = "...")]`；组的 `Default` 仅用于“整个组缺失/null”时由根字段默认提供。
4. **LoadConfig 校正集中在 `apply_load_defaults`**：Inbound 默认/强制 socks、KCP 的 `<=0` 校正、RootCertProvider
   校验、语言/测速/系统代理例外、Fragment 的 legacy 回退等；运行时回退（如空 `Stack`→`gvisor`）不落盘，留在生成器。
5. **revision 语义**：整树 `settings_revision` + 每组 `settings_group_revisions`。`save_settings` 校验整树
   revision，`save_settings_group` 校验该组 revision；任一失败在写入前返回 `E_REVISION_STALE`，旧值不变；
   写盘为 temp+rename 原子写，并与 profile meta 键合并（不覆盖对方）。
6. **apply_timing 表**：由 `compat/fields.settings.yaml` 生成 `FIELD_TIMING`（180 条），保存时对比新旧树得到
   变更叶子路径并分类；整组增删取该组最强 timing（RestartApp > RestartCore > NextLaunch > Immediate > Save）。
7. **FRB 形状**：为便于 UI 与未知键往返，`get_settings` 同时返回强类型 `SettingsDto` 与规范 JSON
   `settings_json`；`save_settings_json` 走规范 JSON。三个 domain 枚举因 FRB 生成器将其判为 opaque，改以
   `i32` 承载（`SysProxyType`/`GirdOrientation`/`InboundProtocol`），在映射层转换，`ConfigType`/`CoreType` 保持枚举。

## 后果

- 存储层与上游逐字段等价（除我们额外保留未知键，属增强）。
- UI 通过规范 JSON 编辑，天然保留未知键与 null/空串。
- 系统代理/TUN/自启/UAC/热键注册等平台行为不在本层执行，避免越权与假成功。
