# T04b — 真实 v2rayN 配置兼容验证

验证 T04 导入/迁移对**真实上游数据形状**的兼容性；修复缺陷；产出脱敏夹具。
真实数据只读，未提交；本文不含任何地址、端口、凭据、订阅 URL 或备注。

## 1. 真实数据来源与完整性

只读来源（用户授权，位于临时目录，未纳入仓库）：

| 文件 | 结构要点 |
|---|---|
| `guiNConfig.json` | 25 个根键；`UiItem` 14 窗口 / 14 主列；`ClashUIItem` 5 列 |
| `guiNDB.db` | 8 表；`ConfigVersion`=4；41 列 `ProfileItem`（含 `EchForceQuery`） |
| `upstream-old-20260602.db` | `ConfigVersion`=2；35 列 `ProfileItem`（无 `ProtoExtra`/`TransportExtra`） |
| `upstream-old-1780395234.db` | `ConfigVersion`=2；35 列 `ProfileItem` |
| `upstream-bak-1788539222.db` | `ConfigVersion`=4；含 `ConfigType` 11(Anytls)/3(Shadowsocks) |
| `upstream-old-config-20260602.json` | 旧版 13 窗口 / 13 主列 |

所有真实库 `PRAGMA integrity_check` = `ok`。真实源文件 SHA256（前 16 位，仅作只读证明）见报告末尾；
验证全程未写入真实目录（见 §5 回归断言）。

## 2. 结构核验（仅结构/计数，不含值）

对每个真实库只读打开：表 / 列数 / 列顺序 / 行数 / 声明类型 / `ConfigVersion` 分布 /
`ConfigType` 分布 / 非 ASCII 与 emoji 计数，与 `crates/persistence/src/schema.rs` 对比。

### 2.1 行数与版本/类型分布（真实库）

| 库 | ProfileItem | SubItem | ServerStat | Routing | ProfileEx | DNS | FullCfgTpl | ProfileGroup |
|---|---|---|---|---|---|---|---|---|
| guiNDB (v4) | 40 | 1 | 10 | 4 | 78 | 2 | 2 | 0 |
| old-20260602 (v2) | 90 | 6 | 0 | 3 | 90 | 2 | 2 | 0 |
| old-1780395234 (v2) | 10 | 9 | 0 | 3 | 470 | 2 | 2 | 0 |
| bak-1788539222 (v4) | 454 | 13 | 85 | 4 | 454 | 2 | 2 | 0 |

| 库 | ConfigVersion 分布 | ConfigType 分布 | Remarks 非ASCII / emoji |
|---|---|---|---|
| guiNDB | {4:40} | {5:27, 7:13} | 40 / 0 |
| old-20260602 | {2:90} | {5:88, 7:2} | 74 / 6 |
| old-1780395234 | {2:10} | {5:8, 7:2} | 9 / 0 |
| bak-1788539222 | {4:454} | {3:2, 5:372, 7:55, 11:25} | 240 / 81 |

### 2.2 与 `schema.rs` 的结构差异（真实上游 vs 冻结 7.25.4）

声明类型：真实库为 sqlite-net 的 `varchar` / `INTEGER` / `float`；`schema.rs` 用
`TEXT`/`INTEGER`/`REAL` 重建。SQLite 类型亲和性等价，导入按列名映射，不受影响。

**列数差异**

| 表 | schema.rs | 真实 v2 (35列系) | 真实 v4 (41列系) | 结论 |
|---|---|---|---|---|
| `ProfileItem` | 41 | 35 | 41 | 旧库缺 `Password/Username/VerifyPeerCertByName/Finalmask/ProtoExtra/TransportExtra`，导入后由迁移补齐 |
| `SubItem` | 17 | 15 | 16 | 真实上游缺 `RequestHeaders`（旧库另缺 `CustomCoreType`）；候选库仍建全 17 列 |
| `ProfileExItem` | 6 | 5 | 6 | 旧库缺 `IpInfo` |
| 其余 5 表 | 一致 | 一致 | 一致 | — |

**列顺序差异（重要）**：真实 v4 库 `ProfileItem` 的列顺序与 `schema.rs`（冻结 7.25.4
C# 属性顺序）**完全不同**：

- schema.rs：`IndexId, ConfigType, CoreType, ConfigVersion, Subid, IsSub, PreSocksPort, DisplayLog, Remarks, Address, Port, ...`
- 真实 v4：`IndexId, ConfigType, ConfigVersion, Address, Port, Ports, Id, AlterId, Security, Network, Remarks, HeaderType, ...`

导入器全程按**列名**读取（`rows.rs::read_table` 用 `stmt.column_names()` 建名→值映射，
`SELECT *` 后按名取值），因此列序差异被安全吸收；这正是 P0 兼容性保证点。该形状已由
`tests/real_shape.rs` 的脱敏夹具固定为回归。

## 3. 缺陷与修复

### 3.1 缺陷 D1（夹具工具，已修复）：脱敏破坏内嵌 JSON 结构

`tools/sanitize_upstream.py` 原先把**全部** TEXT 值替换为随机 ASCII，导致
`ProfileItem.ProtoExtra` / `TransportExtra`、`ProfileItem.Extra`、`RoutingItem.RuleSet`
等**内嵌 JSON 列**被破坏为随机文本（如 `kvkpfltx...`）。导入器对这些列做
`serde_json::from_str`（`mapping.rs:103-104`、`mapping.rs:377`），于是基于该夹具的
4 个 `real_shape` 用例崩溃为 `Json(Error("expected value", line: 1, column: 1))`。

真实上游始终在这些列写入合法 JSON（实测：guiNDB `ProtoExtra` 40/40 合法，
`TransportExtra` 40/40 合法，`RuleSet` 4/4 合法，键集固定，无嵌套 JSON 字符串），
因此这是**夹具保真度缺陷**，不是导入器缺陷（导入器对真实文件工作正常，见 §4）。

修复：对 4 个内嵌 JSON 列（`ProtoExtra`/`TransportExtra`/`Extra`/`RuleSet`）保留 JSON 结构、
键名与非字符串叶子，仅递归替换字符串叶子；键名是 schema 而非秘密。详见
`tools/sanitize_upstream.py::Sanitizer.sanitize_json`。

### 3.2 缺陷 D2（扫描语义，已修复）：子串扫描与 schema 令牌误报

结构保留后，原始普通值可能恰好等于/包含被保留的 JSON 键或数字/布尔叶子
（如原值 `Host`/`Network`/`Port`/`Remarks`/`Type`/`http`/`true`，以及 `'0'`∈`1280`），
以及全空 `ProtoExtra` 结构值 `{"SalamanderPass":"","Ports":"","GeckoMinPacketSize":"","GeckoMaxPacketSize":""}`
会逐字复现。修复：

- 泄漏扫描与 poison 集合改为**用户承载值**口径：普通 TEXT 单元格 ∪ 内嵌 JSON 的字符串叶子；
  排除 JSON 键与非字符串叶子（schema/结构）。
- Pass 1 同时把 JSON 字符串叶子 `learn` 进 poison，消除叶子值（如 `udp`）在随机产物中的子串碰撞。

结果：2780 个原始用户承载值，**0 命中**（见 §5）。

### 3.3 缺陷 D3（用例期望，已修复）：断言真实值

`tests/real_shape.rs` 曾断言 `language == "zh-Hans"`（真实值）。夹具脱敏后该值成为合成值。
改为断言结构（存在且非空）与结构计数（14/14/5），不再绑定真实内容。

### 3.4 回归用例新增

- `real_shape_source_with_wal_imports_committed_content`：真实形状库切 WAL 后写入未 checkpoint
  的行，导入必须包含（经 backup API 快照）。
- `truncated_real_shape_db_errors_without_panicking`：真实形状库截断到 1/3，导入返回
  干净错误或成功提交，绝不 panic，且不残留半写目标库。

## 4. 导入 / 迁移兼容（只读，候选库流程）

`import_from_path` 对真实文件（只读）逐库执行，记录计数与迁移命中（不含任何
Remarks/URL/凭据）：

| 源 | status | source_version | 迁移命中 | warnings | errors | 二次导入 |
|---|---|---|---|---|---|---|
| guiNDB (v4) | Imported | 4 | 无 | 0 | 0 | AlreadyImported |
| old-20260602 (v2) | Imported | 2 | `MIG-ENT-003` 90, `MIG-ENT-004` 90 | 0 | 0 | AlreadyImported |
| old-1780395234 (v2) | Imported | 2 | `MIG-ENT-003` 10, `MIG-ENT-004` 10 | 0 | 0 | AlreadyImported |
| bak-1788539222 (v4) | Imported | 4 | 无 | 0 | 0 | AlreadyImported |

- 导入行数与源行数逐表一致（如 bak：ProfileItem 454、SubItem 13、ServerStatItem 85、ProfileExItem 454）。
- 迁移后全部 `ProfileItem.ConfigVersion` = 4；`ConfigType` 11(Anytls)、3(Shadowsocks) 原样保留。
- 无未知列错误、无致命校验错误、无崩溃。
- **引用解析**：`Subid`↔`SubItem.Id`、订阅 Prev/Next、组 `ChildItems` 均按既有规则解析；
  导入仓库元数据 `last_import_fingerprint` 存在（计数口径，未记录具体标签）。

## 5. 幂等与只读

- 同库两次导入：第二次 `AlreadyImported`、`committed=false`、行数不变。
- 同字节复制到**另一目录**再导入：仍 `AlreadyImported`（内容寻址身份，排除路径）。
- 导入真实库期间源目录字节不变（`real_old_v1...` 用例断言导入前后 `guiNDB.db` 字节相等）。

## 6. 脱敏夹具与 0 命中证据

夹具目录 `fixtures/synthetic/upstream-real-shape/`：

| 文件 | 派生自 | 结构 |
|---|---|---|
| `guiNConfig.json` | `guiNConfig.json` | 25 根键；14 窗口 / 14 主列；5 Clash 列 |
| `guiNDB.db` | `guiNDB.db` | v4；41 列 `ProfileItem`（含 `EchForceQuery`） |
| `upstream-old-v1.db` | `upstream-old-20260602.db` | v2；35 列 |
| `upstream-old-v2.db` | `upstream-old-1780395234.db` | v2；35 列 |
| `upstream-bak.db` | `upstream-bak-1788539222.db` | v4；`ConfigType` 含 11/3 |

脱敏器输出（`tools/sanitize_upstream.py`）：

```
learned 2780 distinct original payload values
  guiNDB.db: {ProfileItem:40, SubItem:1, ServerStatItem:10, RoutingItem:4, ProfileExItem:78, DNSItem:2, FullConfigTemplateItem:2, ProfileGroupItem:0}
  upstream-old-v1.db: {ProfileItem:90, SubItem:6, ServerStatItem:0, RoutingItem:3, ProfileExItem:90, ...}
  upstream-old-v2.db: {ProfileItem:10, SubItem:9, ServerStatItem:0, RoutingItem:3, ProfileExItem:470, ...}
  upstream-bak.db: {ProfileItem:454, SubItem:13, ServerStatItem:85, RoutingItem:4, ProfileExItem:454, ...}
  guiNConfig.json: sanitized
scan: 2780 original payload values checked, 0 hits
scan: 0 hits -- no original payload value survives in the sanitized corpus
```

- 结构/类型/长度量级/非 ASCII/emoji 有无/版本分布与源**完全一致**（逐库对比见 §2.1：
  fixtures 与 real 的行数、`ConfigVersion`/`ConfigType` 分布、Remarks 非ASCII/emoji 计数一致）。
- 两次运行夹具 SHA256 完全一致（确定性）。
- 夹具内嵌 JSON 列 `badjson=0`，键集与真实一致。

## 7. 门禁结果

在 `crates/persistence`：

```
cargo fmt -p persistence --check                     -> exit 0（无 diff）
cargo clippy -p persistence --all-targets --locked -- -D warnings  -> exit 0（无告警）
cargo test -p persistence --locked                   -> exit 0
```

测试汇总：lib 63 + edge_cases 5 + real_shape 8 + upstream_import 8 = **84 passed / 0 failed**；
T04 既有 74 用例保持全绿。

## 8. 未决项

- 真实 v4 `ProfileItem` 列序与 `schema.rs`（冻结 7.25.4）不一致，已由按名映射吸收并回归固定；
  若上游后续新增列，`rows.rs` 会保留到 `raw_records`，但不会出现在候选库固定列中——沿用现策略。
- `SubItem.RequestHeaders` 在真实上游缺失；候选库仍建该列（T04 既有行为），未改动。
- 未验证更旧/更新的上游版本（如 v1 `ConfigVersion`=1）；仅在 v2/v4 真实形状上验证。
- 真实数据未纳入仓库；夹具为单向派生，禁止还原。

## 附：真实源文件 SHA256（前 16 位，只读证明）

`guiNConfig.json 4F1688124DF71E58`、`guiNDB.db 1B26F52F0C5289F6`、
`upstream-old-20260602.db 26BEF32D934ACAD5`、`upstream-old-1780395234.db 3E7173180A8E12E2`、
`upstream-bak-1788539222.db AA978C76FFD1C635`、`upstream-old-config-20260602.json 21083C75868E595E`。
