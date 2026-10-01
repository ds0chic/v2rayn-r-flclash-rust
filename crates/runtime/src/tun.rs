//! TUN orchestration plan (T14).
//!
//! A [`RuntimePlan`] already carries `NetworkPolicy::tun_enabled`, but the
//! platform-level work the *privileged helper* must perform (add routes,
//! configure a TUN adapter) is too structured to express as a boolean. This
//! module defines a small, validated TUN descriptor that a plan embeds as a
//! `ProcessGraph` node with id [`TUN_PROCESS_ID`] and the `Tun` privilege.
//!
//! Everything here is pure: it parses/validates the descriptor and converts it
//! into helper IPC payloads. It never touches the OS, the helper or the UI.

use std::net::IpAddr;
use std::str::FromStr;

use domain::runtime_plan::{ConfigSource, RequiredPrivilege};
use domain::{codes, DomainError, RuntimePlan};
use ipc_contract::{
    parse_cidr, validate_route_entries, validate_tun_address, AddressFamily, CidrAddress,
    HelperError, RouteEntry, TunAddressConfig,
};
use serde::{Deserialize, Serialize};

/// Process-node id that carries the TUN descriptor.
pub const TUN_PROCESS_ID: &str = "tun";

/// Marker for the embedded descriptor, so a wrong body is rejected explicitly.
pub const TUN_CONFIG_KIND: &str = "v2rayn.tun.plan.v1";

/// CLI flag that forces a dry-run of the TUN helper work.
pub const TUN_CLI_DRY_RUN: &str = "--dry-run-tun";

/// Environment variable that forces a dry-run of the TUN helper work.
pub const TUN_ENV_DRY_RUN: &str = "V2RAYN_R_DRY_RUN_TUN";

/// Upper bound on the embedded descriptor body (defends the IPC frame budget).
pub const TUN_MAX_SPEC_BYTES: usize = 1024 * 1024;

/// Maximum number of TUN routes accepted in one descriptor (mirrors the helper).
pub const TUN_MAX_ROUTES: usize = ipc_contract::HELPER_MAX_ROUTE_ENTRIES;

/// One address assigned to the TUN adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunAddress {
    pub address: String,
    pub prefix_len: u8,
}

/// One route the helper must add for the TUN session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunRoute {
    /// Destination prefix in CIDR form.
    pub destination: String,
    /// Next hop; its family must match `destination`.
    pub next_hop: String,
    /// Egress interface index (non-zero).
    pub interface_index: u32,
    pub metric: u32,
}

/// Validated TUN descriptor carried by a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunSpec {
    /// Must equal [`TUN_CONFIG_KIND`].
    pub kind: String,
    /// Adapter label (never a path).
    pub adapter_name: String,
    /// Adapter interface index reported by the OS.
    pub interface_index: u32,
    /// Addresses to assign.
    pub addresses: Vec<TunAddress>,
    /// Optional MTU.
    pub mtu: Option<u16>,
    /// Routes the helper must add for the session.
    #[serde(default)]
    pub routes: Vec<TunRoute>,
    /// Prefixes the core must exclude from the TUN route (upstream
    /// `RouteExcludeAddress`). Validated, then left to the config generator.
    #[serde(default)]
    pub route_exclude: Vec<String>,
}

fn invalid_plan(detail: impl Into<String>) -> DomainError {
    DomainError::new(codes::INVALID_PLAN, "error.invalid_plan").with_detail(detail)
}

fn from_helper(error: HelperError) -> DomainError {
    error.to_domain()
}

impl TunSpec {
    /// Validate every field and cross-field constraint.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.kind != TUN_CONFIG_KIND {
            return Err(invalid_plan(format!(
                "tun descriptor kind `{}` is not `{TUN_CONFIG_KIND}`",
                self.kind
            )));
        }
        if self.routes.len() > TUN_MAX_ROUTES {
            return Err(invalid_plan(format!(
                "tun descriptor has {} routes, exceeding {TUN_MAX_ROUTES}",
                self.routes.len()
            )));
        }
        // Reuse the helper contract validators so plan-side and helper-side
        // agree exactly on bounds (adapter, addresses, MTU, route families).
        validate_tun_address(&self.to_tun_address_config()).map_err(from_helper)?;
        if !self.routes.is_empty() {
            let entries = self.to_route_entries()?;
            validate_route_entries(&entries).map_err(from_helper)?;
        }
        for (index, exclude) in self.route_exclude.iter().enumerate() {
            parse_cidr(exclude).map_err(from_helper).map_err(|error| {
                invalid_plan(format!("route_exclude[{index}] is invalid: {error}"))
            })?;
        }
        Ok(())
    }

    /// Convert to the helper's `SetTunAdapterAddress` payload.
    pub fn to_tun_address_config(&self) -> TunAddressConfig {
        TunAddressConfig {
            adapter_name: self.adapter_name.trim().to_string(),
            interface_index: self.interface_index,
            addresses: self
                .addresses
                .iter()
                .map(|address| CidrAddress {
                    address: address.address.trim().to_string(),
                    prefix_len: address.prefix_len,
                })
                .collect(),
            mtu: self.mtu,
        }
    }

    /// Convert to the helper's `AddRoutes` payload, inferring the family.
    pub fn to_route_entries(&self) -> Result<Vec<RouteEntry>, DomainError> {
        let mut entries = Vec::with_capacity(self.routes.len());
        for (index, route) in self.routes.iter().enumerate() {
            let (destination, _) = parse_cidr(&route.destination).map_err(|error| {
                invalid_plan(format!("route[{index}].destination is invalid: {error:?}"))
            })?;
            let next_hop = IpAddr::from_str(route.next_hop.trim()).map_err(|_| {
                invalid_plan(format!("route[{index}].next_hop is not an IP address"))
            })?;
            if AddressFamily::of(&destination) != AddressFamily::of(&next_hop) {
                return Err(invalid_plan(format!(
                    "route[{index}] destination and next_hop families differ"
                )));
            }
            entries.push(RouteEntry {
                destination: route.destination.trim().to_string(),
                next_hop: route.next_hop.trim().to_string(),
                interface_index: route.interface_index,
                metric: route.metric,
                family: AddressFamily::of(&destination),
            });
        }
        Ok(entries)
    }
}

/// Derive the TUN work from a plan.
///
/// Returns `Ok(None)` when the plan does not request TUN. When it does, the
/// plan must carry exactly one valid `tun` descriptor node with the `Tun`
/// privilege; anything else is a structured `E_INVALID_PLAN` rather than a
/// silent skip.
pub fn tun_spec_from_plan(plan: &RuntimePlan) -> Result<Option<TunSpec>, DomainError> {
    let node = plan
        .process_graph
        .nodes
        .iter()
        .find(|node| node.id == TUN_PROCESS_ID);

    if !plan.network_policy.tun_enabled {
        if node.is_some() {
            return Err(invalid_plan(
                "plan carries a TUN descriptor but network_policy.tun_enabled is false",
            ));
        }
        return Ok(None);
    }

    let node = node.ok_or_else(|| {
        invalid_plan("network_policy.tun_enabled is true but the plan has no `tun` process node")
    })?;
    if !node.privileges.contains(&RequiredPrivilege::Tun) {
        return Err(invalid_plan(
            "`tun` process node is missing the `Tun` privilege",
        ));
    }
    let body = match &node.config {
        ConfigSource::Inline { body } => body,
        ConfigSource::ControlledFile { .. } => {
            return Err(invalid_plan(
                "`tun` descriptor must be inline; controlled-file descriptors are unsupported",
            ))
        }
    };
    if body.len() > TUN_MAX_SPEC_BYTES {
        return Err(invalid_plan(format!(
            "`tun` descriptor is {} bytes, exceeding {TUN_MAX_SPEC_BYTES}",
            body.len()
        )));
    }
    let spec: TunSpec = serde_json::from_str(body)
        .map_err(|error| invalid_plan(format!("`tun` descriptor is not valid JSON: {error}")))?;
    spec.validate()?;
    Ok(Some(spec))
}

/// Deterministic, order-insensitive digest of a route set. Used by the
/// recovery journal to re-verify that a cleaned lease is the one recorded.
pub fn route_digest(routes: &[RouteEntry]) -> String {
    let mut parts: Vec<String> = routes
        .iter()
        .map(|route| {
            format!(
                "{}|{}|{}|{}|{:?}",
                route.destination.trim().to_ascii_lowercase(),
                route.next_hop.trim(),
                route.interface_index,
                route.metric,
                route.family
            )
        })
        .collect();
    parts.sort();
    crate::sha256_hex(parts.join("\n").as_bytes())
}

/// Redacted route summary for logs/journals (counts and families only).
pub fn route_summary(routes: &[RouteEntry]) -> String {
    let v4 = routes
        .iter()
        .filter(|route| route.family == AddressFamily::V4)
        .count();
    let v6 = routes.len() - v4;
    format!("routes={} v4={v4} v6={v6}", routes.len())
}

/// Whether a dry-run was requested. Pure so tests can exercise both inputs.
pub fn dry_run_requested(arg_present: bool, env_value: Option<&str>) -> bool {
    arg_present
        || matches!(
            env_value,
            Some(value)
                if !value.is_empty()
                    && value != "0"
                    && !value.eq_ignore_ascii_case("false")
        )
}

/// Read the dry-run switch from process arguments and the environment.
pub fn dry_run_from_env(args: &[String]) -> bool {
    let arg_present = args.iter().any(|arg| arg == TUN_CLI_DRY_RUN);
    dry_run_requested(arg_present, std::env::var(TUN_ENV_DRY_RUN).ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::runtime_plan::{
        ContentHash, NetworkPolicy, OutboundGraph, ProcessGraph, ProcessNode, RuntimeTarget,
    };
    use domain::{CoreType, RuntimePlan};

    fn valid_spec() -> TunSpec {
        TunSpec {
            kind: TUN_CONFIG_KIND.into(),
            adapter_name: "v2rayn-tun".into(),
            interface_index: 7,
            addresses: vec![TunAddress {
                address: "198.18.0.1".into(),
                prefix_len: 16,
            }],
            mtu: Some(1500),
            routes: vec![
                TunRoute {
                    destination: "0.0.0.0/1".into(),
                    next_hop: "198.18.0.1".into(),
                    interface_index: 7,
                    metric: 1,
                },
                TunRoute {
                    destination: "128.0.0.0/1".into(),
                    next_hop: "198.18.0.1".into(),
                    interface_index: 7,
                    metric: 1,
                },
            ],
            route_exclude: vec!["192.168.0.0/16".into()],
        }
    }

    fn plan_with_tun(spec: Option<&TunSpec>, tun_enabled: bool) -> RuntimePlan {
        let mut process_graph = ProcessGraph::default();
        if let Some(spec) = spec {
            process_graph.add_process(ProcessNode {
                id: TUN_PROCESS_ID.into(),
                core_type: CoreType::SingBox,
                config: ConfigSource::Inline {
                    body: serde_json::to_string(spec).unwrap(),
                },
                ports: vec![],
                privileges: vec![RequiredPrivilege::Tun],
            });
        }
        RuntimePlan {
            plan_id: "p-tun".into(),
            desired_revision: 3,
            target: RuntimeTarget {
                core_type: CoreType::SingBox,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("ab"),
            },
            process_graph,
            outbound_graph: OutboundGraph::default(),
            ports: vec![],
            privileges: if tun_enabled {
                vec![RequiredPrivilege::Tun]
            } else {
                vec![]
            },
            network_policy: NetworkPolicy {
                system_proxy: None,
                tun_enabled,
                bypass: vec![],
            },
            resources: vec![],
        }
    }

    #[test]
    fn disabled_plan_has_no_tun_work() {
        let plan = plan_with_tun(None, false);
        assert_eq!(tun_spec_from_plan(&plan).unwrap(), None);
    }

    #[test]
    fn tun_node_without_flag_is_rejected() {
        let spec = valid_spec();
        let plan = plan_with_tun(Some(&spec), false);
        let error = tun_spec_from_plan(&plan).unwrap_err();
        assert_eq!(error.code, codes::INVALID_PLAN);
    }

    #[test]
    fn enabled_plan_requires_a_tun_node() {
        let plan = plan_with_tun(None, true);
        let error = tun_spec_from_plan(&plan).unwrap_err();
        assert_eq!(error.code, codes::INVALID_PLAN);
        assert!(error.detail.unwrap().contains("no `tun` process node"));
    }

    #[test]
    fn tun_node_requires_the_tun_privilege() {
        let spec = valid_spec();
        let mut plan = plan_with_tun(Some(&spec), true);
        plan.process_graph.nodes[0].privileges = vec![RequiredPrivilege::None];
        assert_eq!(
            tun_spec_from_plan(&plan).unwrap_err().code,
            codes::INVALID_PLAN
        );
    }

    #[test]
    fn tun_node_rejects_controlled_file_config() {
        let spec = valid_spec();
        let mut plan = plan_with_tun(Some(&spec), true);
        plan.process_graph.nodes[0].config = ConfigSource::ControlledFile {
            staged_token: "t".into(),
            sha256: ContentHash::new("ab"),
        };
        assert_eq!(
            tun_spec_from_plan(&plan).unwrap_err().code,
            codes::INVALID_PLAN
        );
    }

    #[test]
    fn enabled_plan_parses_a_valid_descriptor() {
        let spec = valid_spec();
        let plan = plan_with_tun(Some(&spec), true);
        let parsed = tun_spec_from_plan(&plan).unwrap().unwrap();
        assert_eq!(parsed, spec);
    }

    #[test]
    fn rejects_wrong_kind() {
        let mut spec = valid_spec();
        spec.kind = "other".into();
        assert_eq!(spec.validate().unwrap_err().code, codes::INVALID_PLAN);
    }

    #[test]
    fn rejects_bad_adapter_name() {
        let mut spec = valid_spec();
        spec.adapter_name = r"..\evil".into();
        assert!(spec.validate().is_err());
        spec.adapter_name = "  ".into();
        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_zero_interface_index() {
        let mut spec = valid_spec();
        spec.interface_index = 0;
        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_bad_address_and_prefix() {
        let mut spec = valid_spec();
        spec.addresses[0].address = "not-an-ip".into();
        assert!(spec.validate().is_err());

        let mut spec = valid_spec();
        spec.addresses[0].prefix_len = 33;
        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_empty_addresses_and_bad_mtu() {
        let mut spec = valid_spec();
        spec.addresses.clear();
        assert!(spec.validate().is_err());

        let mut spec = valid_spec();
        spec.mtu = Some(100);
        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_route_family_mismatch_and_zero_interface() {
        let mut spec = valid_spec();
        spec.routes[0].next_hop = "fd00::1".into();
        assert!(spec.validate().is_err());

        let mut spec = valid_spec();
        spec.routes[0].interface_index = 0;
        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_too_many_routes() {
        let mut spec = valid_spec();
        spec.routes = (0..=TUN_MAX_ROUTES)
            .map(|index| TunRoute {
                destination: "0.0.0.0/0".into(),
                next_hop: "198.18.0.1".into(),
                interface_index: 7,
                metric: (index as u32).min(9_999),
            })
            .collect();
        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_bad_route_exclude() {
        let mut spec = valid_spec();
        spec.route_exclude = vec!["nope".into()];
        assert!(spec.validate().is_err());
    }

    #[test]
    fn converts_to_helper_payloads() {
        let spec = valid_spec();
        let config = spec.to_tun_address_config();
        assert_eq!(config.adapter_name, "v2rayn-tun");
        assert_eq!(config.interface_index, 7);
        assert_eq!(config.addresses.len(), 1);
        assert_eq!(config.mtu, Some(1500));
        let entries = spec.to_route_entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].family, AddressFamily::V4);
    }

    #[test]
    fn route_digest_is_stable_and_order_insensitive() {
        let spec = valid_spec();
        let entries = spec.to_route_entries().unwrap();
        let mut reversed = entries.clone();
        reversed.reverse();
        assert_eq!(route_digest(&entries), route_digest(&reversed));

        let mut changed = entries.clone();
        changed[0].metric = 99;
        assert_ne!(route_digest(&entries), route_digest(&changed));
    }

    #[test]
    fn route_summary_is_counts_only() {
        let spec = valid_spec();
        let entries = spec.to_route_entries().unwrap();
        let summary = route_summary(&entries);
        assert_eq!(summary, "routes=2 v4=2 v6=0");
        // No address text leaks into the summary.
        assert!(!summary.contains("198.18"));
    }

    #[test]
    fn dry_run_detection_reads_arg_and_env() {
        assert!(dry_run_requested(true, None));
        assert!(dry_run_requested(false, Some("1")));
        assert!(dry_run_requested(false, Some("true")));
        assert!(!dry_run_requested(false, Some("0")));
        assert!(!dry_run_requested(false, Some("false")));
        assert!(!dry_run_requested(false, None));
    }
}
