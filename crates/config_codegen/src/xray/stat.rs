//! Xray statistics/API generation (`V2rayStatisticService`).

use serde_json::{json, Value};

use crate::input::CodegenInput;
use crate::util::LOOPBACK;

/// Returns `(stats, metrics, policy)` values, any of which may be absent.
pub(crate) fn build_statistic(
    input: &CodegenInput,
) -> (Option<Value>, Option<Value>, Option<Value>) {
    let gui = &input.settings.gui;
    if !gui.enable_statistics && !gui.display_real_time_speed {
        return (None, None, None);
    }
    let stats = json!({});
    let metrics = json!({"listen": format!("{LOOPBACK}:{}", input.settings.state_port)});
    let policy = json!({
        "system": {
            "statsOutboundUplink": true,
            "statsOutboundDownlink": true
        }
    });
    (Some(stats), Some(metrics), Some(policy))
}
