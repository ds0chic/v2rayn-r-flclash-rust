# T06a 决策记录：持久化接线与编辑器表单架构

背景：T05 的节点表由 `InMemoryProfileRepository` 与合成数据支撑；T04 的 `persistence::Store`
已实现 schema/迁移/导入但未被应用层使用（除导入路径）。T06a 需要把二者接起来，并交付 11 协议编辑表单。

## 决策

### D1：在 application 内新增 `store_repo::SqliteProfileRepository`

`ProfileRepository` trait 保持不变（T02 合同）。新增 `SqliteProfileRepository` 实现同一 trait，
`ProfileStore` 枚举在 Memory 与 Sqlite 之间分派。理由：trait 合同稳定，UI/FRB 无需感知后端切换；
测试可继续用 InMemory，生产用 SQLite。

- 直接依赖 `rusqlite` 与 `persistence`；`persistence` 复用其 `Store`（唯一写入方语义不变）。
- `store.rs` 仅追加 `query_rows`/`count_query`/`execute` 三个最小 API，不改 T04 既有测试。

### D2：只把「叶子写操作」放到 application，不在 bridge 拼 SQL

`save/delete/copy/set_remarks/set_active` 都在 `AppEngine` 实现，bridge 只做 DTO↔domain 映射。
保证「单一业务写入方」在 AppEngine 层。

### D3：revision 与 active 持久化到 `guiNConfig.json`

`desired_revision` 与 `active_index_id` 写入数据目录的 `guiNConfig.json`（原子写：tmp+rename）。
理由：它们是跨重启需要延续的状态，但不是 ProfileItem 的列；`guiNConfig.json` 正是上游存放
`Config.IndexId` 的位置，避免新增业务表。profile 数据仍在 `guiNDB.db`。

### D4：`ProfileDto` 扩为全字段，拆 Security/ProtoExtra/TransportExtra 子 DTO

FRB 不能翻译 `serde_json::Map`。因此：
- `SecurityDto`(13) + `ProtocolExtraDto`(30) + `TransportExtraDto`(11) 为平铺可翻译类型；
- 未知扩展用 `extra_json`（String）承载，原样往返；
- `ProtocolExtraDto.multiple_load` 用数值（上游即数值）避免 FRB 枚举值映射歧义。

### D5：稳定 ID 由后端分配

新建节点 draft 的 `index_id` 为空，`save_profile` 在 Rust 侧调用 `application::new_index_id()`
（时间戳+进程内计数）分配，UI 不生成身份。

### D6：能力约束在 UI 与服务端之间有明确边界

UI 用 `ProfileCapabilities`（sing-box-only 三类强制 sing_box，其余 xray/sing_box；传输/安全按协议能力）
约束下拉项；编辑已存在节点时把越界的历史值（如合成夹具的 v2fly/tcp）夹到合法集合，保证下拉一致。
服务端仍以 `domain::Profile::validate` 为权威（必填/端口/枚举）。

### D7：`SyntheticBridgePort` 实现完整仓库合同

为让 widget 测试在不加载原生库时仍走「同一合同」，`SyntheticBridgePort` 实现新增的
save/delete/copy/rename/setActive/revision，行为与真实 bridge 对齐（含 `E_REVISION_STALE`、
`E_FIELD_REQUIRED`）。真实 FRB 路径另有 `t06a_frb_bridge_test.dart` 加载真实 DLL 覆盖。

## 影响

- 生产 `engine()` 改为 `AppEngine::open(data_dir)`，打开失败回退 InMemory 并打印（不静默）。
- 测试对现有 T05 widget 测试的三处交互（双击、Ctrl+D、Delete）做了确认框/取消处理，语义未变。
- 未引入新数据库；仍只有 `guiNDB.db` + `guiNConfig.json`。
