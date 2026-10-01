//! Storage schema.
//!
//! The eight upstream `guiNDB.db` tables are recreated in the sqlite-net shape
//! (column name == CLR property name, order preserved) so an imported candidate
//! database stays recognisable and diffable against upstream. `ProtoExtra`,
//! `TransportExtra` and `RoutingItem.RuleSet` remain JSON text columns and are
//! never expanded into tables (plan §11 / `compat/fields.entities.yaml`).

/// SQLite storage class used to recreate an upstream column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlType {
    Text,
    Integer,
    Real,
}

impl SqlType {
    pub const fn sql(self) -> &'static str {
        match self {
            SqlType::Text => "TEXT",
            SqlType::Integer => "INTEGER",
            SqlType::Real => "REAL",
        }
    }
}

/// One column of an upstream table.
#[derive(Debug, Clone, Copy)]
pub struct Column {
    pub name: &'static str,
    pub ty: SqlType,
    pub primary_key: bool,
}

const fn pk(name: &'static str) -> Column {
    Column {
        name,
        ty: SqlType::Text,
        primary_key: true,
    }
}

const fn text(name: &'static str) -> Column {
    Column {
        name,
        ty: SqlType::Text,
        primary_key: false,
    }
}

const fn int(name: &'static str) -> Column {
    Column {
        name,
        ty: SqlType::Integer,
        primary_key: false,
    }
}

const fn real(name: &'static str) -> Column {
    Column {
        name,
        ty: SqlType::Real,
        primary_key: false,
    }
}

/// One upstream-compatible table.
#[derive(Debug, Clone, Copy)]
pub struct Table {
    pub name: &'static str,
    pub columns: &'static [Column],
}

impl Table {
    pub fn create_sql(&self) -> String {
        let mut sql = format!("CREATE TABLE IF NOT EXISTS \"{}\" (", self.name);
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push('"');
            sql.push_str(col.name);
            sql.push_str("\" ");
            sql.push_str(col.ty.sql());
            if col.primary_key {
                sql.push_str(" PRIMARY KEY NOT NULL");
            }
        }
        sql.push(')');
        sql
    }
}

const PROFILE_ITEM: &[Column] = &[
    pk("IndexId"),
    int("ConfigType"),
    int("CoreType"),
    int("ConfigVersion"),
    text("Subid"),
    int("IsSub"),
    int("PreSocksPort"),
    int("DisplayLog"),
    text("Remarks"),
    text("Address"),
    int("Port"),
    text("Password"),
    text("Username"),
    text("Network"),
    text("HeaderType"),
    text("RequestHost"),
    text("Path"),
    text("StreamSecurity"),
    text("AllowInsecure"),
    text("Sni"),
    text("Alpn"),
    text("Fingerprint"),
    text("PublicKey"),
    text("ShortId"),
    text("SpiderX"),
    text("Mldsa65Verify"),
    text("Extra"),
    int("MuxEnabled"),
    text("Cert"),
    text("CertSha"),
    text("EchConfigList"),
    text("VerifyPeerCertByName"),
    text("Finalmask"),
    text("ProtoExtra"),
    text("TransportExtra"),
    text("Ports"),
    int("AlterId"),
    text("Flow"),
    text("Id"),
    text("Security"),
];

const SUB_ITEM: &[Column] = &[
    pk("Id"),
    text("Remarks"),
    text("Url"),
    text("MoreUrl"),
    int("Enabled"),
    text("UserAgent"),
    text("RequestHeaders"),
    int("Sort"),
    text("Filter"),
    int("AutoUpdateInterval"),
    int("UpdateTime"),
    text("ConvertTarget"),
    text("PrevProfile"),
    text("NextProfile"),
    int("PreSocksPort"),
    text("Memo"),
    int("CustomCoreType"),
];

const SERVER_STAT_ITEM: &[Column] = &[
    pk("IndexId"),
    int("TotalUp"),
    int("TotalDown"),
    int("TodayUp"),
    int("TodayDown"),
    int("DateNow"),
];

const ROUTING_ITEM: &[Column] = &[
    pk("Id"),
    text("Remarks"),
    text("Url"),
    text("RuleSet"),
    int("RuleNum"),
    int("Enabled"),
    int("Locked"),
    text("CustomIcon"),
    text("CustomRulesetPath4Singbox"),
    text("DomainStrategy"),
    text("DomainStrategy4Singbox"),
    int("Sort"),
    int("IsActive"),
];

const PROFILE_EX_ITEM: &[Column] = &[
    pk("IndexId"),
    int("Delay"),
    real("Speed"),
    int("Sort"),
    text("Message"),
    text("IpInfo"),
];

const DNS_ITEM: &[Column] = &[
    pk("Id"),
    text("Remarks"),
    int("Enabled"),
    int("CoreType"),
    int("UseSystemHosts"),
    text("NormalDNS"),
    text("TunDNS"),
    text("DomainStrategy4Freedom"),
    text("DomainDNSAddress"),
];

const FULL_CONFIG_TEMPLATE_ITEM: &[Column] = &[
    pk("Id"),
    text("Remarks"),
    int("Enabled"),
    int("CoreType"),
    text("Config"),
    text("TunConfig"),
    int("AddProxyOnly"),
    text("ProxyDetour"),
];

const PROFILE_GROUP_ITEM: &[Column] = &[
    pk("IndexId"),
    text("ChildItems"),
    text("SubChildItems"),
    text("Filter"),
    int("MultipleLoad"),
];

/// The eight upstream tables, in `AppManager.cs` creation order.
pub static UPSTREAM_TABLES: &[Table] = &[
    Table {
        name: "SubItem",
        columns: SUB_ITEM,
    },
    Table {
        name: "ProfileItem",
        columns: PROFILE_ITEM,
    },
    Table {
        name: "ServerStatItem",
        columns: SERVER_STAT_ITEM,
    },
    Table {
        name: "RoutingItem",
        columns: ROUTING_ITEM,
    },
    Table {
        name: "ProfileExItem",
        columns: PROFILE_EX_ITEM,
    },
    Table {
        name: "DNSItem",
        columns: DNS_ITEM,
    },
    Table {
        name: "FullConfigTemplateItem",
        columns: FULL_CONFIG_TEMPLATE_ITEM,
    },
    Table {
        name: "ProfileGroupItem",
        columns: PROFILE_GROUP_ITEM,
    },
];

/// Look up an upstream table spec by name.
pub fn upstream_table(name: &str) -> Option<&'static Table> {
    UPSTREAM_TABLES.iter().find(|t| t.name == name)
}

/// Application-owned tables: import bookkeeping, id remapping, raw record
/// retention, migration log and small key/value metadata. Names are prefixed so
/// they never collide with the upstream table set.
pub static APP_TABLES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS \"import_batches\" (\
        \"source_fingerprint\" TEXT PRIMARY KEY NOT NULL, \
        \"source_id\" TEXT NOT NULL, \"content_hash\" TEXT NOT NULL, \
        \"batch_id\" TEXT NOT NULL, \"source_kind\" TEXT NOT NULL, \
        \"source_version\" INTEGER NOT NULL, \"imported_at\" INTEGER NOT NULL, \
        \"entity_counts\" TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS \"id_map\" (\
        \"batch_id\" TEXT NOT NULL, \"source_table\" TEXT NOT NULL, \
        \"source_id\" TEXT NOT NULL, \"new_id\" TEXT NOT NULL, \
        PRIMARY KEY (\"batch_id\", \"source_table\", \"source_id\"))",
    "CREATE TABLE IF NOT EXISTS \"raw_records\" (\
        \"id\" INTEGER PRIMARY KEY AUTOINCREMENT, \"batch_id\" TEXT NOT NULL, \
        \"source_table\" TEXT NOT NULL, \"source_id\" TEXT, \"raw_json\" TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS \"migration_records\" (\
        \"migration_id\" TEXT NOT NULL, \"source_hash\" TEXT NOT NULL, \
        \"from_version\" INTEGER NOT NULL, \"to_version\" INTEGER NOT NULL, \
        \"applied_at\" INTEGER NOT NULL, \"entities_touched\" INTEGER NOT NULL, \
        \"notes\" TEXT, PRIMARY KEY (\"migration_id\", \"source_hash\"))",
    "CREATE TABLE IF NOT EXISTS \"app_meta\" (\
        \"key\" TEXT PRIMARY KEY NOT NULL, \"value\" TEXT NOT NULL)",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_item_has_forty_columns() {
        assert_eq!(PROFILE_ITEM.len(), 40);
        assert_eq!(SUB_ITEM.len(), 17);
        assert_eq!(SERVER_STAT_ITEM.len(), 6);
        assert_eq!(ROUTING_ITEM.len(), 13);
        assert_eq!(PROFILE_EX_ITEM.len(), 6);
        assert_eq!(DNS_ITEM.len(), 9);
        assert_eq!(FULL_CONFIG_TEMPLATE_ITEM.len(), 8);
        assert_eq!(PROFILE_GROUP_ITEM.len(), 5);
    }

    #[test]
    fn create_sql_quotes_names_and_marks_pk() {
        let sql = upstream_table("SubItem").unwrap().create_sql();
        assert!(sql.contains("\"Id\" TEXT PRIMARY KEY NOT NULL"));
        let p = upstream_table("ProfileItem").unwrap().create_sql();
        assert!(p.starts_with("CREATE TABLE IF NOT EXISTS \"ProfileItem\""));
        assert!(p.contains("\"Port\" INTEGER"));
    }
}
