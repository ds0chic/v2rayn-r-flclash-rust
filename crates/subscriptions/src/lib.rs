//! Share-URI codecs (`Fmt`), subscription content parsing/merging and the
//! download adapter (T09).
//!
//! The behaviour is modelled on the frozen upstream `Handler/Fmt/*` and
//! `Handler/SubscriptionHandler.cs` sources (commit `7d6a967`). The crate is
//! pure: [`parse`], [`merge`] and [`fmt`] perform no I/O, so they can be
//! exercised with synthetic fixtures. Only [`download`] touches the network and
//! it never reads the environment proxy or user subscriptions.
//!
//! Evidence: `docs/evidence/T09.md`; divergences: `docs/decisions/T09-fmt.md`.

#![forbid(unsafe_code)]

pub mod convert;
pub mod download;
pub mod error;
pub mod fmt;
pub mod merge;
pub mod parse;
pub mod util;

pub use convert::{build_convert_url, punycode_url};
pub use download::{
    build_client, download_string, DownloadOptions, Downloaded, Downloader, ProxyConfig,
};
pub use error::{ParseIssue, SubError};
pub use fmt::{
    detect_config_extension, fmt_kind_of, resolve_uri, take_raw_config, to_inner_uri,
    to_inner_uri_with_outbound_loader, to_uri, FmtKind,
};
pub use merge::{
    compare_profile, deduplicate, filter_by_regex, refresh, MergeOptions, MergeResult,
    RefreshOutcome,
};
pub use parse::{parse_content, ContentHint, ParseOptions, ParseResult, ParsedFormat};
pub use util::CancellationWatcher;
