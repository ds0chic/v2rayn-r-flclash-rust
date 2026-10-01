//! Generates the synthetic upstream fixtures under `fixtures/synthetic/`.
//!
//! Run with `cargo run -p persistence --example make_fixtures`. The generated
//! databases contain only synthetic data (RFC 5737 documentation addresses and
//! `.invalid` hosts); no real credentials or subscription URLs are included.

use std::path::PathBuf;

use persistence::schema::UPSTREAM_TABLES;
use rusqlite::Connection;

fn target(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic")
        .join(name)
}

fn create_tables(conn: &Connection) {
    for table in UPSTREAM_TABLES {
        conn.execute_batch(&table.create_sql()).unwrap();
    }
}

fn main() {
    build_v2();
    build_v3();
    println!("synthetic fixtures written under fixtures/synthetic/");
}

fn build_v2() {
    let dir = target("upstream-v2");
    std::fs::create_dir_all(&dir).unwrap();
    let config = r#"{
  "IndexId": "node-hk",
  "SubIndexId": "sub-main",
  "UIItem": {
    "CurrentTheme": "Dark",
    "CurrentLanguage": "zh-Hans",
    "CurrentFontFamily": "Microsoft YaHei",
    "CurrentFontSize": 12,
    "MainGridOrientation": 1,
    "WindowSizeItem": [
      {"TypeName": "MainWindow", "Width": 1200, "Height": 800, "MainGridHeight1": 300, "MainGridHeight2": 0, "Orientation": 1}
    ],
    "MainColumnItem": [
      {"Name": "Remarks", "Width": 180, "Index": 0},
      {"Name": "TestResult", "Width": 90, "Index": 1}
    ],
    "FutureUiFlag": true
  },
  "HysteriaItem": {"UpMbps": 50, "DownMbps": 100, "HopInterval": 15},
  "SystemProxyItem": {"SysProxyType": 1, "SystemProxyExceptions": "", "CustomSystemProxyPacPath": ""},
  "BrandNewRootItem": {"enabled": true, "weight": 7}
}
"#;
    std::fs::write(dir.join("guiNConfig.json"), config).unwrap();

    let db = dir.join("guiNDB.db");
    if db.exists() {
        std::fs::remove_file(&db).unwrap();
    }
    let conn = Connection::open(&db).unwrap();
    create_tables(&conn);

    conn.execute_batch(
        r#"
        INSERT INTO SubItem (Id, Remarks, Url, MoreUrl, Enabled, UserAgent, Sort, AutoUpdateInterval, UpdateTime, CustomCoreType)
          VALUES ('sub-main', '主订阅', 'https://example.invalid/sub', '', 1, 'v2rayN/7.25.4', 0, 0, 1900000000, 24);
        INSERT INTO ProfileItem (IndexId, ConfigType, CoreType, ConfigVersion, Subid, IsSub, DisplayLog, Remarks, Address, Port, Password, Username, Network, Security, Id, ProtoExtra)
          VALUES ('node-hk', 3, NULL, 2, 'sub-main', 1, 1, '香港节点 🚀', '192.0.2.10', 8388, 'sspass', '', 'ws', 'aes-256-gcm', 'sspass', '{"SsMethod":"old","BrandNewFlag":true}');
        INSERT INTO ProfileItem (IndexId, ConfigType, CoreType, ConfigVersion, Subid, IsSub, DisplayLog, Remarks, Address, Port, Password, Username, Network, AlterId, Security, Id)
          VALUES ('node-tw', 1, NULL, 2, 'sub-main', 1, 1, '台灣節點 🇹🇼', '192.0.2.20', 443, 'uuid-1234', '', 'ws', 64, 'auto', 'uuid-1234');
        INSERT INTO ProfileItem (IndexId, ConfigType, CoreType, ConfigVersion, Subid, IsSub, DisplayLog, Remarks, Address, Port, Password, Network, Path, Ports, Id)
          VALUES ('node-hy2', 7, NULL, 2, 'sub-main', 1, 1, 'Hysteria2 德国', '198.51.100.5', 443, 'hy2pass', 'raw', 'obfspass', '443,8443', 'hy2pass');
        INSERT INTO ProfileItem (IndexId, ConfigType, CoreType, ConfigVersion, Subid, IsSub, DisplayLog, Remarks, Address, Port, Password, Network, PublicKey, RequestHost, Path, ShortId, Id)
          VALUES ('node-wg', 9, NULL, 2, '', 0, 1, 'WireGuard 内网', '203.0.113.1', 51820, 'wgpass', 'raw', 'public-key', '10.0.0.2/32', 'reserved-token', '1420', 'wgpass');
        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks, Subid)
          VALUES ('group-101', 101, 2, '策略组', 'sub-main');
        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks)
          VALUES ('group-102', 102, 2, '链式组');
        INSERT INTO ProfileItem (IndexId, ConfigType, CoreType, ConfigVersion, Subid, IsSub, DisplayLog, Remarks, Address, Port, Password, Network, Security, Id)
          VALUES ('node-hk-dup', 3, NULL, 2, 'sub-main', 1, 1, '香港节点 🚀', '192.0.2.11', 8388, 'dup', 'ws', 'aes-256-gcm', 'dup');

        INSERT INTO ProfileGroupItem (IndexId, ChildItems, SubChildItems, Filter, MultipleLoad)
          VALUES ('group-101', 'node-hk,node-tw,missing-id', 'sub-main', '^香港', 1);
        INSERT INTO ProfileGroupItem (IndexId, ChildItems, MultipleLoad)
          VALUES ('group-102', '', 0);

        INSERT INTO RoutingItem (Id, Remarks, Url, RuleSet, RuleNum, Enabled, Locked, IsActive, Sort)
          VALUES ('route-1', '默认路由', '', '[{"Id":"r1","Type":"field","OutboundTag":"香港节点 🚀","Domain":["example.com"],"Enabled":true,"RuleType":1},{"Id":"r2","Type":"field","OutboundTag":"不存在","Enabled":true,"RuleType":1}]', 2, 1, 0, 1, 0);

        INSERT INTO DNSItem (Id, Remarks, Enabled, CoreType, UseSystemHosts, NormalDNS, TunDNS)
          VALUES ('dns-1', '默认DNS', 1, 2, 0, '1.1.1.1', '8.8.8.8');

        INSERT INTO FullConfigTemplateItem (Id, Remarks, Enabled, CoreType, Config, TunConfig, AddProxyOnly)
          VALUES ('tpl-1', 'Xray 模板', 1, 2, '{"log":{"loglevel":"warning"}}', NULL, 0);

        INSERT INTO ServerStatItem (IndexId, TotalUp, TotalDown, TodayUp, TodayDown, DateNow)
          VALUES ('node-hk', 1000, 2000, 10, 20, 19000);

        INSERT INTO ProfileExItem (IndexId, Delay, Speed, Sort, Message, IpInfo)
          VALUES ('node-hk', 120, 1.5, 0, 'ok', '192.0.2.10');
        "#,
    )
    .unwrap();
    conn.close().unwrap();
}

fn build_v3() {
    let dir = target("upstream-v3");
    std::fs::create_dir_all(&dir).unwrap();
    let config = r#"{
  "IndexId": "node-ws",
  "SubIndexId": "sub-v3",
  "UIItem": {"CurrentTheme": "Light", "CurrentLanguage": "en"},
  "SystemProxyItem": {"SysProxyType": 2}
}
"#;
    std::fs::write(dir.join("guiNConfig.json"), config).unwrap();

    let db = dir.join("guiNDB.db");
    if db.exists() {
        std::fs::remove_file(&db).unwrap();
    }
    let conn = Connection::open(&db).unwrap();
    create_tables(&conn);

    conn.execute_batch(
        r#"
        INSERT INTO SubItem (Id, Remarks, Url, Enabled)
          VALUES ('sub-v3', 'V3 订阅', 'https://example.invalid/v3', 1);

        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Address, Port, Password, Network, RequestHost, Path, HeaderType, Id)
          VALUES ('node-ws', 5, 3, 'sub-v3', 'WS 节点', '192.0.2.30', 443, 'uuid-ws', 'ws', 'ws.example.invalid', '/ws', '', 'uuid-ws');
        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Address, Port, Password, Network, RequestHost, Path, HeaderType, Id)
          VALUES ('node-grpc', 5, 3, 'sub-v3', 'gRPC 节点', '192.0.2.31', 443, 'uuid-grpc', 'grpc', 'grpc.example.invalid', 'svc', 'multi', 'uuid-grpc');
        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Address, Port, Password, Network, HeaderType, Path, Id)
          VALUES ('node-kcp', 5, 3, 'sub-v3', 'KCP 节点', '192.0.2.32', 443, 'uuid-kcp', 'kcp', 'srtp', 'seed-token', 'uuid-kcp');
        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Address, Port, Password, Network, RequestHost, Path, Id)
          VALUES ('node-tcp-alias', 5, 3, 'sub-v3', 'TCP 别名节点', '192.0.2.33', 443, 'uuid-tcp', 'tcp', 'tcp.example.invalid', '/tcp', 'uuid-tcp');
        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Address, Port, Password, Network, RequestHost, Id)
          VALUES ('node-h2-legacy', 5, 3, 'sub-v3', 'H2 遗留节点', '192.0.2.34', 443, 'uuid-h2', 'h2', 'h2.example.invalid', 'uuid-h2');
        INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks, ProtoExtra)
          VALUES ('group-v3', 101, 3, 'V3 策略组', '{"GroupType":"PolicyGroup","ChildItems":"node-ws,node-grpc","MultipleLoad":0}');

        INSERT INTO ProfileGroupItem (IndexId, ChildItems, MultipleLoad)
          VALUES ('group-v3', 'node-ws,node-grpc', 0);
        "#,
    )
    .unwrap();
    conn.close().unwrap();
}
