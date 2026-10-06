//! SP-14 All纯预览批导入：整批原子提交基础设施。
//!
//! 语义（IMPLEMENTATION_PLAN §3.5）：`PreviewImport` 不写库；
//! `CommitImport(previewToken, expectedRevision, mutationId, targetGroup?)`
//! 一次提交整批。`preview_token` 与 Dart 侧 FNV-1a 实现一致，
//! 跨层绑定同一预览内容；`mutation_id` 幂等复用 SP-02 的
//! `commit_id_for` 推导。

use domain::{ConfigType, DomainError, Profile};

/// FNV-1a 64 hex：预览内容绑定 token。
///
/// 与 `apps/desktop/lib/features/subs/import_persistence.dart` 的
/// `_previewTokenFor` 同算法，跨层可比对。FRB 正式 `CommitImportRequest`
/// 接线由整合者完成前，本 token 在 Dart 预览/提交 seam 内强制一致。
pub fn preview_token(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 整批提交请求（SP-00 `CommitImportRequest` 的应用层形态）。
#[derive(Debug, Clone)]
pub struct ImportCommit {
    pub profiles: Vec<Profile>,
    pub target_group: Option<String>,
    pub expected_revision: u64,
    pub mutation_id: String,
    pub preview_token: String,
}

/// 整批提交收据（SP-00 `CommitImportResult` 的应用层形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReceipt {
    pub ok: bool,
    pub imported: u32,
    pub commit_id: String,
    pub new_revision: u64,
}

/// 预览内容摘要：参与绑定的稳定字段（不含易变的 `index_id`/`subid`）。
pub fn content_digest(profiles: &[Profile]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for profile in profiles {
        mix(profile.config_type.value().to_le_bytes().as_slice());
        mix(profile.remarks.as_bytes());
        mix(profile.address.as_bytes());
        mix(profile.port.to_le_bytes().as_slice());
        mix(profile.network.as_bytes());
        if let Some(raw) = profile.extra.get("RawConfig").and_then(|v| v.as_str()) {
            mix(raw.as_bytes());
        }
    }
    format!("{hash:016x}")
}

/// `Custom`/`Outbound` 全量配置的文件后缀（与 bridge 侧一致）。
pub fn config_extension(raw: &str) -> &'static str {
    subscriptions::detect_config_extension(raw)
}

/// 内容寻址文件名：同一预览重复提交复用同一文件，不留孤儿。
pub fn staged_file_name(raw: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in raw.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("import-{hash:016x}{}", config_extension(raw))
}

/// 空提交拒绝：无半批概念，直接返回结构化错误。
pub fn empty_commit_error() -> DomainError {
    DomainError::new(domain::codes::FIELD_FORMAT, "error.import_nothing")
}

/// token 未绑定拒绝（调用方必须先登记同一预览）。
pub fn token_mismatch_error() -> DomainError {
    DomainError::new(domain::codes::CONFLICT, "error.preview_mismatch")
        .with_detail("commit preview token does not match the registered preview")
}

/// 过期 revision 拒绝（不写库）。
pub fn stale_revision_error(expected: u64, actual: u64) -> DomainError {
    DomainError::stale_revision(expected, actual)
}

/// 仅协议/复合组草稿以外的普通节点默认 `is_sub = false`（手工批导入语义）。
pub fn normalize_batch(mut profile: Profile, target_group: &str) -> Profile {
    if profile.subid.is_empty() {
        profile.subid = target_group.to_string();
    }
    if profile.index_id.trim().is_empty() {
        profile.index_id = crate::repository::new_index_id();
    }
    if !matches!(
        profile.config_type,
        ConfigType::PolicyGroup | ConfigType::ProxyChain
    ) {
        profile.is_sub = false;
    }
    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_stable_and_content_bound() {
        let a = preview_token("vless://a@192.0.2.1:443#one");
        assert_eq!(a, preview_token("vless://a@192.0.2.1:443#one"));
        assert_ne!(a, preview_token("vless://b@192.0.2.2:443#two"));
        assert_eq!(a.len(), 16);
    }
}
