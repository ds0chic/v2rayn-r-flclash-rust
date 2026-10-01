//! Error model for the update pipeline.
//!
//! Every failure carries enough context to be reported without leaking the
//! request URL's credentials; the caller maps variants to stable error codes.

/// Failure of any stage of the update pipeline.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("invalid metadata: {0}")]
    InvalidMetadata(String),
    #[error("no release found for the requested channel")]
    NoRelease,
    #[error("no asset matched {0}")]
    NoMatchingAsset(String),
    #[error("version {candidate} is not newer than installed {installed}")]
    NotNewer {
        candidate: String,
        installed: String,
    },
    #[error("version {0} exceeds the locked maximum for this core")]
    VersionOutOfRange(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("download timed out")]
    Timeout,
    #[error("download aborted by the peer")]
    Incomplete,
    #[error("download cancelled")]
    Cancelled,
    #[error("download exceeds the {limit} byte limit")]
    TooLarge { limit: u64 },
    #[error("digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("signature verification unsupported: {0}")]
    SignatureUnsupported(String),
    #[error("signature verification failed: {0}")]
    SignatureInvalid(String),
    #[error("architecture mismatch: expected {expected}, found {found}")]
    ArchMismatch { expected: String, found: String },
    #[error("unsafe archive path: {0}")]
    UnsafeArchivePath(String),
    #[error("archive rejected: {0}")]
    UnsafeArchive(String),
    #[error("archive format not supported: {0}")]
    UnsupportedArchive(String),
    #[error("install target escapes the managed root: {0}")]
    UnsafeTarget(String),
    #[error("install plan conflict: {0}")]
    InstallConflict(String),
    #[error("io error: {0}")]
    Io(String),
}
