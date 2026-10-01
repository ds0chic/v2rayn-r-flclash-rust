//! Import report: machine-readable JSON plus a user-readable summary
//! (plan §11 step 5).

use serde::{Deserialize, Serialize};

/// Overall outcome of an import attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    /// Candidate validated and committed.
    Imported,
    /// Same source fingerprint already present; nothing changed.
    AlreadyImported,
    /// Preconditions failed (not a source / unreadable); nothing changed.
    Rejected,
    /// Candidate built but failed validation/commit; nothing changed.
    Failed,
}

impl ImportStatus {
    pub const fn changed_target(self) -> bool {
        matches!(self, ImportStatus::Imported)
    }
}

/// Per-table accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EntityCount {
    pub table: String,
    pub source_rows: u64,
    pub imported_rows: u64,
    pub migrated_rows: u64,
    pub skipped_rows: u64,
}

/// One migration stage result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    pub migration_id: String,
    pub from_version: i32,
    pub to_version: i32,
    pub entities_touched: u64,
}

/// A single warning/error entry. Messages are redacted: identifiers only, never
/// node credentials or subscription URLs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportIssue {
    pub code: String,
    pub message: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
}

impl ReportIssue {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            entity_type: None,
            entity_id: None,
        }
    }

    pub fn with_entity(
        mut self,
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
    ) -> Self {
        self.entity_type = Some(entity_type.into());
        self.entity_id = Some(entity_id.into());
        self
    }
}

/// Counts behind candidate validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ValidationSummary {
    pub checks_run: u32,
    pub checks_failed: u32,
    pub reference_warnings: u32,
    pub unknown_fields_preserved: u32,
}

/// Full import report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub status: ImportStatus,
    pub source_id: String,
    pub source_kind: String,
    pub source_version: i32,
    pub content_hash: String,
    pub source_fingerprint: String,
    pub batch_id: String,
    pub counts: Vec<EntityCount>,
    pub migrations: Vec<MigrationReport>,
    pub warnings: Vec<ReportIssue>,
    pub errors: Vec<ReportIssue>,
    pub validation: ValidationSummary,
    pub committed: bool,
    pub target_backup: Option<String>,
    pub user_summary: String,
}

impl ImportReport {
    pub fn machine_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn render_user_summary(&self) -> String {
        let mut out = String::new();
        let headline = match self.status {
            ImportStatus::Imported => "导入完成",
            ImportStatus::AlreadyImported => "该来源此前已导入，未重复写入",
            ImportStatus::Rejected => "来源不可识别，未修改现有数据",
            ImportStatus::Failed => "候选校验失败，未修改现有数据",
        };
        out.push_str(headline);
        out.push('\n');
        out.push_str(&format!(
            "来源: {} (版本 {})\n",
            self.source_id, self.source_version
        ));
        out.push_str(&format!("指纹: {}\n", self.source_fingerprint));
        if !self.counts.is_empty() {
            out.push_str("表计数:\n");
            for count in &self.counts {
                out.push_str(&format!(
                    "  {} 源 {} / 导入 {} / 迁移 {} / 跳过 {}\n",
                    count.table,
                    count.source_rows,
                    count.imported_rows,
                    count.migrated_rows,
                    count.skipped_rows
                ));
            }
        }
        for migration in &self.migrations {
            out.push_str(&format!(
                "  迁移 {}: v{}->v{} ({} 条)\n",
                migration.migration_id,
                migration.from_version,
                migration.to_version,
                migration.entities_touched
            ));
        }
        out.push_str(&format!(
            "校验: 运行 {} 项, 失败 {} 项, 引用告警 {} 项, 保留未知字段 {} 个\n",
            self.validation.checks_run,
            self.validation.checks_failed,
            self.validation.reference_warnings,
            self.validation.unknown_fields_preserved
        ));
        for warning in &self.warnings {
            out.push_str(&format!("  告警[{}]: {}\n", warning.code, warning.message));
        }
        for error in &self.errors {
            out.push_str(&format!("  错误[{}]: {}\n", error.code, error.message));
        }
        if let Some(backup) = &self.target_backup {
            out.push_str(&format!("旧库备份: {backup}\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_roundtrips_and_renders() {
        let report = ImportReport {
            status: ImportStatus::Failed,
            source_id: "src".into(),
            source_kind: "directory".into(),
            source_version: 2,
            content_hash: "abc".into(),
            source_fingerprint: "src:abc".into(),
            batch_id: "b1".into(),
            counts: vec![EntityCount {
                table: "ProfileItem".into(),
                source_rows: 3,
                imported_rows: 3,
                migrated_rows: 2,
                skipped_rows: 0,
            }],
            migrations: vec![MigrationReport {
                migration_id: "MIG-ENT-003".into(),
                from_version: 2,
                to_version: 3,
                entities_touched: 2,
            }],
            warnings: vec![ReportIssue::new("W_REF", "dangling reference")],
            errors: vec![ReportIssue::new("E_COUNT", "count mismatch")],
            validation: ValidationSummary {
                checks_run: 7,
                checks_failed: 1,
                ..Default::default()
            },
            committed: false,
            target_backup: None,
            user_summary: String::new(),
        };
        let json = report.machine_json().unwrap();
        assert!(json.contains("\"status\": \"failed\""));
        let text = report.render_user_summary();
        assert!(text.contains("候选校验失败"));
        assert!(text.contains("MIG-ENT-003"));
    }
}
