//! Subscription/Fmt error model.
//!
//! Errors carry a stable machine code and are safe to surface to the front end.
//! Per-item failures never carry the offending node text: subscription payloads
//! hold credentials, so locators are index/offset only and messages are
//! redacted (AGENTS.md hard rule 4).

use serde::{Deserialize, Serialize};

use domain::error::codes;

/// One located, non-fatal parse failure. The pipeline continues after emitting
/// one of these, so a single malformed line never discards the whole batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseIssue {
    /// Stable machine code, see [`SubError::code`].
    pub code: String,
    /// Redacted message key / short text.
    pub message: String,
    /// 0-based line or array index within the parsed content.
    pub item_index: Option<usize>,
    /// Byte offset of the item within the content, when known.
    pub byte_offset: Option<usize>,
}

impl ParseIssue {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            item_index: None,
            byte_offset: None,
        }
    }

    pub fn at(mut self, index: usize, offset: usize) -> Self {
        self.item_index = Some(index);
        self.byte_offset = Some(offset);
        self
    }

    pub fn from_error(err: &SubError, index: usize, offset: usize) -> Self {
        Self::new(err.code(), err.to_string()).at(index, offset)
    }
}

/// Error raised by a Fmt codec, the parse pipeline or the downloader.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubError {
    /// No content to parse / empty subscription body.
    #[error("empty content")]
    Empty,
    /// Base64/JSON/percent decoding failed.
    #[error("decode failed: {0}")]
    Decode(String),
    /// The content is not one of the supported formats.
    #[error("unsupported format: {0}")]
    Unsupported(String),
    /// A share URI or subscription URL could not be parsed.
    #[error("invalid uri: {0}")]
    InvalidUri(String),
    /// A structural field is missing or out of range.
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    /// A resource guard was tripped (size / count / depth / regex length).
    #[error("limit exceeded: {kind} = {value} (max {max})")]
    LimitExceeded {
        kind: &'static str,
        value: usize,
        max: usize,
    },
    /// Cooperative cancellation was observed.
    #[error("cancelled")]
    Cancelled,
    /// Transport-level failure (connect, TLS, status, body).
    #[error("http error: {0}")]
    Http(String),
    /// The download exceeded its time budget.
    #[error("timeout")]
    Timeout,
    /// The response body exceeded `max_bytes`.
    #[error("response too large")]
    TooLarge,
    /// A configured request header is invalid or disallowed.
    #[error("invalid header: {0}")]
    HeaderInvalid(String),
    /// Local I/O failure while writing/reading a temp artifact.
    #[error("io error: {0}")]
    Io(String),
    /// `via_proxy` was requested but no local proxy endpoint is available.
    /// The caller must surface this instead of silently downloading direct.
    #[error("proxy unavailable")]
    ProxyUnavailable,
}

impl SubError {
    /// Stable machine code mapped onto the shared domain code space so the
    /// bridge can reuse one error vocabulary.
    pub const fn code(&self) -> &'static str {
        match self {
            SubError::Empty => codes::INVALID_ARGUMENT,
            SubError::Decode(_) => codes::FIELD_FORMAT,
            SubError::Unsupported(_) => codes::INVALID_ARGUMENT,
            SubError::InvalidUri(_) => codes::FIELD_FORMAT,
            SubError::InvalidConfig(_) => codes::FIELD_FORMAT,
            SubError::LimitExceeded { .. } => codes::FIELD_RANGE,
            SubError::Cancelled => codes::CANCELLED,
            SubError::Http(_) => codes::UNAVAILABLE,
            SubError::Timeout => codes::TIMEOUT,
            SubError::TooLarge => codes::FIELD_RANGE,
            SubError::HeaderInvalid(_) => codes::FIELD_FORMAT,
            SubError::Io(_) => codes::INTERNAL,
            SubError::ProxyUnavailable => codes::PROXY_UNAVAILABLE,
        }
    }

    /// Whether retrying the same request could plausibly succeed.
    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            SubError::Http(_) | SubError::Timeout | SubError::Io(_)
        )
    }

    /// Convert into the shared front-end-safe error contract.
    pub fn to_domain(&self) -> domain::DomainError {
        let message_key = format!("error.{}", self.message_key());
        let mut err = domain::DomainError::new(self.code(), &message_key);
        if self.retryable() {
            err = err.retryable();
        }
        err.with_detail(self.redacted_detail())
    }

    fn message_key(&self) -> &'static str {
        match self {
            SubError::Empty => "empty_content",
            SubError::Decode(_) => "decode",
            SubError::Unsupported(_) => "unsupported",
            SubError::InvalidUri(_) => "invalid_uri",
            SubError::InvalidConfig(_) => "invalid_config",
            SubError::LimitExceeded { .. } => "limit_exceeded",
            SubError::Cancelled => "cancelled",
            SubError::Http(_) => "http",
            SubError::Timeout => "timeout",
            SubError::TooLarge => "too_large",
            SubError::HeaderInvalid(_) => "header_invalid",
            SubError::Io(_) => "io",
            SubError::ProxyUnavailable => "proxy_unavailable",
        }
    }

    /// A short, credential-free detail string. Never includes the raw payload.
    fn redacted_detail(&self) -> String {
        match self {
            SubError::LimitExceeded { kind, value, max } => format!("{kind}={value} max={max}"),
            SubError::Http(status) => format!("status/transport: {status}"),
            _ => String::new(),
        }
    }
}

impl From<std::io::Error> for SubError {
    fn from(err: std::io::Error) -> Self {
        SubError::Io(err.kind().to_string())
    }
}
