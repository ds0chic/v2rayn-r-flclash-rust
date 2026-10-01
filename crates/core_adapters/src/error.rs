//! Shared classification for failures against a local core endpoint.

/// Classification of a failed HTTP request against a kernel control endpoint.
///
/// This mirrors the observable upstream behaviour (`HttpClientHelper.TryGetAsync`
/// swallows everything and returns `null`; the Clash/Xray services then retry or
/// fall back) but keeps the reason so callers can distinguish "core not up yet"
/// from "wrong secret" from "broken payload".
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    /// TCP connect failed (core process not listening yet / already gone).
    #[error("connection failed: {0}")]
    Connect(String),
    /// Request exceeded the configured timeout.
    #[error("request timed out")]
    Timeout,
    /// Endpoint replied 401 (Clash `secret`/token mismatch).
    #[error("unauthorized")]
    Unauthorized,
    /// Any other non-success HTTP status.
    #[error("http status {0}")]
    Status(u16),
    /// Response body was not the expected JSON shape.
    #[error("response decode failed: {0}")]
    Decode(String),
}

/// Classify a `reqwest` transport error. HTTP status codes are checked by the
/// caller (the error type does not carry the response).
pub(crate) fn classify_reqwest(err: &reqwest::Error) -> HttpError {
    if err.is_timeout() {
        HttpError::Timeout
    } else if err.is_connect() {
        HttpError::Connect(err.to_string())
    } else if err.is_decode() {
        HttpError::Decode(err.to_string())
    } else {
        HttpError::Connect(err.to_string())
    }
}
