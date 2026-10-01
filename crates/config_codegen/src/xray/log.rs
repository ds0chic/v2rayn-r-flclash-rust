//! Xray log generation (`V2rayLogService`).

use serde_json::{json, Value};

use crate::input::CodegenInput;
use crate::util::{join_path, obj};

pub(crate) fn build_log(input: &CodegenInput) -> Value {
    let core_basic = &input.settings.core_basic;
    let mut log = obj();
    if core_basic.log_enabled {
        log.insert(
            "access".into(),
            json!(join_path(
                &input.settings.log_directory,
                &format!("Vaccess_{}.txt", input.settings.log_date)
            )),
        );
        log.insert(
            "error".into(),
            json!(join_path(
                &input.settings.log_directory,
                &format!("Verror_{}.txt", input.settings.log_date)
            )),
        );
    }
    log.insert("loglevel".into(), json!(core_basic.loglevel));
    Value::Object(log)
}
