//! TUN descriptor planning (T14 runtime integration).
//!
//! Pure mapping from the persisted `TunModeItem` settings plus
//! operator-provided hints (adapter name, interface index, explicit helper
//! routes) into a validated [`TunSpec`](runtime::tun::TunSpec), and attaching
//! it to a [`RuntimePlan`] so net-host can execute it through the privileged
//! helper. Never touches the OS, the helper or the UI.
//!
//! Mapping notes (upstream `TunModeItem`, compat `fields.settings.yaml`):
//!
//! - `EnableTun=false` means "no TUN work": returns `Ok(None)`, and the plan
//!   must not carry a `tun` node either (net-host rejects the combination).
//! - The core-level TUN section (stack, auto/strict route, ICMP, legacy
//!   protect) stays in the generated core config (`codegen.rs`); this module
//!   only builds the *helper* descriptor: adapter addresses, MTU, explicit
//!   routes and route exclusions.
//! - `IPv4Address`/`IPv6Address` are CIDR strings (`172.18.0.1/30`, the
//!   upstream `Global.TunIPv4Address` default). Bare IPs are rejected with a
//!   fielded error rather than guessed.
//! - `Mtu<=0` falls back to `1280` (upstream `Global.TunMtus.First()`); the
//!   helper contract then enforces `576..=9000`.

use domain::runtime_plan::{ConfigSource, ProcessNode, RequiredPrivilege};
use domain::settings::TunModeItem;
use domain::{codes, DomainError, RuntimePlan};
use runtime::tun::{TunAddress, TunSpec, TUN_CONFIG_KIND, TUN_PROCESS_ID};

/// Default adapter label when the operator did not name one.
pub const DEFAULT_TUN_ADAPTER: &str = "v2rayn-tun";
/// Default adapter address (upstream `Global.TunIPv4Address.First()`).
pub const DEFAULT_TUN_IPV4_CIDR: &str = "172.18.0.1/30";

/// MTU fallback when the setting is unset (`Mtu<=0`, upstream
/// `Global.TunMtus.First()=1280`).
pub const DEFAULT_TUN_MTU_FALLBACK: u16 = 1280;

/// Env overrides for the TUN helper hints during isolated/dry-run runs.
///
/// The OS interface index is discovered from the live adapter name (controlled
/// `netsh interface ipv4 show interfaces` query on Windows) instead of being a
/// mandatory operator variable. `V2RAYN_R_TUN_ADAPTER` selects/relabels the
/// adapter and `V2RAYN_R_TUN_INTERFACE_INDEX` remains an explicit override for
/// isolated runs and tests whose adapter does not exist yet. Discovery is
/// best-effort: an unknown adapter stays `0`, which the builder rejects loudly
/// rather than guessing an index. Real device creation stays a blocked path.
pub fn tun_hints_from_env() -> TunPlanHints {
    let mut hints = TunPlanHints::default();
    if let Ok(name) = std::env::var("V2RAYN_R_TUN_ADAPTER") {
        if !name.trim().is_empty() {
            hints.adapter_name = name;
        }
    }
    let explicit = std::env::var("V2RAYN_R_TUN_INTERFACE_INDEX")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(0);
    hints.interface_index = resolve_interface_index(&hints.adapter_name, explicit);
    hints
}

/// Resolve the interface index: an explicit non-zero override wins, otherwise
/// the adapter is discovered from the OS. `0` means "unknown" and is rejected
/// by [`tun_spec_from_settings`].
pub fn resolve_interface_index(adapter_name: &str, explicit: u32) -> u32 {
    if explicit != 0 {
        return explicit;
    }
    discover_interface_index(adapter_name).unwrap_or(0)
}

/// Query the OS for the index of `adapter_name` (case-insensitive). On Windows
/// this parses the controlled `netsh interface ipv4 show interfaces` output;
/// other platforms return `None` (the helper/backend is Windows-only in T14).
#[cfg(windows)]
pub fn discover_interface_index(adapter_name: &str) -> Option<u32> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("netsh")
        .args(["interface", "ipv4", "show", "interfaces"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_interface_index(&String::from_utf8_lossy(&output.stdout), adapter_name)
}

#[cfg(not(windows))]
pub fn discover_interface_index(_adapter_name: &str) -> Option<u32> {
    None
}

/// Parse `netsh interface ipv4 show interfaces` output for the row whose Name
/// column equals `adapter_name`. Layout: `Idx Met MTU State Name...`; the name
/// may contain spaces. Pure and unit-testable with mock output.
pub fn parse_interface_index(output: &str, adapter_name: &str) -> Option<u32> {
    let want = adapter_name.trim().to_ascii_lowercase();
    if want.is_empty() {
        return None;
    }
    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let Some(index) = parts.next().and_then(|token| token.parse::<u32>().ok()) else {
            continue;
        };
        let rest: Vec<&str> = parts.collect();
        if rest.len() < 4 {
            continue;
        }
        let name = rest[3..].join(" ");
        if name.trim().to_ascii_lowercase() == want {
            return Some(index);
        }
    }
    None
}

/// Operator hints that settings alone cannot provide: the OS interface index
/// (refused when still unknown) and any explicit helper routes. Empty routes
/// mean "adapter address only"; in-core routing (`auto_route`) stays in the
/// generated core config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunPlanHints {
    pub adapter_name: String,
    pub interface_index: u32,
    pub routes: Vec<runtime::tun::TunRoute>,
}

impl Default for TunPlanHints {
    fn default() -> Self {
        Self {
            adapter_name: DEFAULT_TUN_ADAPTER.to_string(),
            interface_index: 0,
            routes: Vec::new(),
        }
    }
}

fn invalid(field: &str, detail: impl Into<String>) -> DomainError {
    DomainError::new(codes::INVALID_ARGUMENT, "error.invalid_tun_setting")
        .with_field(field)
        .with_detail(detail)
}

fn tun_address(field: &str, cidr: &str) -> Result<TunAddress, DomainError> {
    let cidr = cidr.trim();
    let (address, prefix_len) = ipc_contract::parse_cidr(cidr).map_err(|_| {
        invalid(
            field,
            format!("`{cidr}` is not CIDR (expected `address/prefix_len`)"),
        )
    })?;
    Ok(TunAddress {
        address: address.to_string(),
        prefix_len,
    })
}

/// Process-node id that carries a *deferred* TUN descriptor: TUN is enabled but
/// the adapter does not exist yet, so the core's own tun inbound must create it
/// first. net-host discovers the interface after the core starts, then applies
/// the address/routes through the helper (R3-04).
pub const TUN_DEFERRED_PROCESS_ID: &str = "tun-deferred";

/// Assemble the descriptor fields shared by the resolved and deferred paths.
/// The caller decides whether the `interface_index` is final; the deferred path
/// keeps `0` and lets net-host fill it after discovery.
fn build_tun_spec_fields(
    item: &TunModeItem,
    hints: &TunPlanHints,
    interface_index: u32,
) -> Result<TunSpec, DomainError> {
    let adapter_name = if hints.adapter_name.trim().is_empty() {
        DEFAULT_TUN_ADAPTER.to_string()
    } else {
        hints.adapter_name.trim().to_string()
    };
    let v4 = item
        .ipv4_address
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_TUN_IPV4_CIDR);
    let mut addresses = vec![tun_address("IPv4Address", v4)?];
    if item.enable_ipv6_address {
        if let Some(v6) = item
            .ipv6_address
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            addresses.push(tun_address("IPv6Address", v6)?);
        }
    }
    let mtu = if item.mtu <= 0 {
        DEFAULT_TUN_MTU_FALLBACK
    } else if item.mtu > u16::MAX as i32 {
        return Err(invalid("Mtu", format!("MTU {} exceeds u16", item.mtu)));
    } else {
        item.mtu as u16
    };
    Ok(TunSpec {
        kind: TUN_CONFIG_KIND.to_string(),
        adapter_name,
        interface_index,
        addresses,
        mtu: Some(mtu),
        routes: hints.routes.clone(),
        route_exclude: item.route_exclude_address.clone().unwrap_or_default(),
    })
}

/// Build the helper TUN descriptor from settings, or `Ok(None)` when TUN is
/// disabled. Every failure is a structured, fielded error.
///
/// This is the *resolved* path: it requires a known interface index and is used
/// when the adapter already exists. When the index is unknown, the caller uses
/// [`tun_deferred_spec_from_settings`] instead (R3-04).
pub fn tun_spec_from_settings(
    item: &TunModeItem,
    hints: &TunPlanHints,
) -> Result<Option<TunSpec>, DomainError> {
    if !item.enable_tun {
        return Ok(None);
    }
    if hints.interface_index == 0 {
        return Err(invalid(
            "interface_index",
            "TUN interface index is unknown (0); refusing to build a descriptor",
        ));
    }
    let spec = build_tun_spec_fields(item, hints, hints.interface_index)?;
    spec.validate()?;
    Ok(Some(spec))
}

/// Build a *deferred* TUN descriptor for the first-TUN path: TUN is enabled and
/// the adapter does not exist yet, so the plan must not be rejected. The body
/// carries the adapter/addresses/MTU/route-exclude with `interface_index = 0`;
/// net-host discovers the interface (created by the core's tun inbound) and
/// fills it in before the helper runs (R3-04).
///
/// Returns `Ok(None)` when TUN is disabled or the interface is already known
/// (the resolved path applies then).
pub fn tun_deferred_spec_from_settings(
    item: &TunModeItem,
    hints: &TunPlanHints,
) -> Result<Option<TunSpec>, DomainError> {
    if !item.enable_tun || hints.interface_index != 0 {
        return Ok(None);
    }
    let mut spec = build_tun_spec_fields(item, hints, 0)?;
    // Routes are discovered with the same adapter, so their index is pending too.
    for route in &mut spec.routes {
        route.interface_index = 0;
    }
    Ok(Some(spec))
}

/// Attach a validated descriptor to a plan: sets `network_policy.tun_enabled`,
/// ensures the `Tun` privilege, and (re)places the `tun` process node so
/// [`tun_spec_from_plan`](runtime::tun::tun_spec_from_plan) round-trips.
/// Idempotent: attaching twice keeps a single node.
pub fn attach_tun_to_plan(plan: &mut RuntimePlan, spec: &TunSpec) -> Result<(), DomainError> {
    spec.validate()?;
    let body = serde_json::to_string(spec).map_err(|e| {
        DomainError::new(codes::INTERNAL, "error.tun_plan_encode").with_detail(e.to_string())
    })?;
    plan.network_policy.tun_enabled = true;
    if !plan.privileges.contains(&RequiredPrivilege::Tun) {
        plan.privileges.push(RequiredPrivilege::Tun);
    }
    let node = ProcessNode {
        id: TUN_PROCESS_ID.to_string(),
        core_type: plan.target.core_type,
        config: ConfigSource::Inline { body },
        ports: Vec::new(),
        privileges: vec![RequiredPrivilege::Tun],
    };
    match plan
        .process_graph
        .nodes
        .iter_mut()
        .find(|node| node.id == TUN_PROCESS_ID)
    {
        Some(existing) => *existing = node,
        None => plan.process_graph.add_process(node),
    }
    Ok(())
}

/// Attach a *deferred* descriptor to a plan (R3-04): TUN is on, the adapter is
/// not created yet. Uses the distinct [`TUN_DEFERRED_PROCESS_ID`] so the strict
/// [`tun_spec_from_plan`](runtime::tun::tun_spec_from_plan) resolver is not what
/// net-host executes; net-host discovers the interface after the core starts and
/// then applies the filled-in descriptor through the helper.
///
/// `network_policy.tun_enabled` is set so the plan truthfully records TUN, and
/// the `Tun` privilege is requested; the body is serialized without validating
/// the (still zero) interface index.
pub fn attach_deferred_tun_to_plan(
    plan: &mut RuntimePlan,
    spec: &TunSpec,
) -> Result<(), DomainError> {
    let body = serde_json::to_string(spec).map_err(|e| {
        DomainError::new(codes::INTERNAL, "error.tun_plan_encode").with_detail(e.to_string())
    })?;
    plan.network_policy.tun_enabled = true;
    if !plan.privileges.contains(&RequiredPrivilege::Tun) {
        plan.privileges.push(RequiredPrivilege::Tun);
    }
    let node = ProcessNode {
        id: TUN_DEFERRED_PROCESS_ID.to_string(),
        core_type: plan.target.core_type,
        config: ConfigSource::Inline { body },
        ports: Vec::new(),
        privileges: vec![RequiredPrivilege::Tun],
    };
    match plan
        .process_graph
        .nodes
        .iter_mut()
        .find(|node| node.id == TUN_DEFERRED_PROCESS_ID)
    {
        Some(existing) => *existing = node,
        None => plan.process_graph.add_process(node),
    }
    Ok(())
}

/// Ownership key of one applied TUN lease (SP-09 preparation).
///
/// A reopened manager recovers by this key — adapter identity, OS interface
/// index and the order-insensitive route digest — never by the current
/// desired settings. The adapter name compares case-insensitively after
/// trimming (Windows display casing is not identity); the digest is opaque.
/// Pure and side-effect-free; the host journal compares the same fields.
pub fn tun_ownership_key(adapter_name: &str, interface_index: u32, route_digest: &str) -> String {
    format!(
        "{}|{}|{}",
        adapter_name.trim().to_ascii_lowercase(),
        interface_index,
        route_digest.trim(),
    )
}

/// Injected IPv6 capability probe for TUN planning (SP-10, TUN-A06).
///
/// Production assembly today passes no probe (the codegen context stays
/// false/empty); the read-only platform probe that feeds this is registered
/// interface work (RT-12), not silently defaulted here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ipv6Probe {
    /// No probe result yet: never treated as "no IPv6", only as unverified.
    #[default]
    Unknown,
    /// Probed: no global IPv6 on this machine.
    NoGlobal,
    /// Probed: global IPv6 is available.
    HasGlobal,
}

/// Probe inputs the TUN plan consumes beyond persisted settings (SP-10).
/// Defaults to all-unknown; the platform layer injects real probe results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TunProbeContext {
    pub ipv6: Ipv6Probe,
}

/// Note key when IPv6 was requested but the probe is still unknown: coverage
/// is unverified (isolated-environment check pending), never silently false.
pub const TUN_IPV6_UNVERIFIED_NOTE: &str = "tun.ipv6_unverified";

/// Note key when IPv6 was requested but the probe found no global IPv6: the
/// plan refuses to claim IPv6.
pub const TUN_IPV6_NO_GLOBAL_NOTE: &str = "tun.ipv6_no_global";

/// How the requested IPv6 setting resolves against the probe (SP-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv6PlanDecision {
    /// Whether the plan should carry the requested IPv6 address.
    pub include_ipv6: bool,
    /// Readable diagnostic when the decision needs one; `None` means the
    /// probe silently agrees with the request.
    pub note: Option<&'static str>,
}

/// Resolve the requested IPv6 setting against the injected probe (SP-10).
///
/// An explicit user address is honored under `Unknown` (with an unverified
/// diagnostic, never a silent drop) and refused under `NoGlobal` (with a
/// diagnostic, never a false claim). Disabled or address-less IPv6 stays
/// excluded with no note. Pure and side-effect-free; the codegen caller
/// wiring (which today passes no probe) is SP-00 integrator work.
pub fn resolve_ipv6_for_plan(item: &TunModeItem, ctx: &TunProbeContext) -> Ipv6PlanDecision {
    let requested = item.enable_ipv6_address
        && item
            .ipv6_address
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    if !requested {
        return Ipv6PlanDecision {
            include_ipv6: false,
            note: None,
        };
    }
    match ctx.ipv6 {
        Ipv6Probe::HasGlobal => Ipv6PlanDecision {
            include_ipv6: true,
            note: None,
        },
        Ipv6Probe::Unknown => Ipv6PlanDecision {
            include_ipv6: true,
            note: Some(TUN_IPV6_UNVERIFIED_NOTE),
        },
        Ipv6Probe::NoGlobal => Ipv6PlanDecision {
            include_ipv6: false,
            note: Some(TUN_IPV6_NO_GLOBAL_NOTE),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::runtime_plan::{
        ContentHash, NetworkPolicy, OutboundGraph, ProcessGraph, RuntimeTarget,
    };
    use domain::{CoreType, RuntimePlan};
    use runtime::tun::{tun_spec_from_plan, TunRoute};

    fn item() -> TunModeItem {
        TunModeItem {
            enable_tun: true,
            mtu: 9000,
            ipv4_address: Some("172.18.0.1/30".into()),
            ..TunModeItem::default()
        }
    }

    fn hints() -> TunPlanHints {
        TunPlanHints {
            adapter_name: "v2rayn-tun".into(),
            interface_index: 9,
            routes: vec![TunRoute {
                destination: "0.0.0.0/0".into(),
                next_hop: "172.18.0.1".into(),
                interface_index: 9,
                metric: 1,
            }],
        }
    }

    fn blank_plan() -> RuntimePlan {
        RuntimePlan {
            plan_id: "p-tun".into(),
            desired_revision: 3,
            target: RuntimeTarget {
                core_type: CoreType::SingBox,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("ab"),
            },
            process_graph: ProcessGraph::default(),
            outbound_graph: OutboundGraph::default(),
            ports: vec![],
            privileges: vec![],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        }
    }

    #[test]
    fn disabled_tun_builds_nothing() {
        let mut off = item();
        off.enable_tun = false;
        assert_eq!(tun_spec_from_settings(&off, &hints()).unwrap(), None);
    }

    #[test]
    fn explicit_ipv4_cidr_is_used() {
        let spec = tun_spec_from_settings(&item(), &hints()).unwrap().unwrap();
        assert_eq!(spec.addresses.len(), 1);
        assert_eq!(spec.addresses[0].address, "172.18.0.1");
        assert_eq!(spec.addresses[0].prefix_len, 30);
        assert_eq!(spec.mtu, Some(9000));
        assert_eq!(spec.interface_index, 9);
    }

    #[test]
    fn missing_ipv4_falls_back_to_upstream_default() {
        let mut unset = item();
        unset.ipv4_address = None;
        let spec = tun_spec_from_settings(&unset, &hints()).unwrap().unwrap();
        assert_eq!(spec.addresses[0].address, "172.18.0.1");
        assert_eq!(spec.addresses[0].prefix_len, 30);
    }

    #[test]
    fn bare_ip_is_rejected_with_field() {
        let mut bare = item();
        bare.ipv4_address = Some("172.18.0.1".into());
        let error = tun_spec_from_settings(&bare, &hints()).unwrap_err();
        assert_eq!(error.code, codes::INVALID_ARGUMENT);
        assert_eq!(error.field_path.as_deref(), Some("IPv4Address"));
    }

    #[test]
    fn zero_interface_index_is_rejected() {
        let mut no_if = hints();
        no_if.interface_index = 0;
        let error = tun_spec_from_settings(&item(), &no_if).unwrap_err();
        assert_eq!(error.code, codes::INVALID_ARGUMENT);
        assert_eq!(error.field_path.as_deref(), Some("interface_index"));
    }

    #[test]
    fn nonpositive_mtu_falls_back_to_1280() {
        for mtu in [0, -5] {
            let mut fallback = item();
            fallback.mtu = mtu;
            let spec = tun_spec_from_settings(&fallback, &hints())
                .unwrap()
                .unwrap();
            assert_eq!(spec.mtu, Some(1280));
        }
    }

    #[test]
    fn out_of_range_mtu_is_rejected() {
        let mut bad = item();
        bad.mtu = 100;
        assert!(tun_spec_from_settings(&bad, &hints()).is_err());
    }

    #[test]
    fn ipv6_included_only_when_enabled_with_address() {
        let mut both = item();
        both.enable_ipv6_address = true;
        both.ipv6_address = Some("fd00::1/64".into());
        let spec = tun_spec_from_settings(&both, &hints()).unwrap().unwrap();
        assert_eq!(spec.addresses.len(), 2);

        let mut enabled_no_addr = item();
        enabled_no_addr.enable_ipv6_address = true;
        let spec = tun_spec_from_settings(&enabled_no_addr, &hints())
            .unwrap()
            .unwrap();
        assert_eq!(spec.addresses.len(), 1);

        let mut disabled_with_addr = item();
        disabled_with_addr.ipv6_address = Some("fd00::1/64".into());
        let spec = tun_spec_from_settings(&disabled_with_addr, &hints())
            .unwrap()
            .unwrap();
        assert_eq!(spec.addresses.len(), 1);
    }

    #[test]
    fn bad_route_exclude_is_rejected() {
        let mut bad = item();
        bad.route_exclude_address = Some(vec!["nope".into()]);
        assert!(tun_spec_from_settings(&bad, &hints()).is_err());
    }

    #[test]
    fn attach_round_trips_through_plan_and_is_idempotent() {
        let spec = tun_spec_from_settings(&item(), &hints()).unwrap().unwrap();
        let mut plan = blank_plan();
        attach_tun_to_plan(&mut plan, &spec).unwrap();
        assert!(plan.network_policy.tun_enabled);
        assert!(plan.privileges.contains(&RequiredPrivilege::Tun));
        plan.validate().unwrap();
        let parsed = tun_spec_from_plan(&plan).unwrap().unwrap();
        assert_eq!(parsed, spec);
        attach_tun_to_plan(&mut plan, &spec).unwrap();
        assert_eq!(
            plan.process_graph
                .nodes
                .iter()
                .filter(|node| node.id == TUN_PROCESS_ID)
                .count(),
            1
        );
        assert_eq!(
            plan.privileges
                .iter()
                .filter(|privilege| **privilege == RequiredPrivilege::Tun)
                .count(),
            1
        );
    }

    #[test]
    fn deferred_spec_used_when_interface_unknown() {
        let mut zero = hints();
        zero.interface_index = 0;
        let spec = tun_deferred_spec_from_settings(&item(), &zero)
            .unwrap()
            .unwrap();
        assert_eq!(spec.interface_index, 0, "deferred index is pending");
        assert_eq!(spec.adapter_name, "v2rayn-tun");
        assert_eq!(spec.kind, TUN_CONFIG_KIND);
        // The strict resolved resolver still refuses an explicit zero index.
        assert!(tun_spec_from_settings(&item(), &zero).is_err());
    }

    #[test]
    fn deferred_spec_is_none_when_disabled_or_index_known() {
        let mut off = item();
        off.enable_tun = false;
        let mut zero = hints();
        zero.interface_index = 0;
        assert_eq!(tun_deferred_spec_from_settings(&off, &zero).unwrap(), None);
        assert_eq!(
            tun_deferred_spec_from_settings(&item(), &hints()).unwrap(),
            None
        );
    }

    #[test]
    fn deferred_attach_uses_distinct_node_and_flag() {
        let mut zero = hints();
        zero.interface_index = 0;
        let spec = tun_deferred_spec_from_settings(&item(), &zero)
            .unwrap()
            .unwrap();
        let mut plan = blank_plan();
        attach_deferred_tun_to_plan(&mut plan, &spec).unwrap();
        assert!(plan.network_policy.tun_enabled);
        assert!(plan.privileges.contains(&RequiredPrivilege::Tun));
        assert!(plan
            .process_graph
            .nodes
            .iter()
            .any(|node| node.id == TUN_DEFERRED_PROCESS_ID));
        assert!(!plan
            .process_graph
            .nodes
            .iter()
            .any(|node| node.id == TUN_PROCESS_ID));
        plan.validate().unwrap();
        // A deferred descriptor is not a resolved one: the strict resolver
        // must not treat it as ready work.
        assert!(tun_spec_from_plan(&plan).is_err());
    }

    #[test]
    fn attach_rejects_invalid_spec() {
        let mut bad = tun_spec_from_settings(&item(), &hints()).unwrap().unwrap();
        bad.interface_index = 0;
        let mut plan = blank_plan();
        assert!(attach_tun_to_plan(&mut plan, &bad).is_err());
        assert!(!plan.network_policy.tun_enabled);
    }

    /// Real `netsh interface ipv4 show interfaces` shape (English Win11).
    const NETSH_SAMPLE: &str = "\r\n\
Idx     Met    MTU          State                Name\r\n\
---  ----------  ----------  ------------  ---------------------------\r\n\
  1          75  4294967295  connected     Loopback Pseudo-Interface 1\r\n\
  7          35  1500        disconnected  Wi-Fi\r\n\
  9          25  1500        connected     v2rayn-tun\r\n";

    #[test]
    fn parse_finds_adapter_by_name_and_skips_header() {
        assert_eq!(parse_interface_index(NETSH_SAMPLE, "v2rayn-tun"), Some(9));
        assert_eq!(parse_interface_index(NETSH_SAMPLE, "V2RAYN-TUN"), Some(9));
        assert_eq!(parse_interface_index(NETSH_SAMPLE, "Wi-Fi"), Some(7));
    }

    #[test]
    fn parse_missing_or_blank_adapter_is_none() {
        assert_eq!(parse_interface_index(NETSH_SAMPLE, "nope"), None);
        assert_eq!(parse_interface_index(NETSH_SAMPLE, ""), None);
        assert_eq!(parse_interface_index("garbage\nnot a table", "x"), None);
    }

    #[test]
    fn parse_handles_adapter_names_with_spaces() {
        let sample = "  4   10   1500  connected  My TUN Adapter Name\r\n";
        assert_eq!(
            parse_interface_index(sample, "my tun adapter name"),
            Some(4)
        );
    }

    #[test]
    fn explicit_index_wins_over_discovery() {
        assert_eq!(resolve_interface_index("v2rayn-tun", 42), 42);
    }

    #[test]
    fn ownership_key_is_stable_across_casing_and_spacing() {
        assert_eq!(
            tun_ownership_key("v2rayn-tun", 9, "abc123"),
            tun_ownership_key("  V2RAYN-TUN ", 9, "abc123"),
        );
    }

    #[test]
    fn ownership_key_separates_index_and_digest() {
        let base = tun_ownership_key("v2rayn-tun", 9, "abc123");
        assert_ne!(base, tun_ownership_key("v2rayn-tun", 11, "abc123"));
        assert_ne!(base, tun_ownership_key("v2rayn-tun", 9, "def456"));
        assert_ne!(base, tun_ownership_key("other-tun", 9, "abc123"));
    }

    // -- SP-10 injectable IPv6 probe (CP-12 / TUN-A06) ------------------------
    //
    // Red contract: the TUN plan consumes an explicit IPv6 capability probe
    // instead of a hardcoded false/empty context. Unknown is never silently
    // false (diagnostic note, coverage unverified); NoGlobal refuses to
    // claim IPv6 (diagnostic note); only a positive probe includes IPv6
    // silently. Disabled IPv6 stays excluded with no note.

    #[test]
    fn sp10_ipv6_probe_decision_matrix() {
        let mut want_v6 = item();
        want_v6.enable_ipv6_address = true;
        want_v6.ipv6_address = Some("fd00::1/64".into());

        let decided = resolve_ipv6_for_plan(
            &want_v6,
            &TunProbeContext {
                ipv6: Ipv6Probe::HasGlobal,
            },
        );
        assert!(decided.include_ipv6);
        assert!(decided.note.is_none());

        let decided = resolve_ipv6_for_plan(&want_v6, &TunProbeContext::default());
        assert!(
            decided.include_ipv6,
            "an explicit user request is honored, not silently dropped"
        );
        assert_eq!(decided.note, Some(TUN_IPV6_UNVERIFIED_NOTE));

        let decided = resolve_ipv6_for_plan(
            &want_v6,
            &TunProbeContext {
                ipv6: Ipv6Probe::NoGlobal,
            },
        );
        assert!(!decided.include_ipv6, "no global IPv6 must not claim IPv6");
        assert_eq!(decided.note, Some(TUN_IPV6_NO_GLOBAL_NOTE));

        let mut off = item();
        off.enable_ipv6_address = false;
        off.ipv6_address = Some("fd00::1/64".into());
        let decided = resolve_ipv6_for_plan(&off, &TunProbeContext::default());
        assert!(!decided.include_ipv6);
        assert!(decided.note.is_none());

        let mut enabled_no_addr = item();
        enabled_no_addr.enable_ipv6_address = true;
        enabled_no_addr.ipv6_address = None;
        let decided = resolve_ipv6_for_plan(&enabled_no_addr, &TunProbeContext::default());
        assert!(!decided.include_ipv6);
        assert!(decided.note.is_none());
    }
}
