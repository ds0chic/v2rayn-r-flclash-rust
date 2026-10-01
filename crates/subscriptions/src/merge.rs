//! Subscription merge semantics: regex `Filter`, `KeepOlderDedupl` dedup and
//! the "never clear the old set on failure/empty" rule.

use domain::{ConfigType, Profile};

use crate::error::SubError;
use crate::util::is_regex_match;

/// Options controlling a subscription refresh.
#[derive(Debug, Clone)]
pub struct MergeOptions {
    /// `GuiItem.KeepOlderDedupl`: keep the older node when duplicates collapse.
    pub keep_older: bool,
    /// `SubItem.Filter`: a regex tested against `remarks`.
    pub filter: Option<String>,
    /// Upper bound on the filter pattern length; longer patterns fail open.
    pub max_pattern_len: usize,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            keep_older: true,
            filter: None,
            max_pattern_len: 4096,
        }
    }
}

/// Result of a successful refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeResult {
    pub profiles: Vec<Profile>,
    pub filtered_out: usize,
    pub deduplicated: usize,
    pub existing_count: usize,
}

/// Outcome of [`refresh`]. An empty or failed incoming batch never clears the
/// existing set.
#[derive(Debug, Clone, PartialEq)]
pub enum RefreshOutcome {
    Replaced(Box<MergeResult>),
    /// Incoming was empty: the old set is retained untouched.
    PreservedOnEmpty {
        existing_count: usize,
    },
    /// Parsing failed: the old set is retained and the error is surfaced.
    PreservedOnError(Box<SubError>, usize),
}

/// `ConfigHandler.CompareProfileItem(..., remarks=false)`.
pub fn compare_profile(o: &Profile, n: &Profile, remarks: bool) -> bool {
    let o_proto = &o.proto_extra;
    let n_proto = &n.proto_extra;
    let o_transport = &o.transport_extra;
    let n_transport = &n.transport_extra;

    o.config_type == n.config_type
        && eq_opt(Some(o.address.as_str()), Some(n.address.as_str()))
        && o.port == n.port
        && eq_opt(Some(o.password.as_str()), Some(n.password.as_str()))
        && eq_opt(Some(o.username.as_str()), Some(n.username.as_str()))
        && eq_opt(
            o_proto.vless_encryption.as_deref(),
            n_proto.vless_encryption.as_deref(),
        )
        && eq_opt(o_proto.ss_method.as_deref(), n_proto.ss_method.as_deref())
        && eq_opt(
            o_proto.vmess_security.as_deref(),
            n_proto.vmess_security.as_deref(),
        )
        && eq_opt(Some(o.network.as_str()), Some(n.network.as_str()))
        && eq_opt(
            o_transport.raw_header_type.as_deref(),
            n_transport.raw_header_type.as_deref(),
        )
        && eq_opt(o_transport.host.as_deref(), n_transport.host.as_deref())
        && eq_opt(o_transport.path.as_deref(), n_transport.path.as_deref())
        && eq_opt(
            o_transport.xhttp_mode.as_deref(),
            n_transport.xhttp_mode.as_deref(),
        )
        && eq_opt(
            o_transport.xhttp_extra.as_deref(),
            n_transport.xhttp_extra.as_deref(),
        )
        && eq_opt(
            o_transport.grpc_authority.as_deref(),
            n_transport.grpc_authority.as_deref(),
        )
        && eq_opt(
            o_transport.grpc_service_name.as_deref(),
            n_transport.grpc_service_name.as_deref(),
        )
        && eq_opt(
            o_transport.grpc_mode.as_deref(),
            n_transport.grpc_mode.as_deref(),
        )
        && eq_opt(
            o_transport.kcp_header_type.as_deref(),
            n_transport.kcp_header_type.as_deref(),
        )
        && eq_opt(
            o_transport.kcp_seed.as_deref(),
            n_transport.kcp_seed.as_deref(),
        )
        && (o.config_type == ConfigType::Trojan
            || eq_opt(
                o.security.stream_security.as_deref(),
                n.security.stream_security.as_deref(),
            ))
        && eq_opt(o_proto.flow.as_deref(), n_proto.flow.as_deref())
        && eq_opt(
            o_proto.salamander_pass.as_deref(),
            n_proto.salamander_pass.as_deref(),
        )
        && eq_opt(o.security.sni.as_deref(), n.security.sni.as_deref())
        && eq_opt(o.security.alpn.as_deref(), n.security.alpn.as_deref())
        && eq_opt(
            o.security.fingerprint.as_deref(),
            n.security.fingerprint.as_deref(),
        )
        && eq_opt(
            o.security.public_key.as_deref(),
            n.security.public_key.as_deref(),
        )
        && eq_opt(
            o.security.short_id.as_deref(),
            n.security.short_id.as_deref(),
        )
        && eq_opt(o.finalmask.as_deref(), n.finalmask.as_deref())
        && (!remarks || o.remarks == n.remarks)
}

/// Empty strings and absent values compare equal.
fn eq_opt(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        (Some(a), None) => a.is_empty(),
        (None, Some(b)) => b.is_empty(),
        (None, None) => true,
    }
}

/// Return `(matched, dropped)` after applying the subscription filter regex.
pub fn filter_by_regex(
    items: &[Profile],
    pattern: &str,
    max_pattern_len: usize,
) -> (Vec<Profile>, usize) {
    if pattern.is_empty() {
        return (items.to_vec(), 0);
    }
    let mut kept = Vec::new();
    let mut dropped = 0;
    for item in items {
        if is_regex_match(&item.remarks, pattern, max_pattern_len) {
            kept.push(item.clone());
        } else {
            dropped += 1;
        }
    }
    (kept, dropped)
}

/// `ConfigHandler.DedupServerList`: deduplicate by transport identity, keeping
/// the older node first when `keep_older` is set (complex nodes always stay).
pub fn deduplicate(items: &[Profile], keep_older: bool) -> (Vec<Profile>, usize) {
    let mut ordered: Vec<Profile> = items.to_vec();
    if !keep_older {
        ordered.reverse();
    }
    let mut kept: Vec<Profile> = Vec::new();
    let mut removed = 0;
    for item in ordered {
        if item.config_type.is_complex() {
            kept.push(item);
            continue;
        }
        if kept
            .iter()
            .any(|existing| compare_profile(existing, &item, false))
        {
            removed += 1;
        } else {
            kept.push(item);
        }
    }
    (kept, removed)
}

/// Compose filter + dedup into a refresh. Empty incoming preserves the old set.
pub fn refresh(existing: &[Profile], incoming: &[Profile], opts: &MergeOptions) -> RefreshOutcome {
    if incoming.is_empty() {
        return RefreshOutcome::PreservedOnEmpty {
            existing_count: existing.len(),
        };
    }
    let (filtered, filtered_out) = match opts.filter.as_deref() {
        Some(pattern) if !pattern.is_empty() => {
            filter_by_regex(incoming, pattern, opts.max_pattern_len)
        }
        _ => (incoming.to_vec(), 0),
    };
    let (deduped, deduplicated) = deduplicate(&filtered, opts.keep_older);
    RefreshOutcome::Replaced(Box::new(MergeResult {
        profiles: deduped,
        filtered_out,
        deduplicated,
        existing_count: existing.len(),
    }))
}

/// Like [`refresh`] but sourced from a fallible parse, preserving the old set
/// and surfacing the error when parsing failed.
pub fn refresh_result(
    existing: &[Profile],
    parsed: Result<Vec<Profile>, SubError>,
    opts: &MergeOptions,
) -> RefreshOutcome {
    match parsed {
        Ok(items) => refresh(existing, &items, opts),
        Err(err) => RefreshOutcome::PreservedOnError(Box::new(err), existing.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(remarks: &str, address: &str, port: i32) -> Profile {
        Profile {
            config_type: ConfigType::Vless,
            remarks: remarks.into(),
            address: address.into(),
            port,
            password: "id".into(),
            ..Profile::default()
        }
    }

    #[test]
    fn identical_nodes_collapse_keeping_older() {
        let a = node("first", "example.com", 443);
        let b = node("second", "example.com", 443);
        let (kept_older, removed) = deduplicate(&[a.clone(), b.clone()], true);
        assert_eq!(removed, 1);
        assert_eq!(kept_older[0].remarks, "first");

        let (kept_newer, _) = deduplicate(&[a, b], false);
        assert_eq!(kept_newer[0].remarks, "second");
    }

    #[test]
    fn complex_nodes_are_never_deduplicated() {
        let mut group_a = Profile {
            config_type: ConfigType::PolicyGroup,
            remarks: "g".into(),
            ..Profile::default()
        };
        let mut group_b = group_a.clone();
        group_a.index_id = "a".into();
        group_b.index_id = "b".into();
        let (kept, removed) = deduplicate(&[group_a, group_b], true);
        assert_eq!(removed, 0);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn filter_drops_non_matching_remarks() {
        let items = vec![node("Tokyo node", "a", 1), node("Frankfurt node", "b", 2)];
        let (kept, dropped) = filter_by_regex(&items, "Tokyo", 1024);
        assert_eq!(kept.len(), 1);
        assert_eq!(dropped, 1);
        // Invalid pattern fails open (nothing dropped).
        let (kept, dropped) = filter_by_regex(&items, "(?P<", 1024);
        assert_eq!(kept.len(), 2);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn empty_incoming_preserves_existing() {
        let existing = vec![node("keep", "a", 1)];
        match refresh(&existing, &[], &MergeOptions::default()) {
            RefreshOutcome::PreservedOnEmpty { existing_count } => assert_eq!(existing_count, 1),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parse_error_preserves_existing() {
        let existing = vec![node("keep", "a", 1)];
        let outcome = refresh_result(
            &existing,
            Err(SubError::Decode("bad".into())),
            &MergeOptions::default(),
        );
        assert!(matches!(outcome, RefreshOutcome::PreservedOnError(_, 1)));
    }
}
