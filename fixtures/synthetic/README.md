# fixtures/synthetic — T04 合成上游夹具

本目录只包含**合成**数据，用于 T04（crates/persistence）的导入/迁移测试。
不包含任何真实用户配置、订阅地址、凭据或本机路径。地址使用 RFC 5737 文档网段
（192.0.2.0/24、198.51.100.0/24、203.0.113.0/24），主机名使用 `.invalid`。

## 生成方式

```
cargo run -p persistence --example make_fixtures
```

生成器位于 `crates/persistence/examples/make_fixtures.rs`，可重复运行，输出稳定。

## 文件

| 路径 | 内容 |
|---|---|
| `upstream-v2/guiNConfig.json` | 含主题/语言/字体、窗口与列状态、`BrandNewRootItem` 未知键、`HysteriaItem` 全局值 |
| `upstream-v2/guiNDB.db` | 8 张上游表；7 条 `ProfileItem`（含 `ConfigVersion=2`、101/102 组、重复 Remarks、中文/emoji 名称、SS/VMess/Hysteria2/WireGuard）、`ProfileGroupItem` 迁移源、路由规则（含失效 OutboundTag）、DNS/模板/统计/测速行 |
| `upstream-v3/guiNConfig.json` | 最小配置 |
| `upstream-v3/guiNDB.db` | `ConfigVersion=3` 节点，覆盖 ws/grpc/kcp/tcp 别名/h2 遗留传输，以及一个 V3 组 |

## 用途

- `crates/persistence/tests/upstream_import.rs`：V2/V3 迁移、幂等、损坏库、拒绝路径。
- `crates/persistence/tests/edge_cases.rs`：空配置、全字段、null/空串/零值边界、失败不落库。

夹具目录在测试中**只读**；导入始终写入临时目录。
