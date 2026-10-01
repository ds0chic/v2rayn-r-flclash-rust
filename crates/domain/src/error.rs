//! Stable, front-end-safe error model.
//!
//! Rules (plan §14):
//! - every error carries a stable machine `code`;
//! - user-facing text is a *message key* resolved by the UI, never a raw panic
//!   or captured stdout;
//! - optional `field_path` locates the offending form field;
//! - `retryable` tells the UI whether a retry is meaningful;
//! - `operation_id` / `job_id` correlate the failure with an async task;
//! - `detail` is redacted diagnostic text (no credentials, no raw config).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error codes. These strings are part of the IPC and FRB contract and
/// must not change without a protocol version bump.
pub mod codes {
    pub const INVALID_ARGUMENT: &str = "E_INVALID_ARGUMENT";
    pub const INVALID_ENUM: &str = "E_INVALID_ENUM";
    pub const FIELD_REQUIRED: &str = "E_FIELD_REQUIRED";
    pub const FIELD_RANGE: &str = "E_FIELD_RANGE";
    pub const FIELD_FORMAT: &str = "E_FIELD_FORMAT";
    pub const REVISION_STALE: &str = "E_REVISION_STALE";
    pub const NOT_FOUND: &str = "E_NOT_FOUND";
    pub const CONFLICT: &str = "E_CONFLICT";
    pub const INVALID_PLAN: &str = "E_INVALID_PLAN";
    pub const GRAPH_CYCLE: &str = "E_GRAPH_CYCLE";
    pub const PORT_CONFLICT: &str = "E_PORT_CONFLICT";
    pub const DANGLING_REFERENCE: &str = "E_DANGLING_REFERENCE";
    pub const CANCELLED: &str = "E_CANCELLED";
    pub const NOT_CANCELLABLE: &str = "E_NOT_CANCELLABLE";
    pub const TIMEOUT: &str = "E_TIMEOUT";
    pub const IPC_VERSION_MISMATCH: &str = "E_IPC_VERSION_MISMATCH";
    pub const IPC_MESSAGE_TOO_LARGE: &str = "E_IPC_MESSAGE_TOO_LARGE";
    pub const PERMISSION_DENIED: &str = "E_PERMISSION_DENIED";
    pub const UNAVAILABLE: &str = "E_UNAVAILABLE";
    pub const INTERNAL: &str = "E_INTERNAL";
}

/// One structured error value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainError {
    /// Stable machine code, see [`codes`].
    pub code: String,
    /// Message key resolved to localized text by the UI layer.
    pub message_key: String,
    /// Dotted path of the offending field, when applicable.
    pub field_path: Option<String>,
    /// Whether a retry at the same input could plausibly succeed.
    pub retryable: bool,
    /// Correlates with an `OperationId` / `JobId` when the error is async.
    pub operation_id: Option<String>,
    /// Redacted diagnostic detail. Never raw config or credentials.
    pub detail: Option<String>,
}

impl DomainError {
    pub fn new(code: &str, message_key: &str) -> Self {
        Self {
            code: code.to_string(),
            message_key: message_key.to_string(),
            field_path: None,
            retryable: false,
            operation_id: None,
            detail: None,
        }
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field_path = Some(field.into());
        self
    }

    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation_id = Some(operation.into());
        self
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Reject an enum value that is not recognized. `raw` is the displayed
    /// value only; no config contents are carried here.
    pub fn invalid_enum(enum_name: &str, raw: String) -> Self {
        Self::new(codes::INVALID_ENUM, "error.invalid_enum")
            .with_field(enum_name)
            .with_detail(format!("unsupported value `{raw}`"))
    }

    pub fn invalid_argument(message_key: &str, detail: impl Into<String>) -> Self {
        Self::new(codes::INVALID_ARGUMENT, message_key).with_detail(detail)
    }

    pub fn stale_revision(expected: u64, actual: u64) -> Self {
        Self::new(codes::REVISION_STALE, "error.revision_stale")
            .with_detail(format!("expected revision {expected}, current {actual}"))
    }

    pub fn not_found(kind: &str, id: &str) -> Self {
        Self::new(codes::NOT_FOUND, "error.not_found")
            .with_field(kind)
            .with_detail(format!("{kind} `{id}` not found"))
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.code, self.message_key)?;
        if let Some(path) = &self.field_path {
            write!(f, " field={path}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DomainError {}
