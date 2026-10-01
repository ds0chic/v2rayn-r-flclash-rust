//! Pure domain models for the v2rayN Flutter+Rust rewrite (T02).
//!
//! This crate carries no Flutter, window or platform APIs. It defines the
//! contract types shared by the application layer, the bridge and net-host:
//!
//! - [`enums`] — upstream enums with their exact numeric values;
//! - [`error`] — stable, front-end-safe error model;
//! - [`revision`] — desired vs. applied revision semantics;
//! - [`job`] — job identity, lifecycle and idempotent cancellation;
//! - [`reference`] — the six upstream reference-expression semantics;
//! - [`profile`] — `ProfileItem` + `ProtoExtra` + `TransportExtra`;
//! - [`entities`] — subscription/routing/DNS/template/stat/migration records;
//! - [`settings`] — the guiNConfig.json `Config` tree skeleton;
//! - [`runtime_plan`] — `RuntimePlan` + `OutboundGraph`/`ProcessGraph`;
//! - [`event`] — the `EventEnvelope` control/telemetry contract.

// `DomainError` is the deliberate shared error contract type. Boxing it would
// ripple through every public signature and the FRB/IPC boundary, so the
// large-Err lint is allowed here with intent.
#![allow(clippy::result_large_err)]

pub mod entities;
pub mod enums;
pub mod error;
pub mod event;
pub mod job;
pub mod profile;
pub mod reference;
pub mod revision;
pub mod runtime_plan;
pub mod settings;
pub mod settings_timing;
pub mod summary;

// Re-export the most-used items at the crate root for ergonomics.
pub use entities::{
    ColumnDefinition, CoreInstallation, CoreTypeBinding, DnsProfile, FullConfigTemplate,
    GlobalHotkey, InboundListener, MigrationRecord, RoutingProfile, RoutingRule, Subscription,
    TaskRecord, TrafficStats, WindowState,
};
pub use enums::{
    ConfigType, CoreType, GirdOrientation, InboundProtocol, MultipleLoad, Network, RuleMode,
    RuleType, Security, SpeedTestAction, SysProxyType,
};
pub use error::{codes, DomainError};
pub use event::{
    EventChannel, EventEnvelope, EventEpoch, EventKind, EventSeq, JobEvent, RuntimeState,
    RuntimeStateChanged,
};
pub use job::{CancelOutcome, CancellationToken, JobId, JobState};
pub use profile::{ExtraMap, Profile, ProtocolExtra, SecurityParams, TransportExtra};
pub use reference::{ReferenceExpr, ReferenceKind, ReferenceSource, SELF_SENTINEL};
pub use revision::{AppliedRevision, DesiredRevision, RevisionPair, RevisionState};
pub use runtime_plan::{
    ConfigSource, ContentHash, NetworkPolicy, OutboundEdge, OutboundGraph, OutboundNode,
    PortRequest, PortTransport, ProcessEdge, ProcessGraph, ProcessNode, RequiredPrivilege,
    RuntimePlan, RuntimeTarget,
};
pub use settings::{
    is_settings_group, AppSettings, ApplyTiming, CheckUpdateItem, ClashUiItem, ConstItem,
    CoreBasicItem, Fragment4RayItem, GrpcItem, GuiItem, HappyEyeballs4RayItem, HysteriaItem,
    KcpItem, MsgUiItem, Mux4RayItem, Mux4SboxItem, RoutingBasicItem, SettingsChange, SimpleDnsItem,
    SpeedTestItem, SystemProxyItem, TunModeItem, UiItem, WebDavItem, CORE_TYPE_CONTROLS,
    SETTINGS_GROUPS,
};
pub use summary::ProfileSummary;

/// The upstream baseline this model set was derived from.
pub const SOURCE_COMMIT: &str = "7d6a967c18c697f28dc6917122ed3a4993fcf336";
/// Upstream release version of the frozen baseline.
pub const UPSTREAM_VERSION: &str = "7.25.4";
