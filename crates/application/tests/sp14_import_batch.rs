//! SP-14 All纯预览批导入：整批原子 + token/revision/mutation 正确合同。
//!
//! 合成数据专用；SQLite 落在 `tempfile` 独立目录；端口/宿主网络/系统代理均不触碰。

use application::import_batch::{preview_token, ImportCommit};
use application::AppEngine;
use domain::{ConfigType, Profile as DomainProfile};
use std::sync::Arc;

fn vless_profile(remarks: &str) -> DomainProfile {
    DomainProfile {
        index_id: String::new(),
        config_type: ConfigType::Vless,
        remarks: remarks.to_string(),
        address: "192.0.2.1".to_string(),
        port: 443,
        ..Default::default()
    }
}

fn test_engine() -> AppEngine {
    AppEngine::with_runtime(Arc::new(application::NullRuntimeClient::new()))
}

#[test]
fn preview_token_binds_content() {
    let a = preview_token("vless://a@192.0.2.1:443#one");
    let b = preview_token("vless://a@192.0.2.1:443#one");
    let c = preview_token("vless://b@192.0.2.2:443#two");
    assert!(!a.is_empty());
    assert_eq!(a, b, "same content must bind the same token");
    assert_ne!(a, c, "different content must not share a token");
}

#[test]
fn preview_token_matches_fnv1a_vectors() {
    // Standard FNV-1a 64 vectors; the Dart seam implements the same algorithm
    // so both layers bind identical tokens for identical text.
    assert_eq!(preview_token(""), "cbf29ce484222325");
    assert_eq!(preview_token("a"), "af63dc4c8601ec8c");
}

#[test]
fn bulk_10k_grouped_and_ungrouped_consistent() {
    // SP-14 acceptance scale: 10k All/ungrouped and 10k grouped batches commit
    // whole and read back identically, including across an independent reopen.
    for target in [None, Some("sp14-bulk-group")] {
        let dir = tempfile::tempdir().expect("temp dir");
        let engine = AppEngine::open_with_runtime(
            dir.path(),
            Arc::new(application::NullRuntimeClient::new()),
        )
        .expect("open engine");
        let profiles: Vec<DomainProfile> = (0..10_000)
            .map(|i| DomainProfile {
                index_id: String::new(),
                config_type: ConfigType::Vless,
                remarks: format!("sp14-bulk-{i:05}"),
                address: "192.0.2.1".to_string(),
                port: 443,
                ..Default::default()
            })
            .collect();
        let started = std::time::Instant::now();
        let token = engine.register_import_preview("sp14-bulk-10k", &profiles);
        let commit = ImportCommit {
            profiles,
            target_group: target.map(str::to_string),
            expected_revision: engine.desired_revision(),
            mutation_id: format!("sp14-m-bulk-{}", target.unwrap_or("all")),
            preview_token: token,
        };
        let receipt = engine.commit_import_batch(commit).expect("bulk commit ok");
        let elapsed = started.elapsed();
        assert_eq!(receipt.imported, 10_000);
        assert_eq!(engine.profile_count(), 10_000);
        println!("SP-14 10k commit target={:?} elapsed={:?}", target, elapsed);
        engine.reopen().expect("reopen");
        assert_eq!(engine.profile_count(), 10_000);
        let page = engine
            .query_profiles(
                application::ProfileFilter::default(),
                application::ProfileSort::IndexId,
                application::PageRequest {
                    cursor: 0,
                    page_size: 10_000,
                },
            )
            .expect("query")
            .items;
        assert_eq!(page.len(), 10_000);
        let wanted = target.unwrap_or_default().to_string();
        assert!(
            page.iter().all(|p| p.subid == wanted),
            "every row keeps its target group"
        );
    }
}

#[test]
fn commit_is_atomic_and_idempotent() {
    let engine = test_engine();
    let expected = engine.desired_revision();
    let profiles = vec![vless_profile("sp14-a"), vless_profile("sp14-b")];
    let token = engine.register_import_preview("batch-a-b", &profiles);
    let commit = ImportCommit {
        profiles,
        target_group: None,
        expected_revision: expected,
        mutation_id: "sp14-m-atomic".to_string(),
        preview_token: token,
    };
    let receipt = engine
        .commit_import_batch(commit.clone())
        .expect("commit ok");
    assert!(receipt.ok);
    assert_eq!(receipt.imported, 2);
    assert_eq!(engine.profile_count(), 2);

    // 同一 mutationId 重放：返回同一收据，不再写第二遍。
    let replay = engine.commit_import_batch(commit).expect("replay ok");
    assert_eq!(replay.imported, receipt.imported);
    assert_eq!(replay.commit_id, receipt.commit_id);
    assert_eq!(engine.profile_count(), 2);
}

#[test]
fn stale_revision_rejected_without_write() {
    let engine = test_engine();
    let before = engine.profile_count();
    let profiles = vec![vless_profile("sp14-stale")];
    let token = engine.register_import_preview("stale", &profiles);
    let stale = ImportCommit {
        profiles,
        target_group: None,
        expected_revision: engine.desired_revision() + 99,
        mutation_id: "sp14-m-stale".to_string(),
        preview_token: token,
    };
    let err = engine
        .commit_import_batch(stale)
        .expect_err("stale must fail");
    assert_eq!(err.code, domain::codes::REVISION_STALE);
    assert_eq!(engine.profile_count(), before);
}

#[test]
fn wrong_preview_token_rejected_without_write() {
    let engine = test_engine();
    let before = engine.profile_count();
    let profiles = vec![vless_profile("sp14-tok")];
    let _ = engine.register_import_preview("real-preview", &profiles);
    assert!(!engine.check_import_token(&profiles, "tampered"));
    let commit = ImportCommit {
        profiles,
        target_group: None,
        expected_revision: engine.desired_revision(),
        mutation_id: "sp14-m-tok".to_string(),
        preview_token: "tampered".to_string(),
    };
    let err = engine
        .commit_import_batch(commit)
        .expect_err("token must fail");
    assert_eq!(err.code, domain::codes::CONFLICT);
    assert_eq!(engine.profile_count(), before);
}

#[test]
fn custom_staging_rolls_back_on_db_failure() {
    // Custom 全量配置：暂存文件 + DB 事务要么一起成功，要么都不留。
    let dir = tempfile::tempdir().expect("temp dir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("open engine");
    let raw = r#"{"outbounds":[{"tag":"sp14-custom"}],"x-future":1}"#.to_string();
    let mut custom = DomainProfile {
        index_id: String::new(),
        config_type: ConfigType::Custom,
        remarks: "sp14-custom".to_string(),
        ..Default::default()
    };
    custom
        .extra
        .insert("RawConfig".to_string(), serde_json::Value::String(raw));
    let profiles = vec![custom, vless_profile("sp14-plain")];
    let token = engine.register_import_preview("custom-batch", &profiles);
    let commit = ImportCommit {
        profiles,
        target_group: None,
        expected_revision: engine.desired_revision(),
        mutation_id: "sp14-m-custom".to_string(),
        preview_token: token,
    };
    // 注入 DB 失败：暂存文件必须被清理，不留孤儿。
    engine.set_import_commit_fault(true);
    let err = engine
        .commit_import_batch(commit)
        .expect_err("fault must fail");
    assert_eq!(err.code, domain::codes::INTERNAL);
    assert_eq!(engine.profile_count(), 0);
    let config_dir = dir.path().join("config");
    let orphans = std::fs::read_dir(&config_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(orphans, 0, "failed commit must not leave staged files");
}

#[test]
fn custom_commit_persist_failure_rolls_back_rows_and_staged_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("open engine");
    let mut custom = DomainProfile {
        index_id: String::new(),
        config_type: ConfigType::Custom,
        remarks: "sp14-persist-failure".to_string(),
        ..Default::default()
    };
    custom.extra.insert(
        "RawConfig".to_string(),
        serde_json::Value::String(r#"{"outbounds":[{"tag":"persist-failure"}]}"#.into()),
    );
    let profiles = vec![custom];
    let token = engine.register_import_preview("custom-persist-failure", &profiles);
    let commit = ImportCommit {
        profiles,
        target_group: None,
        expected_revision: engine.desired_revision(),
        mutation_id: "sp14-m-persist-failure".to_string(),
        preview_token: token,
    };
    std::fs::create_dir(dir.path().join("guiNConfig.json.tmp")).unwrap();

    assert!(engine.commit_import_batch(commit).is_err());
    assert_eq!(engine.profile_count(), 0);
    assert_eq!(engine.desired_revision(), 0);
    assert_eq!(
        std::fs::read_dir(dir.path().join("config"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn custom_commit_succeeds_and_reopens() {
    let dir = tempfile::tempdir().expect("temp dir");
    let engine =
        AppEngine::open_with_runtime(dir.path(), Arc::new(application::NullRuntimeClient::new()))
            .expect("open engine");
    let raw = r#"{"outbounds":[{"tag":"sp14-ok"}],"x-future":1}"#.to_string();
    let mut custom = DomainProfile {
        index_id: String::new(),
        config_type: ConfigType::Custom,
        remarks: "sp14-ok".to_string(),
        ..Default::default()
    };
    custom
        .extra
        .insert("RawConfig".to_string(), serde_json::Value::String(raw));
    let profiles = vec![custom];
    let token = engine.register_import_preview("custom-ok", &profiles);
    let commit = ImportCommit {
        profiles,
        target_group: None,
        expected_revision: engine.desired_revision(),
        mutation_id: "sp14-m-ok".to_string(),
        preview_token: token,
    };
    let receipt = engine.commit_import_batch(commit).expect("commit ok");
    assert_eq!(receipt.imported, 1);
    let stored = engine
        .query_profiles(
            application::ProfileFilter::default(),
            application::ProfileSort::IndexId,
            application::PageRequest {
                cursor: 0,
                page_size: 10,
            },
        )
        .expect("query")
        .items;
    assert_eq!(stored.len(), 1);
    assert!(
        !stored[0].address.trim().is_empty(),
        "custom commit must materialize the file Address"
    );
    let materialized = dir.path().join("config").join(&stored[0].address);
    assert!(materialized.exists(), "staged file must exist after commit");
    // 独立重开仍一致。
    engine.reopen().expect("reopen");
    assert_eq!(engine.profile_count(), 1);
}
