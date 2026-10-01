# T04 决策 — 存储、幂等与冲突策略

状态：接受（T04 实施）。依据：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §4/§11/§15，
`compat/fields.entities.yaml`，`compat/domain-map.yaml`。

## 1. 存储格式与所有权

- **单写者**：`persistence::Store` 是应用数据库的唯一写入入口（plan §4）。所有多行写入
  在 `unchecked_transaction()` 事务内完成。DB 访问在应用层放专用阻塞线程，不塞进异步
  runtime 工作线程（本 crate 只提供同步 API）。
- **上游兼容形状**：重建 8 张表（`SubItem/ProfileItem/ServerStatItem/RoutingItem/
  ProfileExItem/DNSItem/FullConfigTemplateItem/ProfileGroupItem`），列名=实体属性名、
  顺序与 `AppManager.cs` 建表一致；`ProtoExtra`/`TransportExtra`/`RoutingItem.RuleSet`
  保持 JSON 文本列，不展开成表。
- **应用自有表**（前缀无冲突）：`import_batches`、`id_map`、`raw_records`、
  `migration_records`、`app_meta`。
- **不依赖类型猜测**：读取一律 `SELECT *` + 列名映射；列多/列少都不崩溃（`rows.rs`）。

## 2. 导入六步与提交边界

1. `upstream_db::identify`：只读识别版本（`MAX(ConfigVersion)`）、表集合、缺失资源。
2. `snapshot`：用 **SQLite backup API** 复制到工作目录（含 WAL），计算
   `content_hash = H(config_hash : db_hash)`；禁止只拷主库。
3. 构建 **candidate**：若目标已存在，先以 backup 复制为候选（保留用户既有数据），
   再导入本批；建立 `old id → new id`（`derived_id(ns, fp:old)`）并写 `id_map`、
   原始行进 `raw_records`；迁移在插入前应用。
4. `validate_candidate`：计数、主键唯一性为**致命**；字段、引用、活动节点、路径、
   组环为**告警**（原版合法存在失效引用，不能因此拒绝导入）。
5. 报告：`ImportReport`（机器 JSON + 用户文本）。
6. `commit_candidate`：目标存在则改名为 `*.bak`，再原子改名候选为目标；失败回滚备份，
   候选删除。**失败不原地重写、不落库**。

## 3. 幂等策略

- 指纹 `source_fingerprint = SHA256(source_id ‖ content_hash)`；`source_id` 取自源路径
  规范化哈希，`content_hash` 覆盖 config+db 字节。`import_batches` 记录指纹与批次。
- 命中同指纹 → `ImportStatus::AlreadyImported`，完全不写目标；**不覆盖导入后的用户编辑**。
- 不同源导入 → 候选基于目标快照追加，不替换目标。派生 id 以各自指纹为命名空间，避免碰撞。

## 4. 冲突策略（可预测，不按名合并）

- **不与同名节点合并**：即使 Remarks 相同也各自插入（夹具中两条“香港节点 🚀”保留为两行）。
- 组 `ChildItems` 通过 id 重映射保持顺序；失效 id 原样保留并告警，不删除原表达式。
- 引用语义按类型区分：Remarks（OutboundTag/Prev/Next）、IndexId 列表（ChildItems）、
  订阅 Id+正则（SubChildItems）、`self` 哨兵（REF-ENT-006，非 self 才解析，防注入）。
  原始表达式字符串始终保留（`ReferenceExpr.raw`）。

## 5. 备份 / 恢复

- 目录形式 bundle：`manifest.json`（format_version=1、db/config 哈希、资源清单、
  计数）+ backup 快照 + config + 资源；资源相对路径拒绝 `..`/绝对/盘符。
- 恢复：先 `verify_backup`（哈希+清单），复制到候选并 `PRAGMA integrity_check`，
  再走同一个原子提交；任一失败保持现有配置不变。
- 原版 `guiConfigs/` ZIP 仅“识别 + 导入路径”（`recognize_archive` +
  `identify_archive`/`snapshot` 提取），UI 流程归 T16。

## 6. 已知取舍

- 领域模型字段为 snake_case，而上游 JSON 为 PascalCase；持久层用专用 storage 结构
  （`*Blob`/`ConfigStorage`/`RulesItemStorage`）承载上游大小写并桥接到领域模型，避免
  改动其他 crate。
- 传输迁移忠实复刻“先 V3 后 V4 同一轮”的上游语义：一轮结束后所有旧行到达 V4，
  因此 `transports_migrated` 会包含本轮刚升到 V3 的行。
- `ProfileGroupItem` 结构保留但数据不作为活动数据导入（仅迁移源，行进入 `raw_records`）。

---

## 7. 修订注记（HEAD `2fbe9f1` 之后；M-009/M-010）

### 7.1 REF-ENT-005 语义偏离（有意保留，需 T16 决策）

- **上游语义**（`work/research-v2rayn/.../Handler/Fmt/InnerFmt.cs:54-94`）：内部导入 `Resolve` 时，
  `ChildItems` 经 `indexIdMap` 重映射，**重映射失败的子 id 被丢弃**；重写后既无 `ChildItems` 也无
  `SubChildItems` 的**空组被移除**。
- **Rust 语义**（`crates/persistence/src/candidate.rs:490-504` `remap_id_list`）：失效子 id **原样保留**，
  不删除子表达式、不删空组，仅以告警（`W_REF_CHILD`）暴露。
- **判定**：这是**有意的、更保守的偏离**——保留原文可避免导入期静默丢失用户数据，符合本项目
  "不静默丢弃"原则，且原式字符串（`ReferenceExpr.raw`）本就要求保留。它**不等价于上游兼容**。
- **T16 待办**：导入 UI 需要暴露**严格/保留模式开关**：
  - 严格模式（默认，贴近上游）：丢弃无法重映射的子 id、移除空组；
  - 保留模式：保留原文并告警（当前行为）。
  在开关落地前，台账 `REF-ENT-005` 不得标记为 `verified`。

### 7.2 明文秘密载体（M-003 / M-010，本回合只做最小加固，不实现加密）

未决项登记（T16 输入清单）：

- `services/net_host/src/session.rs` staged `config.json`：**含内联节点凭据**。本回合最小加固：
  会话结束/回滚时删除 staged 配置与 `core.log`（`journal::remove_staged_artifacts`）；staged 目录与
  文件设置"仅当前用户" DACL（`net_host::dacl::restrict_to_current_user`，`D|P(A;OICI;GA;;;<sid>)`）。
  **仍未做**：落盘加密、崩溃窗口内的强删、`%TEMP%`/页文件残留防护。
- `crates/persistence/src/backup.rs` bundle 目录 / `.bak` / `raw_records`（含 `Password`/`Id`/
  `Security`）：**全明文**，无加密、无权限收紧、无轮转。**未实现加密**；T16 必须决定：
  备份目录 ACL/加密、`raw_records` 保留期与轮转、`.bak` 清理策略。
- 本地 bundle ≠ 上游兼容库：T04 自产 bundle 仅供本应用，T16 若要与上游 `guiConfigs/` 互操作需另做差分。

以上两项是**结构性安全未决项**，在 T16 收敛前相关能力不得标记 `verified`。
