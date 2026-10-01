//! sing-box experimental generation (`SingboxStatisticService`).

use serde_json::{json, Value};

use crate::input::CodegenInput;
use crate::util::{join_path, LOOPBACK};

/// Returns the `experimental` object (always present because `clash_api` is
/// written unconditionally by the upstream source).
pub(crate) fn build_experimental(input: &CodegenInput) -> Value {
    let mut experimental = serde_json::Map::new();
    experimental.insert(
        "clash_api".into(),
        json!({"external_controller": format!("{LOOPBACK}:{}", input.settings.state_port2)}),
    );
    if input.settings.core_basic.enable_cache_file4_sbox {
        experimental.insert(
            "cache_file".into(),
            json!({
                "enabled": true,
                "path": join_path(&input.settings.bin_directory, "cache.db"),
                "store_fakeip": input.dns.as_ref().map(|d| d.simple.fake_ip).unwrap_or(false)
            }),
        );
    }
    Value::Object(experimental)
}
