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
/// The TUN adapter is created by the core; the OS interface index is normally
/// discovered after the core starts (tracked as the FIX-13 discovery blocker).
/// These variables let an isolated run provide the adapter/index explicitly so
/// the plan -> helper -> core chain can be exercised without a real adapter.
/// An unset or non-numeric index stays `0`, which the builder rejects loudly.
pub fn tun_hints_from_env() -> TunPlanHints {
    let mut hints = TunPlanHints::default();
    if let Ok(name) = std::env::var("V2RAYN_R_TUN_ADAPTER") {
        if !name.trim().is_empty() {
            hints.adapter_name = name;
        }
    }
    if let Ok(index) = std::env::var("V2RAYN_R_TUN_INTERFACE_INDEX") {
        if let Ok(parsed) = index.trim().parse::<u32>() {
            hints.interface_index = parsed;
        }
    }
    hints
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

/// Build the helper TUN descriptor from settings, or `Ok(None)` when TUN is
/// disabled. Every failure is a structured, fielded error.
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
    let spec = TunSpec {
        kind: TUN_CONFIG_KIND.to_string(),
        adapter_name,
        interface_index: hints.interface_index,
        addresses,
        mtu: Some(mtu),
        routes: hints.routes.clone(),
        route_exclude: item.route_exclude_address.clone().unwrap_or_default(),
    };
    spec.validate()?;
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
    fn attach_rejects_invalid_spec() {
        let mut bad = tun_spec_from_settings(&item(), &hints()).unwrap().unwrap();
        bad.interface_index = 0;
        let mut plan = blank_plan();
        assert!(attach_tun_to_plan(&mut plan, &bad).is_err());
        assert!(!plan.network_policy.tun_enabled);
    }
}
