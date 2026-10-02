//! Temporary T18b evidence seeder (deleted after the screenshot run).
//!
//! Seeds a data directory with one Xray node, marks it active and moves the
//! inbound base port to 11808 (never 10808), so the debug app's
//! `V2RAYN_R_AUTOSTART` hook applies the real persisted plan on launch.

use std::sync::Arc;

use application::{AppEngine, NetHostClient};
use domain::{ConfigType, CoreType, DesiredRevision, Profile};

fn main() {
    let dir = std::env::args().nth(1).expect("usage: t18b_seed <data_dir>");
    let engine = AppEngine::open_with_runtime(
        std::path::Path::new(&dir),
        Arc::new(NetHostClient::new()),
    )
    .expect("open engine");
    let mut node = Profile {
        index_id: "t18b-node".into(),
        config_type: ConfigType::Vless,
        core_type: Some(CoreType::Xray),
        remarks: "t18b-applied".into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    node.proto_extra.vless_encryption = Some("none".into());
    let revision = engine.desired_revision();
    engine
        .save_profile(node, DesiredRevision::new(revision))
        .expect("save profile");
    engine
        .set_active(Some("t18b-node".into()))
        .expect("set active");
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = 11808;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
    println!(
        "seeded dir={dir} active=t18b-node desired={}",
        engine.desired_revision()
    );
}
