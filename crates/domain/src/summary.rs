//! Lightweight profile row used by the profiles table.
//!
//! This is the T01 14-column shape (`compat/layouts.yaml` LAY-PROFILES-002)
//! plus the row identity and owning core. It is a *view* type: the full
//! persisted entity is [`crate::Profile`].

use serde::{Deserialize, Serialize};

use crate::enums::{ConfigType, CoreType};

/// One row of the profiles table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: String,
    pub config_type: ConfigType,
    pub remarks: String,
    pub address: String,
    pub port: u16,
    pub network: String,
    pub stream_security: String,
    pub sub_remarks: String,
    pub delay: i32,
    pub speed: String,
    pub today_up: u64,
    pub ip_info: String,
    pub today_down: u64,
    pub total_up: u64,
    pub total_down: u64,
    pub core_type: CoreType,
}
