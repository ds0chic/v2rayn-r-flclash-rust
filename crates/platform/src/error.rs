//! Shared platform error type.

use std::fmt;
use std::net::SocketAddr;

/// Errors returned by platform backends.
#[derive(Debug)]
pub enum PlatformError {
    /// A required path does not exist (custom PAC/script field validation).
    NotFound(String),
    /// A value is malformed or an argument is unusable.
    Invalid(String),
    /// The requested local port is already bound.
    PortInUse(SocketAddr),
    /// Transport/IO failure (PAC socket operations, file reads).
    Io(std::io::Error),
    /// The backend has no implementation on this platform.
    Unsupported(&'static str),
    /// An OS API call failed; carries a human-readable diagnostic.
    Backend(String),
    /// A single-instance named mutex already exists.
    AlreadyRunning(String),
}

/// Convenience alias used across the crate.
pub type Result<T> = std::result::Result<T, PlatformError>;

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::NotFound(p) => write!(f, "path not found: {p}"),
            PlatformError::Invalid(m) => write!(f, "invalid argument: {m}"),
            PlatformError::PortInUse(a) => write!(f, "address already in use: {a}"),
            PlatformError::Io(e) => write!(f, "io error: {e}"),
            PlatformError::Unsupported(w) => write!(f, "unsupported on this platform: {w}"),
            PlatformError::Backend(m) => write!(f, "platform backend error: {m}"),
            PlatformError::AlreadyRunning(n) => write!(f, "single instance already running: {n}"),
        }
    }
}

impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlatformError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PlatformError {
    fn from(e: std::io::Error) -> Self {
        PlatformError::Io(e)
    }
}
