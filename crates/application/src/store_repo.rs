//! SQLite-backed [`ProfileRepository`] plus a small enum that lets the engine
//! run either against real storage or the in-memory store used by tests.
//!
//! The mapping mirrors `compat/domain-map.yaml` (`ProfileItem` 40 columns +
//! `ProtoExtra` 30 + `TransportExtra` 11). Legacy columns that upstream keeps
//! only for migration are preserved in `Profile::extra` so a round-trip never
//! drops them.

use domain::{codes, ConfigType, CoreType, DomainError, Profile, SecurityParams, TrafficStats};
use persistence::mapping::map_traffic;
use persistence::rows::RawRow;
use persistence::{ProtocolExtraBlob, Store, TransportExtraBlob};
use rusqlite::types::ToSql;
use serde_json::{json, Value};

use crate::repository::{
    InMemoryProfileRepository, PageRequest, ProfileFilter, ProfilePage, ProfileRepository,
    ProfileSort, SubRepository,
};
use crate::subs::SubItem;

/// Columns retained verbatim in `Profile::extra` (superseded by the JSON
/// blobs, but still present in an imported database).
const LEGACY_COLUMNS: &[&str] = &[
    "HeaderType",
    "RequestHost",
    "Path",
    "Extra",
    "Ports",
    "AlterId",
    "Flow",
    "Id",
    "Security",
];

/// Map a persistence failure onto the shared error contract.
pub fn storage_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::new(codes::INTERNAL, "error.storage").with_detail(error.to_string())
}

/// Classified storage failure: preserves the persistence layer's stable code
/// and message key so the UI can distinguish a corrupt database, a read-only
/// directory, a full disk or a lock conflict instead of a single opaque
/// `error.storage` (R4-27). Transient IO/SQLite failures are retryable.
pub fn persistence_storage_error(error: persistence::PersistenceError) -> DomainError {
    let retryable = matches!(
        error,
        persistence::PersistenceError::Io(_) | persistence::PersistenceError::Sqlite(_)
    );
    let domain = DomainError::new(error.code(), error.message_key()).with_detail(error.to_string());
    if retryable {
        domain.retryable()
    } else {
        domain
    }
}

/// A SQLite-backed profile repository.
pub struct SqliteProfileRepository {
    store: Store,
}

impl SqliteProfileRepository {
    /// Open (creating if needed) the database at `path`.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, DomainError> {
        let store = Store::create(path).map_err(persistence_storage_error)?;
        Ok(Self { store })
    }

    /// Wrap an already-open store (used by import/migration tests).
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Atomically replace one subscription's *subscription-sourced* nodes in a
    /// single SQLite transaction: delete the existing `Subid = subid AND
    /// IsSub = 1` rows, then upsert the replacement set. Manual nodes in the
    /// same group (`IsSub = 0`, e.g. a batch import) are preserved, matching
    /// upstream `RemoveServersViaSubid(config, subid, isSub: true)`
    /// (`ConfigHandler.cs:2246-2259`). Any failure rolls the transaction back
    /// so the old nodes survive. `fail_after_delete` is a test-only injection
    /// that errors after the deletes (before commit) to prove the rollback.
    ///
    /// After the node upsert the matched `ServerStatItem` rows are cloned from
    /// the old identity to the new one inside the same transaction (upstream
    /// `CloneServerStatItem`, `ConfigHandler.cs:2120-2131`), so traffic totals
    /// follow a node whose stable id changed while its transport identity did
    /// not. `(added, removed)` reports the replacement set size and the number
    /// of replaced subscription rows, not the group's total (manual nodes are
    /// not "added" by an update).
    pub fn replace_for_sub(
        &self,
        subid: &str,
        profiles: Vec<Profile>,
        remove_existing: bool,
        fail_after_delete: bool,
    ) -> Result<(usize, usize), DomainError> {
        let old_profiles: Vec<Profile> = self
            .store
            .query_rows(
                "SELECT * FROM \"ProfileItem\" WHERE \"Subid\" = ?1",
                &[&subid],
            )
            .map_err(persistence_storage_error)?
            .iter()
            .map(profile_from_row)
            .collect();
        let to_delete: Vec<String> = if remove_existing {
            old_profiles
                .iter()
                .filter(|p| p.is_sub)
                .map(|p| p.index_id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let removed = to_delete.len();
        let tx = self.store.begin().map_err(persistence_storage_error)?;
        for id in &to_delete {
            tx.execute(
                "DELETE FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
                rusqlite::params![id],
            )
            .map_err(|error| persistence_storage_error(error.into()))?;
        }
        if fail_after_delete {
            return Err(DomainError::new(codes::INTERNAL, "error.storage")
                .with_detail("injected failure after delete"));
        }
        for profile in &profiles {
            let row = row_from_profile(profile);
            self.store
                .upsert_row(&tx, &row)
                .map_err(persistence_storage_error)?;
        }
        if remove_existing {
            for profile in &profiles {
                let Some(old) = crate::subs::find_matched_profile(&old_profiles, profile) else {
                    continue;
                };
                if old.index_id == profile.index_id {
                    continue;
                }
                let stat = self
                    .store
                    .query_rows(
                        "SELECT * FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                        &[&old.index_id],
                    )
                    .map_err(persistence_storage_error)?;
                for mut cloned in stat {
                    cloned.table = "ServerStatItem".to_string();
                    cloned.set("IndexId", json!(profile.index_id.clone()));
                    self.store
                        .upsert_row(&tx, &cloned)
                        .map_err(persistence_storage_error)?;
                }
            }
        }
        tx.commit()
            .map_err(|error| persistence_storage_error(error.into()))?;
        Ok((profiles.len(), removed))
    }
}

impl ProfileRepository for SqliteProfileRepository {
    fn get(&self, index_id: &str) -> Result<Option<Profile>, DomainError> {
        let rows = self
            .store
            .query_rows(
                "SELECT * FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
                &[&index_id],
            )
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(profile_from_row))
    }

    fn upsert(&mut self, profile: Profile) -> Result<(), DomainError> {
        let row = row_from_profile(&profile);
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, index_id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute(
                "DELETE FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
                &[&index_id],
            )
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn query(
        &self,
        filter: &ProfileFilter,
        sort: ProfileSort,
        page: PageRequest,
    ) -> Result<ProfilePage, DomainError> {
        let (where_sql, params) = build_where(filter);

        let count_sql = format!("SELECT COUNT(*) FROM \"ProfileItem\"{where_sql}");
        let count_params: Vec<&dyn ToSql> = params.iter().map(|s| s as &dyn ToSql).collect();
        let total = self
            .store
            .count_query(&count_sql, &count_params)
            .map_err(persistence_storage_error)? as usize;

        let order_sql = match sort {
            ProfileSort::Remarks => " ORDER BY \"Remarks\" COLLATE NOCASE ASC, \"IndexId\" ASC",
            ProfileSort::Address => " ORDER BY \"Address\" ASC, \"IndexId\" ASC",
            ProfileSort::Delay => {
                " ORDER BY (SELECT \"Delay\" FROM \"ProfileExItem\" e WHERE e.\"IndexId\" = \
                 \"ProfileItem\".\"IndexId\") ASC, \"IndexId\" ASC"
            }
            ProfileSort::IndexId => " ORDER BY rowid ASC",
        };
        let limit = page.page_size.max(1) as i64;
        let offset = page.cursor as i64;
        let limit_index = params.len() + 1;
        let offset_index = params.len() + 2;
        let sql = format!(
            "SELECT * FROM \"ProfileItem\"{where_sql}{order_sql} LIMIT ?{limit_index} OFFSET ?{offset_index}"
        );
        let mut all_params: Vec<&dyn ToSql> = params.iter().map(|s| s as &dyn ToSql).collect();
        all_params.push(&limit);
        all_params.push(&offset);

        let rows = self
            .store
            .query_rows(&sql, &all_params)
            .map_err(persistence_storage_error)?;
        let items: Vec<Profile> = rows.iter().map(profile_from_row).collect();
        let end = page.cursor + items.len();
        let next_cursor = if end < total { Some(end) } else { None };
        Ok(ProfilePage {
            items,
            total,
            next_cursor,
        })
    }

    fn count(&self) -> usize {
        self.store.count_rows("ProfileItem").unwrap_or(0) as usize
    }
}

/// A SQLite-backed subscription repository sharing the profile store's `Store`.
pub struct SqliteSubRepository {
    store: Store,
}

impl SqliteSubRepository {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }
}

impl SubRepository for SqliteSubRepository {
    fn list(&self) -> Result<Vec<SubItem>, DomainError> {
        let rows = self
            .store
            .read_rows("SubItem")
            .map_err(persistence_storage_error)?;
        let mut items: Vec<SubItem> = rows.iter().map(SubItem::from_row).collect();
        items.sort_by_key(|s| s.sort);
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<SubItem>, DomainError> {
        let rows = self
            .store
            .query_rows("SELECT * FROM \"SubItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(SubItem::from_row))
    }

    fn upsert(&mut self, item: SubItem) -> Result<(), DomainError> {
        let row = item.to_row();
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute("DELETE FROM \"SubItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn count(&self) -> usize {
        self.store.count_rows("SubItem").unwrap_or(0) as usize
    }
}

/// A SQLite-backed routing repository sharing the profile store's `Store`.
pub struct SqliteRoutingRepository {
    store: Store,
}

impl SqliteRoutingRepository {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }
}

impl crate::routing::RoutingRepository for SqliteRoutingRepository {
    fn list(&self) -> Result<Vec<domain::RoutingProfile>, DomainError> {
        let rows = self
            .store
            .read_rows("RoutingItem")
            .map_err(persistence_storage_error)?;
        let mut items: Vec<domain::RoutingProfile> =
            rows.iter().map(crate::routing::routing_from_row).collect();
        items.sort_by_key(|r| r.sort);
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<domain::RoutingProfile>, DomainError> {
        let rows = self
            .store
            .query_rows("SELECT * FROM \"RoutingItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(crate::routing::routing_from_row))
    }

    fn upsert(&mut self, item: domain::RoutingProfile) -> Result<(), DomainError> {
        let row = crate::routing::routing_to_row(&item);
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute("DELETE FROM \"RoutingItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn count(&self) -> usize {
        self.store.count_rows("RoutingItem").unwrap_or(0) as usize
    }
}

/// A SQLite-backed DNS repository sharing the profile store's `Store`.
pub struct SqliteDnsRepository {
    store: Store,
}

impl SqliteDnsRepository {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }
}

impl crate::dns::DnsRepository for SqliteDnsRepository {
    fn list(&self) -> Result<Vec<domain::DnsProfile>, DomainError> {
        let rows = self
            .store
            .read_rows("DNSItem")
            .map_err(persistence_storage_error)?;
        let mut items: Vec<domain::DnsProfile> =
            rows.iter().map(crate::dns::dns_from_row).collect();
        items.sort_by(|a, b| {
            a.core_type
                .value()
                .cmp(&b.core_type.value())
                .then_with(|| a.remarks.cmp(&b.remarks))
        });
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<domain::DnsProfile>, DomainError> {
        let rows = self
            .store
            .query_rows("SELECT * FROM \"DNSItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(crate::dns::dns_from_row))
    }

    fn upsert(&mut self, item: domain::DnsProfile) -> Result<(), DomainError> {
        let row = crate::dns::dns_to_row(&item);
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute("DELETE FROM \"DNSItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn count(&self) -> usize {
        self.store.count_rows("DNSItem").unwrap_or(0) as usize
    }
}

fn build_where(filter: &ProfileFilter) -> (String, Vec<String>) {
    let mut parts: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();
    if let Some(text) = &filter.text {
        let needle = format!("%{text}%");
        parts.push("(\"Remarks\" LIKE ? OR \"Address\" LIKE ?)".to_string());
        params.push(needle.clone());
        params.push(needle);
    }
    if !filter.config_types.is_empty() {
        let list = filter
            .config_types
            .iter()
            .map(|c| c.value().to_string())
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("\"ConfigType\" IN ({list})"));
    }
    if let Some(subid) = &filter.subid {
        parts.push("\"Subid\" = ?".to_string());
        params.push(subid.clone());
    }
    let where_sql = if parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", parts.join(" AND "))
    };
    (where_sql, params)
}

/// Build a `ProfileItem` row from a domain profile.
pub fn row_from_profile(profile: &Profile) -> RawRow {
    let mut row = RawRow::new("ProfileItem");
    row.set("IndexId", json!(profile.index_id));
    row.set("ConfigType", json!(profile.config_type.value()));
    row.set(
        "CoreType",
        profile.core_type.map_or(Value::Null, |c| json!(c.value())),
    );
    row.set("ConfigVersion", json!(profile.config_version));
    row.set("Subid", json!(profile.subid));
    row.set("IsSub", json!(i64::from(profile.is_sub)));
    row.set(
        "PreSocksPort",
        profile.pre_socks_port.map_or(Value::Null, |v| json!(v)),
    );
    row.set("DisplayLog", json!(i64::from(profile.display_log)));
    row.set("Remarks", json!(profile.remarks));
    row.set("Address", json!(profile.address));
    row.set("Port", json!(profile.port));
    row.set("Password", json!(profile.password));
    row.set("Username", json!(profile.username));
    row.set("Network", json!(profile.network));
    row.set(
        "StreamSecurity",
        opt_json(profile.security.stream_security.as_deref()),
    );
    row.set(
        "AllowInsecure",
        opt_json(profile.security.allow_insecure.as_deref()),
    );
    row.set("Sni", opt_json(profile.security.sni.as_deref()));
    row.set("Alpn", opt_json(profile.security.alpn.as_deref()));
    row.set(
        "Fingerprint",
        opt_json(profile.security.fingerprint.as_deref()),
    );
    row.set(
        "PublicKey",
        opt_json(profile.security.public_key.as_deref()),
    );
    row.set("ShortId", opt_json(profile.security.short_id.as_deref()));
    row.set("SpiderX", opt_json(profile.security.spider_x.as_deref()));
    row.set(
        "Mldsa65Verify",
        opt_json(profile.security.mldsa65_verify.as_deref()),
    );
    row.set("Cert", opt_json(profile.security.cert.as_deref()));
    row.set("CertSha", opt_json(profile.security.cert_sha.as_deref()));
    row.set(
        "EchConfigList",
        opt_json(profile.security.ech_config_list.as_deref()),
    );
    row.set(
        "VerifyPeerCertByName",
        opt_json(profile.security.verify_peer_cert_by_name.as_deref()),
    );
    row.set("Finalmask", opt_json(profile.finalmask.as_deref()));
    row.set(
        "MuxEnabled",
        profile
            .mux_enabled
            .map_or(Value::Null, |v| json!(i64::from(v))),
    );
    row.set(
        "ProtoExtra",
        json!(ProtocolExtraBlob::from_domain(&profile.proto_extra)
            .to_json()
            .unwrap_or_else(|_| "{}".to_string())),
    );
    row.set(
        "TransportExtra",
        json!(TransportExtraBlob::from_domain(&profile.transport_extra)
            .to_json()
            .unwrap_or_else(|_| "{}".to_string())),
    );

    // Preserve any legacy columns carried in `extra`; drop unknown top-level
    // keys (upstream has no column for them).
    for column in LEGACY_COLUMNS {
        if let Some(value) = profile.extra.get(*column) {
            row.set(column, value.clone());
        } else {
            row.set(column, Value::Null);
        }
    }
    row
}

/// Build a domain profile from a `ProfileItem` row.
pub fn profile_from_row(row: &RawRow) -> Profile {
    let config_type = row
        .opt_i64("ConfigType")
        .and_then(|v| ConfigType::from_value(v as i32))
        .unwrap_or(ConfigType::Vmess);
    let core_type = row
        .opt_i64("CoreType")
        .and_then(|v| CoreType::from_value(v as i32));

    let mut extra = domain::ExtraMap::new();
    for &column in LEGACY_COLUMNS {
        if let Some(value) = row.get(column) {
            if !value.is_null() {
                let empty_string = value.as_str().is_some_and(str::is_empty);
                if !empty_string {
                    extra.insert((*column).to_string(), value.clone());
                }
            }
        }
    }

    let proto_extra = ProtocolExtraBlob::parse(row.opt_string("ProtoExtra").as_deref())
        .map(|b| b.to_domain())
        .unwrap_or_default();
    let transport_extra = TransportExtraBlob::parse(row.opt_string("TransportExtra").as_deref())
        .map(|b| b.to_domain())
        .unwrap_or_default();

    Profile {
        index_id: row.string("IndexId"),
        config_type,
        core_type,
        config_version: row.opt_i64("ConfigVersion").unwrap_or(4) as i32,
        subid: row.string("Subid"),
        is_sub: row.bool("IsSub"),
        pre_socks_port: row.opt_i64("PreSocksPort").map(|v| v as i32),
        display_log: row.bool("DisplayLog"),
        remarks: row.string("Remarks"),
        address: row.string("Address"),
        port: row.opt_i64("Port").unwrap_or(0) as i32,
        password: row.string("Password"),
        username: row.string("Username"),
        network: row.string("Network"),
        mux_enabled: row.opt_i64("MuxEnabled").map(|v| v != 0),
        finalmask: row.opt_string("Finalmask"),
        outbound_tag: None,
        security: SecurityParams {
            stream_security: row.opt_string("StreamSecurity"),
            allow_insecure: row.opt_string("AllowInsecure"),
            sni: row.opt_string("Sni"),
            alpn: row.opt_string("Alpn"),
            fingerprint: row.opt_string("Fingerprint"),
            public_key: row.opt_string("PublicKey"),
            short_id: row.opt_string("ShortId"),
            spider_x: row.opt_string("SpiderX"),
            mldsa65_verify: row.opt_string("Mldsa65Verify"),
            cert: row.opt_string("Cert"),
            cert_sha: row.opt_string("CertSha"),
            ech_config_list: row.opt_string("EchConfigList"),
            verify_peer_cert_by_name: row.opt_string("VerifyPeerCertByName"),
        },
        proto_extra,
        transport_extra,
        extra,
    }
}

fn opt_json(value: Option<&str>) -> Value {
    match value {
        Some(v) if !v.is_empty() => json!(v),
        _ => Value::Null,
    }
}

/// Storage backend selected at engine construction.
pub enum ProfileStore {
    Memory(InMemoryProfileRepository),
    Sqlite(SqliteProfileRepository),
}

impl ProfileStore {
    /// Transactional replace for one subscription (see
    /// [`SqliteProfileRepository::replace_for_sub`]). The in-memory backend
    /// snapshots the affected rows and restores them on failure so the
    /// "old data survives a mid-replace error" contract holds for tests.
    pub fn replace_for_sub(
        &mut self,
        subid: &str,
        profiles: Vec<Profile>,
        remove_existing: bool,
        fail_after_delete: bool,
    ) -> Result<(usize, usize), DomainError> {
        match self {
            ProfileStore::Memory(repo) => {
                let snapshot = repo.snapshot_for_sub(subid);
                let added_count = profiles.len();
                let mut removed = 0usize;
                let result: Result<(usize, usize), DomainError> = (|| {
                    if remove_existing {
                        // Only subscription-sourced rows are replaced; manual
                        // `IsSub = false` nodes in the same group are kept
                        // (upstream `RemoveServersViaSubid(..., isSub: true)`).
                        for id in snapshot
                            .iter()
                            .filter(|p| p.is_sub)
                            .map(|p| p.index_id.clone())
                            .collect::<Vec<_>>()
                        {
                            if repo.remove(&id)? {
                                removed += 1;
                            }
                        }
                    }
                    if fail_after_delete {
                        return Err(DomainError::new(codes::INTERNAL, "error.storage")
                            .with_detail("injected failure after delete"));
                    }
                    for profile in &profiles {
                        repo.upsert(profile.clone())?;
                    }
                    Ok((added_count, removed))
                })();
                if result.is_err() {
                    repo.restore_snapshot(subid, snapshot);
                }
                result
            }
            ProfileStore::Sqlite(repo) => {
                repo.replace_for_sub(subid, profiles, remove_existing, fail_after_delete)
            }
        }
    }
}

impl ProfileRepository for ProfileStore {
    fn get(&self, index_id: &str) -> Result<Option<Profile>, DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.get(index_id),
            ProfileStore::Sqlite(repo) => repo.get(index_id),
        }
    }

    fn upsert(&mut self, profile: Profile) -> Result<(), DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.upsert(profile),
            ProfileStore::Sqlite(repo) => repo.upsert(profile),
        }
    }

    fn remove(&mut self, index_id: &str) -> Result<bool, DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.remove(index_id),
            ProfileStore::Sqlite(repo) => repo.remove(index_id),
        }
    }

    fn query(
        &self,
        filter: &ProfileFilter,
        sort: ProfileSort,
        page: PageRequest,
    ) -> Result<ProfilePage, DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.query(filter, sort, page),
            ProfileStore::Sqlite(repo) => repo.query(filter, sort, page),
        }
    }

    fn count(&self) -> usize {
        match self {
            ProfileStore::Memory(repo) => repo.count(),
            ProfileStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// Storage backend for subscriptions, selected at engine construction.
pub enum SubStore {
    Memory(crate::repository::InMemorySubRepository),
    Sqlite(SqliteSubRepository),
}

impl SubRepository for SubStore {
    fn list(&self) -> Result<Vec<SubItem>, DomainError> {
        match self {
            SubStore::Memory(repo) => repo.list(),
            SubStore::Sqlite(repo) => repo.list(),
        }
    }

    fn get(&self, id: &str) -> Result<Option<SubItem>, DomainError> {
        match self {
            SubStore::Memory(repo) => repo.get(id),
            SubStore::Sqlite(repo) => repo.get(id),
        }
    }

    fn upsert(&mut self, item: SubItem) -> Result<(), DomainError> {
        match self {
            SubStore::Memory(repo) => repo.upsert(item),
            SubStore::Sqlite(repo) => repo.upsert(item),
        }
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        match self {
            SubStore::Memory(repo) => repo.remove(id),
            SubStore::Sqlite(repo) => repo.remove(id),
        }
    }

    fn count(&self) -> usize {
        match self {
            SubStore::Memory(repo) => repo.count(),
            SubStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// Storage backend for routing profiles, selected at engine construction.
pub enum RoutingStore {
    Memory(crate::routing::InMemoryRoutingRepository),
    Sqlite(SqliteRoutingRepository),
}

impl crate::routing::RoutingRepository for RoutingStore {
    fn list(&self) -> Result<Vec<domain::RoutingProfile>, DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.list(),
            RoutingStore::Sqlite(repo) => repo.list(),
        }
    }

    fn get(&self, id: &str) -> Result<Option<domain::RoutingProfile>, DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.get(id),
            RoutingStore::Sqlite(repo) => repo.get(id),
        }
    }

    fn upsert(&mut self, item: domain::RoutingProfile) -> Result<(), DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.upsert(item),
            RoutingStore::Sqlite(repo) => repo.upsert(item),
        }
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.remove(id),
            RoutingStore::Sqlite(repo) => repo.remove(id),
        }
    }

    fn count(&self) -> usize {
        match self {
            RoutingStore::Memory(repo) => repo.count(),
            RoutingStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// Storage backend for DNS profiles, selected at engine construction.
pub enum DnsStore {
    Memory(crate::dns::InMemoryDnsRepository),
    Sqlite(SqliteDnsRepository),
}

impl crate::dns::DnsRepository for DnsStore {
    fn list(&self) -> Result<Vec<domain::DnsProfile>, DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.list(),
            DnsStore::Sqlite(repo) => repo.list(),
        }
    }

    fn get(&self, id: &str) -> Result<Option<domain::DnsProfile>, DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.get(id),
            DnsStore::Sqlite(repo) => repo.get(id),
        }
    }

    fn upsert(&mut self, item: domain::DnsProfile) -> Result<(), DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.upsert(item),
            DnsStore::Sqlite(repo) => repo.upsert(item),
        }
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.remove(id),
            DnsStore::Sqlite(repo) => repo.remove(id),
        }
    }

    fn count(&self) -> usize {
        match self {
            DnsStore::Memory(repo) => repo.count(),
            DnsStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// SQLite-backed `ServerStatItem` store for the T15a statistics pipeline.
pub struct SqliteTrafficStore {
    store: Store,
}

impl SqliteTrafficStore {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }
}

impl crate::monitor::TrafficStore for SqliteTrafficStore {
    fn load(&self) -> Result<Vec<TrafficStats>, DomainError> {
        let rows = self
            .store
            .read_rows("ServerStatItem")
            .map_err(persistence_storage_error)?;
        Ok(rows.iter().map(map_traffic).collect())
    }

    fn upsert(&mut self, stat: &TrafficStats) -> Result<(), DomainError> {
        let mut row = RawRow::new("ServerStatItem");
        row.set("IndexId", json!(&stat.index_id));
        row.set("TotalUp", json!(stat.total_up));
        row.set("TotalDown", json!(stat.total_down));
        row.set("TodayUp", json!(stat.today_up));
        row.set("TodayDown", json!(stat.today_down));
        row.set("DateNow", json!(stat.date_now));
        let conn = self.store.connection();
        self.store
            .upsert_row(conn, &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, index_id: &str) -> Result<(), DomainError> {
        self.store
            .execute(
                "DELETE FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                &[&index_id],
            )
            .map_err(persistence_storage_error)?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DomainError> {
        self.store
            .execute("DELETE FROM \"ServerStatItem\"", &[])
            .map_err(persistence_storage_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::synthetic_full_profile;

    #[test]
    fn sqlite_replace_for_sub_rolls_back_on_injected_failure() {
        use crate::synthetic::synthetic_full_profile;
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let mut seed = vec![synthetic_full_profile(1), synthetic_full_profile(2)];
        for p in &mut seed {
            p.subid = "s-sql".to_string();
        }
        let (added, _) = repo.replace_for_sub("s-sql", seed, true, false).unwrap();
        assert_eq!(added, 2);
        let mut replacement = vec![synthetic_full_profile(3)];
        for p in &mut replacement {
            p.subid = "s-sql".to_string();
        }
        let err = repo
            .replace_for_sub("s-sql", replacement, true, true)
            .unwrap_err();
        assert_eq!(err.code, domain::codes::INTERNAL);
        let kept: Vec<_> = repo
            .query(
                &crate::repository::ProfileFilter {
                    subid: Some("s-sql".to_string()),
                    ..Default::default()
                },
                crate::repository::ProfileSort::IndexId,
                crate::repository::PageRequest {
                    cursor: 0,
                    page_size: u32::MAX,
                },
            )
            .unwrap()
            .items;
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn round_trips_all_extras_through_sqlite() {
        let mut repo = SqliteProfileRepository::open(":memory:").unwrap();
        let mut profile = synthetic_full_profile(1);
        profile.proto_extra.ss_method = Some("aes-256-gcm".into());
        profile.proto_extra.flow = Some("xtls-rprx-vision".into());
        profile.transport_extra.path = Some("/ws".into());
        profile.transport_extra.host = Some("example.com".into());
        profile.security.sni = Some("sni.example.com".into());
        profile.extra.insert("HeaderType".into(), json!("none"));
        repo.upsert(profile.clone()).unwrap();

        let loaded = repo.get(&profile.index_id).unwrap().unwrap();
        assert_eq!(loaded.proto_extra.ss_method.as_deref(), Some("aes-256-gcm"));
        assert_eq!(loaded.proto_extra.flow.as_deref(), Some("xtls-rprx-vision"));
        assert_eq!(loaded.transport_extra.path.as_deref(), Some("/ws"));
        assert_eq!(loaded.transport_extra.host.as_deref(), Some("example.com"));
        assert_eq!(loaded.security.sni.as_deref(), Some("sni.example.com"));
        assert_eq!(loaded.extra.get("HeaderType"), Some(&json!("none")));
    }

    #[test]
    fn subscription_replace_keeps_manual_nodes_and_clones_stats() {
        // R4-17 / D04+D05: a subscription replace only removes `IsSub = 1`
        // rows of the group, keeps a manual `IsSub = 0` node, and clones the
        // matched `ServerStatItem` row onto the new identity.
        let mut repo = SqliteProfileRepository::open(":memory:").unwrap();
        let mut old_sub = synthetic_full_profile(10);
        old_sub.subid = "s-A".into();
        old_sub.is_sub = true;
        let old_id = old_sub.index_id.clone();
        let mut manual = synthetic_full_profile(11);
        manual.subid = "s-A".into();
        manual.is_sub = false;
        manual.address = "198.51.100.99".into();
        let manual_id = manual.index_id.clone();
        repo.upsert(old_sub.clone()).unwrap();
        repo.upsert(manual.clone()).unwrap();

        let mut stat = RawRow::new("ServerStatItem");
        stat.set("IndexId", json!(old_id.clone()));
        stat.set("TotalUp", json!(111));
        stat.set("TotalDown", json!(222));
        stat.set("TodayUp", json!(1));
        stat.set("TodayDown", json!(2));
        stat.set("DateNow", json!(0));
        repo.store()
            .upsert_row(repo.store().connection(), &stat)
            .unwrap();

        let mut new_sub = old_sub.clone();
        new_sub.index_id = "new-sub-id".into();
        let (added, removed) = repo
            .replace_for_sub("s-A", vec![new_sub.clone()], true, false)
            .unwrap();
        assert_eq!(added, 1, "only the replacement set counts as added");
        assert_eq!(removed, 1, "only the IsSub=1 row is replaced");

        assert!(repo.get(&manual_id).unwrap().is_some(), "manual node kept");
        assert!(repo.get(&old_id).unwrap().is_none(), "old sub node removed");
        assert!(
            repo.get("new-sub-id").unwrap().is_some(),
            "new node present"
        );

        let cloned = repo
            .store()
            .query_rows(
                "SELECT * FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                &[&"new-sub-id"],
            )
            .unwrap();
        assert_eq!(cloned.len(), 1, "stat cloned to the new identity");
        assert_eq!(cloned[0].i64("TotalUp"), 111);
        assert_eq!(cloned[0].i64("TotalDown"), 222);
        // Upstream `CloneServerStatItem` copies; the old row stays.
        let old_stat = repo
            .store()
            .query_rows(
                "SELECT * FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                &[&old_id],
            )
            .unwrap();
        assert_eq!(old_stat.len(), 1);
    }
}
