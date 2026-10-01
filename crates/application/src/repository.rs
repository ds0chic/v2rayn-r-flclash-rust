//! Profile repository abstraction and its in-memory implementation.
//!
//! T02 provides the trait + an in-memory store so higher layers and the FRB
//! API can be exercised without a database. T04 supplies the SQLite-backed
//! implementation behind the same trait. The trait is deliberately
//! synchronous and cheap; the async boundary lives at the FRB layer.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use domain::{DesiredRevision, DomainError, Profile};

static INDEX_SEQ: AtomicU64 = AtomicU64::new(0);

/// Generate a fresh stable profile id (time + process-local counter). Never a
/// row number, and unique across restarts for practical purposes.
pub fn new_index_id() -> String {
    let seq = INDEX_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("p-{nanos:x}-{seq:x}")
}

/// Query filter for [`ProfileRepository::query`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileFilter {
    /// Case-insensitive substring match on remarks/address.
    pub text: Option<String>,
    pub config_types: Vec<domain::ConfigType>,
    pub subid: Option<String>,
}

/// Sort key for [`ProfileRepository::query`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileSort {
    #[default]
    Remarks,
    Address,
    Delay,
    /// Stable insertion order.
    IndexId,
}

/// Paging request with a stable cursor (index into the ordered result set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub cursor: usize,
    pub page_size: u32,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            cursor: 0,
            page_size: 100,
        }
    }
}

/// A page of profiles plus the next cursor and total count.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfilePage {
    pub items: Vec<Profile>,
    pub total: usize,
    /// `None` when the page is the last one.
    pub next_cursor: Option<usize>,
}

/// Storage contract for profiles.
pub trait ProfileRepository: Send {
    fn get(&self, index_id: &str) -> Result<Option<Profile>, DomainError>;

    /// Insert or replace by `index_id`.
    fn upsert(&mut self, profile: Profile) -> Result<(), DomainError>;

    fn remove(&mut self, index_id: &str) -> Result<bool, DomainError>;

    /// Query with filter/sort/cursor paging.
    fn query(
        &self,
        filter: &ProfileFilter,
        sort: ProfileSort,
        page: PageRequest,
    ) -> Result<ProfilePage, DomainError>;

    fn count(&self) -> usize;
}

/// In-memory repository. Insertion order is preserved for the `IndexId` sort.
#[derive(Debug, Default)]
pub struct InMemoryProfileRepository {
    by_id: HashMap<String, Profile>,
    order: Vec<String>,
}

impl InMemoryProfileRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_profiles(profiles: Vec<Profile>) -> Self {
        let mut repo = Self::new();
        for p in profiles {
            // Ignore duplicate errors in seeding; last write wins.
            let _ = repo.upsert(p);
        }
        repo
    }

    fn matches(profile: &Profile, filter: &ProfileFilter) -> bool {
        if let Some(subid) = &filter.subid {
            if &profile.subid != subid {
                return false;
            }
        }
        if !filter.config_types.is_empty() && !filter.config_types.contains(&profile.config_type) {
            return false;
        }
        if let Some(text) = &filter.text {
            let needle = text.to_ascii_lowercase();
            let hay = format!("{}\u{0}{}", profile.remarks, profile.address).to_ascii_lowercase();
            if !hay.contains(&needle) {
                return false;
            }
        }
        true
    }
}

impl ProfileRepository for InMemoryProfileRepository {
    fn get(&self, index_id: &str) -> Result<Option<Profile>, DomainError> {
        Ok(self.by_id.get(index_id).cloned())
    }

    fn upsert(&mut self, profile: Profile) -> Result<(), DomainError> {
        if profile.index_id.trim().is_empty() {
            return Err(
                DomainError::new(domain::codes::FIELD_REQUIRED, "error.index_id_required")
                    .with_field("index_id"),
            );
        }
        if !self.by_id.contains_key(&profile.index_id) {
            self.order.push(profile.index_id.clone());
        }
        self.by_id.insert(profile.index_id.clone(), profile);
        Ok(())
    }

    fn remove(&mut self, index_id: &str) -> Result<bool, DomainError> {
        let existed = self.by_id.remove(index_id).is_some();
        if existed {
            self.order.retain(|id| id != index_id);
        }
        Ok(existed)
    }

    fn query(
        &self,
        filter: &ProfileFilter,
        sort: ProfileSort,
        page: PageRequest,
    ) -> Result<ProfilePage, DomainError> {
        let mut items: Vec<Profile> = self
            .order
            .iter()
            .filter_map(|id| self.by_id.get(id))
            .filter(|p| Self::matches(p, filter))
            .cloned()
            .collect();

        match sort {
            ProfileSort::Remarks => items.sort_by(|a, b| {
                a.remarks
                    .to_ascii_lowercase()
                    .cmp(&b.remarks.to_ascii_lowercase())
                    .then_with(|| a.index_id.cmp(&b.index_id))
            }),
            ProfileSort::Address => items.sort_by(|a, b| {
                a.address
                    .cmp(&b.address)
                    .then_with(|| a.index_id.cmp(&b.index_id))
            }),
            ProfileSort::Delay => items.sort_by(|a, b| a.index_id.cmp(&b.index_id)),
            ProfileSort::IndexId => {}
        }

        let total = items.len();
        let start = page.cursor.min(total);
        let size = page.page_size.max(1) as usize;
        let end = (start + size).min(total);
        let page_items = items[start..end].to_vec();
        let next_cursor = if end < total { Some(end) } else { None };

        Ok(ProfilePage {
            items: page_items,
            total,
            next_cursor,
        })
    }

    fn count(&self) -> usize {
        self.by_id.len()
    }
}

/// The desired-revision counter, owned by the repository's writer.
///
/// It is kept separate from the repository so a caller can compute the next
/// revision before committing a mutation atomically at the persistence layer.
#[derive(Debug)]
pub struct RevisionStore {
    desired: DesiredRevision,
}

impl Default for RevisionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RevisionStore {
    pub fn new() -> Self {
        Self::with_desired(DesiredRevision::ZERO)
    }

    /// Start from a persisted revision (restart continuity).
    pub fn with_desired(desired: DesiredRevision) -> Self {
        Self { desired }
    }

    pub fn desired(&self) -> DesiredRevision {
        self.desired
    }

    /// Advance the desired revision and return the new value.
    pub fn bump(&mut self) -> DesiredRevision {
        self.desired = self.desired.next();
        self.desired
    }

    /// Check an expected revision against the current one.
    pub fn check(&self, expected: DesiredRevision) -> Result<(), DomainError> {
        if expected == self.desired {
            Ok(())
        } else {
            Err(DomainError::stale_revision(
                expected.get(),
                self.desired.get(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::synthetic_full_profile;

    #[test]
    fn upsert_and_get_roundtrip() {
        let mut repo = InMemoryProfileRepository::new();
        let p = synthetic_full_profile(1);
        let id = p.index_id.clone();
        repo.upsert(p.clone()).unwrap();
        assert_eq!(repo.get(&id).unwrap().unwrap(), p);
        assert_eq!(repo.count(), 1);
    }

    #[test]
    fn upsert_requires_index_id() {
        let mut repo = InMemoryProfileRepository::new();
        let mut p = synthetic_full_profile(1);
        p.index_id.clear();
        assert_eq!(
            repo.upsert(p).unwrap_err().code,
            domain::codes::FIELD_REQUIRED
        );
    }

    #[test]
    fn query_pages_with_cursor() {
        let repo =
            InMemoryProfileRepository::with_profiles((0..25).map(synthetic_full_profile).collect());
        let first = repo
            .query(
                &ProfileFilter::default(),
                ProfileSort::Remarks,
                PageRequest {
                    cursor: 0,
                    page_size: 10,
                },
            )
            .unwrap();
        assert_eq!(first.items.len(), 10);
        assert_eq!(first.total, 25);
        assert_eq!(first.next_cursor, Some(10));

        let last = repo
            .query(
                &ProfileFilter::default(),
                ProfileSort::Remarks,
                PageRequest {
                    cursor: 20,
                    page_size: 10,
                },
            )
            .unwrap();
        assert_eq!(last.items.len(), 5);
        assert_eq!(last.next_cursor, None);
    }

    #[test]
    fn filter_by_text_and_type() {
        let repo =
            InMemoryProfileRepository::with_profiles((0..10).map(synthetic_full_profile).collect());
        let filter = ProfileFilter {
            text: Some("synthetic-00003".into()),
            ..Default::default()
        };
        let page = repo
            .query(&filter, ProfileSort::IndexId, PageRequest::default())
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].remarks, "Synthetic-00003");
    }

    #[test]
    fn revision_store_bumps_and_checks() {
        let mut store = RevisionStore::new();
        assert_eq!(store.desired().get(), 0);
        assert!(store.check(DesiredRevision::ZERO).is_ok());
        store.bump();
        assert!(store.check(DesiredRevision::ZERO).is_err());
        assert!(store.check(DesiredRevision(1)).is_ok());
    }
}
