//! T10: group/chain save-reopen, cycle rejection, filter/sub-children,
//! child order, template CRUD and Custom/Outbound validation — all through
//! the real `AppEngine` (SQLite when a data dir is used).

use std::sync::Arc;

use application::runtime_client::NullRuntimeClient;
use application::{AppEngine, PageRequest, ProfileFilter, ProfileSort};
use domain::{ConfigType, CoreType, DesiredRevision, FullConfigTemplate, MultipleLoad, Profile};

fn open(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn leaf(id: &str, subid: &str, remarks: &str) -> Profile {
    Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        subid: subid.into(),
        remarks: remarks.into(),
        address: "192.0.2.1".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    }
}

fn save(engine: &AppEngine, profile: Profile) -> Profile {
    let revision = engine.desired_revision();
    engine
        .save_profile(profile, DesiredRevision::new(revision))
        .expect("save")
        .0
}

fn group(remarks: &str, children: &[&str], load: MultipleLoad) -> Profile {
    let mut profile = Profile {
        index_id: format!("group-{remarks}"),
        config_type: ConfigType::PolicyGroup,
        remarks: remarks.into(),
        ..Default::default()
    };
    profile.proto_extra.group_type = Some("PolicyGroup".into());
    profile.proto_extra.multiple_load = Some(load);
    if !children.is_empty() {
        profile.proto_extra.child_items = Some(
            children
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    profile
}

#[test]
fn group_save_reopen_keeps_child_order() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    save(&engine, leaf("c1", "", "HK-1"));
    save(&engine, leaf("c2", "", "HK-2"));
    save(&engine, leaf("c3", "", "US-1"));
    let saved = save(&engine, group("g", &["c2", "c1"], MultipleLoad::LeastPing));
    assert_eq!(saved.proto_extra.group_type.as_deref(), Some("PolicyGroup"));

    let reopened = open(dir.path());
    let children = reopened.group_children(&saved.index_id).unwrap();
    let ids: Vec<&str> = children.iter().map(|p| p.index_id.as_str()).collect();
    assert_eq!(ids, vec!["c2", "c1"]);

    // Query path also round-trips the extras.
    let page = reopened
        .query_profiles(
            ProfileFilter::default(),
            ProfileSort::IndexId,
            PageRequest {
                cursor: 0,
                page_size: 100,
            },
        )
        .unwrap();
    assert!(page.items.iter().any(|p| p.index_id == saved.index_id));
}

#[test]
fn proxy_chain_save_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    save(&engine, leaf("n1", "", "entry"));
    save(&engine, leaf("n2", "", "exit"));
    let mut chain = group("chain", &["n1", "n2"], MultipleLoad::RoundRobin);
    chain.index_id = "chain-1".into();
    chain.config_type = ConfigType::ProxyChain;
    chain.proto_extra.group_type = Some("ProxyChain".into());
    let saved = save(&engine, chain);
    assert_eq!(saved.config_version, 4);

    let reopened = open(dir.path());
    let children = reopened.group_children(&saved.index_id).unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].index_id, "n1");
}

#[test]
fn cycle_is_rejected_on_update() {
    let engine = AppEngine::in_memory();
    save(&engine, leaf("l1", "", "leaf"));
    let mut a = group("a", &["l1"], MultipleLoad::LeastPing);
    a.index_id = "a".into();
    save(&engine, a);
    let mut b = group("b", &["a"], MultipleLoad::Fallback);
    b.index_id = "b".into();
    save(&engine, b);
    // Closing the loop a -> b -> a must fail.
    let mut a2 = group("a", &["b"], MultipleLoad::LeastPing);
    a2.index_id = "a".into();
    let revision = engine.desired_revision();
    let err = engine
        .save_profile(a2, DesiredRevision::new(revision))
        .unwrap_err();
    assert_eq!(err.code, domain::codes::GRAPH_CYCLE);
}

#[test]
fn self_cycle_is_rejected() {
    let engine = AppEngine::in_memory();
    let c = group("c", &["c"], MultipleLoad::Random);
    let revision = engine.desired_revision();
    let err = engine
        .save_profile(c, DesiredRevision::new(revision))
        .unwrap_err();
    // Self id is unknown to the map, so it fails as a dangling reference.
    assert!(matches!(
        err.code.as_str(),
        "E_DANGLING_REFERENCE" | "E_GRAPH_CYCLE"
    ));
}

#[test]
fn missing_child_and_bad_filter_rejected() {
    let engine = AppEngine::in_memory();
    let g = group("g", &["nope"], MultipleLoad::LeastPing);
    let revision = engine.desired_revision();
    let err = engine
        .save_profile(g, DesiredRevision::new(revision))
        .unwrap_err();
    assert_eq!(err.code, domain::codes::DANGLING_REFERENCE);

    let mut f = group("f", &[], MultipleLoad::LeastPing);
    f.index_id = "f".into();
    f.proto_extra.child_items = None;
    f.proto_extra.sub_child_items = Some("sub-1".into());
    f.proto_extra.filter = Some("(".into());
    let revision = engine.desired_revision();
    let err = engine
        .save_profile(f, DesiredRevision::new(revision))
        .unwrap_err();
    assert_eq!(err.code, domain::codes::FIELD_FORMAT);
}

#[test]
fn sub_children_with_self_sentinel_and_filter() {
    let engine = AppEngine::in_memory();
    save(&engine, leaf("a", "sub-1", "HK-1"));
    save(&engine, leaf("b", "sub-1", "HK-2"));
    save(&engine, leaf("c", "sub-1", "US-1"));
    save(&engine, leaf("d", "sub-2", "HK-3"));
    let mut g = group("g", &[], MultipleLoad::LeastPing);
    g.index_id = "g".into();
    g.subid = "sub-1".into();
    g.proto_extra.child_items = None;
    g.proto_extra.sub_child_items = Some("self".into());
    g.proto_extra.filter = Some("^HK".into());
    let saved = save(&engine, g);
    let children = engine.group_children(&saved.index_id).unwrap();
    let ids: Vec<&str> = children.iter().map(|p| p.index_id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b"]);
}

#[test]
fn all_five_multiple_load_modes_validate() {
    let engine = AppEngine::in_memory();
    save(&engine, leaf("m1", "", "m1"));
    for (index, mode) in [
        MultipleLoad::LeastPing,
        MultipleLoad::Fallback,
        MultipleLoad::Random,
        MultipleLoad::RoundRobin,
        MultipleLoad::LeastLoad,
    ]
    .into_iter()
    .enumerate()
    {
        let mut g = group(&format!("mode-{index}"), &["m1"], mode);
        g.index_id = format!("mode-{index}");
        save(&engine, g);
    }
    assert_eq!(engine.profile_count(), 6);
}

#[test]
fn gen_group_all_and_region() {
    let engine = AppEngine::in_memory();
    let sub = application::SubItem {
        id: "sub-1".into(),
        remarks: "demo".into(),
        url: "https://example.com/sub".into(),
        ..Default::default()
    };
    engine.save_sub_item(sub).unwrap();
    save(&engine, leaf("h1", "sub-1", "HK-1"));
    save(&engine, leaf("h2", "sub-1", "HK-2"));
    save(&engine, leaf("u1", "sub-1", "US-1"));

    let all = engine.gen_group_all("sub-1").unwrap();
    assert_eq!(all.config_type, ConfigType::PolicyGroup);
    assert_eq!(all.proto_extra.sub_child_items.as_deref(), Some("sub-1"));

    let regions = engine.gen_group_region("sub-1").unwrap();
    let mut names: Vec<String> = regions.iter().map(|p| p.remarks.clone()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["demo - HK".to_string(), "demo - US".to_string()]
    );
    // Region groups resolve to their filtered members.
    let hk = regions.iter().find(|p| p.remarks.ends_with("HK")).unwrap();
    let children = engine.group_children(&hk.index_id).unwrap();
    assert_eq!(children.len(), 2);
}

#[test]
fn template_crud_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    let initial = engine.list_templates().unwrap();
    assert_eq!(initial.len(), 2);

    let item = FullConfigTemplate {
        remarks: "V2ray".into(),
        core_type: CoreType::Xray,
        enabled: true,
        config: Some(r#"{"log": {"loglevel": "warning"}}"#.into()),
        tun_config: None,
        add_proxy_only: Some(true),
        proxy_detour: Some("direct".into()),
        ..Default::default()
    };
    let saved = engine.save_template(item).unwrap();
    assert_eq!(saved.id, "builtin-xray");

    let fetched = engine
        .get_template_for_core(CoreType::Xray)
        .unwrap()
        .unwrap();
    assert_eq!(
        fetched.config.as_deref(),
        Some(r#"{"log": {"loglevel": "warning"}}"#)
    );
    assert_eq!(fetched.add_proxy_only, Some(true));

    // Invalid JSON is refused and never persisted.
    let mut bad = fetched.clone();
    bad.config = Some("{not json".into());
    assert!(engine.save_template(bad).is_err());

    // Unsupported core is refused.
    let mut wrong_core = fetched.clone();
    wrong_core.id = "other".into();
    wrong_core.core_type = CoreType::Mihomo;
    assert!(engine.save_template(wrong_core).is_err());

    // Delete removes the row (and only that row); a second delete is a no-op.
    assert!(engine.delete_template(&saved.id).unwrap());
    assert!(!engine.delete_template(&saved.id).unwrap());
    assert_eq!(engine.list_templates().unwrap().len(), 1);

    // Reopen: the deletion survived; the sing-box builtin remains.
    let reopened = open(dir.path());
    let items = reopened.list_templates().unwrap();
    assert_eq!(items.len(), 1);
    assert!(reopened
        .get_template_for_core(CoreType::SingBox)
        .unwrap()
        .is_some());
    assert!(reopened
        .get_template_for_core(CoreType::Xray)
        .unwrap()
        .is_none());
}

#[test]
fn custom_outbound_validation() {
    let engine = AppEngine::in_memory();
    let mut custom = Profile {
        index_id: "cu-1".into(),
        config_type: ConfigType::Custom,
        core_type: Some(CoreType::Xray),
        remarks: "custom".into(),
        address: "custom.json".into(),
        display_log: true,
        ..Default::default()
    };
    custom.proto_extra.extra.insert(
        "customConfigText".into(),
        serde_json::Value::String(r#"{"log": {}}"#.into()),
    );
    custom.proto_extra.is_singbox_endpoint = None;
    let saved = save(&engine, custom);
    assert_eq!(saved.config_version, 4);

    let mut outbound = Profile {
        index_id: "ob-1".into(),
        config_type: ConfigType::Outbound,
        remarks: "outbound".into(),
        address: "outbound.json".into(),
        pre_socks_port: Some(99999),
        ..Default::default()
    };
    outbound.proto_extra.is_singbox_endpoint = Some(true);
    let revision = engine.desired_revision();
    let err = engine
        .save_profile(outbound, DesiredRevision::new(revision))
        .unwrap_err();
    assert_eq!(err.code, domain::codes::FIELD_RANGE);
}
