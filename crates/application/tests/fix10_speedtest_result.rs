//! FIX-10: speedtest result lifecycle and node-management parity.
//!
//! Covers the parts that live below the bridge:
//!   * `ProfileExItem` (delay/speed/sort/message/ip) persists to SQLite and is
//!     reloaded by a reopened engine (PR-17);
//!   * `Sort` written by move/column order survives reopen (PR-15 store side);
//!   * `remove_invalid_profiles` deletes only the failed, non-complex profiles
//!     of the given group (PR-11);
//!   * `deduplicate_profiles` collapses transport-identical nodes and keeps the
//!     older entry per `KeepOlderDedupl` (PR-12), preserving complex nodes.
//!
//! All addresses are RFC 5737 / reserved example addresses; no network is used.

use std::sync::Arc;

use application::runtime_client::NullRuntimeClient;
use application::speedtest::{deduplicate_profiles, ProfileExItem};
use application::{AppEngine, PageRequest, ProfileFilter, ProfileSort};
use domain::{ConfigType, CoreType, Profile};

fn open(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn vless(id: &str, subid: &str, addr: &str, port: i32) -> Profile {
    Profile {
        index_id: id.to_string(),
        config_type: ConfigType::Vless,
        core_type: Some(CoreType::Xray),
        subid: subid.to_string(),
        is_sub: true,
        remarks: format!("node-{id}"),
        address: addr.into(),
        port,
        // Dedup identity is transport-based; a shared credential makes two
        // entries with the same address/port genuine duplicates.
        password: "shared-uuid".into(),
        network: "raw".into(),
        proto_extra: domain::ProtocolExtra {
            vless_encryption: Some("none".into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Same transport identity as [`vless`], but a distinct id/remarks.
fn vless_dup(id: &str, subid: &str, addr: &str, port: i32) -> Profile {
    let mut p = vless(id, subid, addr, port);
    p.remarks = format!("dup-{id}");
    p
}

fn save(engine: &AppEngine, profile: Profile) {
    let rev = engine.desired_revision();
    engine
        .save_imported_profile(profile, domain::DesiredRevision::new(rev))
        .expect("save");
}

fn ex(id: &str, delay: i32, speed: f64) -> ProfileExItem {
    ProfileExItem {
        index_id: id.to_string(),
        delay,
        speed,
        sort: 0,
        message: String::new(),
        ip_info: String::new(),
    }
}

#[test]
fn profile_ex_and_sort_survive_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");

    {
        let engine = open(dir.path());
        save(&engine, vless("a", "sub-1", "192.0.2.10", 443));
        save(&engine, vless("b", "sub-1", "192.0.2.11", 443));

        engine
            .profile_ex_flush(&[
                ex("a", 42, 8.5),
                ex("b", -1, 0.0),
                ProfileExItem {
                    message: "fail".into(),
                    ..ex("b", -1, 0.0)
                },
            ])
            .expect("flush");
        engine
            .set_profile_sort(&["b".to_string(), "a".to_string()])
            .expect("sort");
    }

    let engine = open(dir.path());
    let rows = engine.profile_ex_all().expect("reload");
    let a = rows.iter().find(|r| r.index_id == "a").expect("a row");
    let b = rows.iter().find(|r| r.index_id == "b").expect("b row");
    assert_eq!(a.delay, 42);
    assert_eq!(a.speed, 8.5);
    assert_eq!(b.delay, -1);
    // `b` is first after the move → sort 10; `a` second → sort 20.
    assert_eq!(b.sort, 10, "moved-first node keeps its persisted Sort");
    assert_eq!(a.sort, 20);

    // Re-derive order from the persisted Sort and confirm it is stable.
    let mut ordered: Vec<(&str, i32)> =
        rows.iter().map(|r| (r.index_id.as_str(), r.sort)).collect();
    ordered.sort_by_key(|(_, s)| *s);
    assert_eq!(ordered[0].0, "b");
    assert_eq!(ordered[1].0, "a");
}

#[test]
fn remove_invalid_deletes_only_failed_non_complex_in_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());

    save(&engine, vless("ok", "sub-1", "192.0.2.20", 443));
    save(&engine, vless("bad", "sub-1", "192.0.2.21", 443));
    // A failed node in another group must never be deleted from sub-1.
    save(&engine, vless("other-group", "sub-2", "192.0.2.22", 443));
    engine
        .profile_ex_flush(&[
            ex("ok", 30, 1.0),
            ex("bad", -1, 0.0),
            ex("other-group", -1, 0.0),
        ])
        .expect("flush");

    let removed = engine.remove_invalid_profiles("sub-1").expect("remove");
    assert_eq!(removed, 1, "only the failed node of sub-1 is deleted");

    assert!(engine.profile_by_id("ok").unwrap().is_some());
    assert!(engine.profile_by_id("bad").unwrap().is_none());
    assert!(
        engine.profile_by_id("other-group").unwrap().is_some(),
        "another group's failed node stays"
    );
}

#[test]
fn deduplicate_keeps_older_and_preserves_complex_nodes() {
    // Two transport-identical vless nodes plus a policy group.
    let a = vless("dup-old", "sub-1", "192.0.2.30", 443);
    let b = vless_dup("dup-new", "sub-1", "192.0.2.30", 443);
    let group = Profile {
        index_id: "grp".into(),
        config_type: ConfigType::PolicyGroup,
        ..Default::default()
    };

    // keep_older=true keeps the first occurrence ("dup-old").
    let (kept, removed) = deduplicate_profiles(&[a.clone(), b.clone(), group.clone()], true);
    assert_eq!(kept, 2, "one duplicate collapsed, group preserved");
    assert_eq!(removed, vec!["dup-new".to_string()]);

    // keep_older=false reverses first, so the newer entry wins.
    let (kept, removed) = deduplicate_profiles(&[a.clone(), b.clone(), group], false);
    assert_eq!(kept, 2);
    assert_eq!(removed, vec!["dup-old".to_string()]);
}

#[test]
fn deduplicate_through_engine_persists_removal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());
    save(&engine, vless("d1", "sub-1", "192.0.2.40", 443));
    save(&engine, vless_dup("d2", "sub-1", "192.0.2.40", 443));
    save(&engine, vless("d3", "sub-1", "192.0.2.41", 443));

    let (total, kept, removed) = engine.deduplicate_profiles("sub-1", true).expect("dedup");
    assert_eq!(total, 3);
    assert_eq!(kept, 2);
    assert_eq!(removed.len(), 1);
    engine.delete_profiles(&removed).expect("delete");

    let page = engine
        .query_profiles(
            ProfileFilter {
                subid: Some("sub-1".into()),
                ..Default::default()
            },
            ProfileSort::IndexId,
            PageRequest {
                cursor: 0,
                page_size: u32::MAX,
            },
        )
        .expect("query");
    assert_eq!(page.items.len(), 2, "duplicate really removed from storage");
}
