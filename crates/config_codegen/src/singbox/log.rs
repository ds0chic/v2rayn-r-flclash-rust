//! sing-box log generation (`SingboxLogService`).

use serde_json::{json, Value};

use crate::input::CodegenInput;
use crate::util::{join_path, obj, NONE};

pub(crate) fn build_log(input: &CodegenInput) -> Value {
    let core_basic = &input.settings.core_basic;
    let mut log = obj();
    match core_basic.loglevel.as_str() {
        "debug" | "info" | "error" => {
            log.insert("level".into(), json!(core_basic.loglevel));
        }
        "warning" => {
            log.insert("level".into(), json!("warn"));
        }
        _ => {}
    }
    if core_basic.loglevel == NONE {
        log.insert("disabled".into(), json!(true));
    }
    if core_basic.log_enabled {
        log.insert(
            "output".into(),
            json!(join_path(
                &input.settings.log_directory,
                &format!("sbox_{}.txt", input.settings.log_date)
            )),
        );
    }
    // Base template `SingboxSampleClientConfig` keeps timestamp.
    log.insert("timestamp".into(), json!(true));
    Value::Object(log)
}
