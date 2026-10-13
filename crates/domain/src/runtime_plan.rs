//! Runtime plan and its dependency graphs (plan §12).
//!
//! net-host receives an immutable [`RuntimePlan`]: the target core + version,
//! controlled config reference (never a raw arbitrary path), required ports,
//! dependency topology, required privileges and network policy. The plan also
//! carries the compiled [`OutboundGraph`] (in-core inbound/detour references,
//! e.g. Xray `dialerProxy` / sing-box `detour`) and the [`ProcessGraph`]
//! (separate core processes, e.g. TUN helper cores).
//!
//! Validation detects cycles, port conflicts and dangling references and
//! returns structured [`DomainError`]s with stable codes.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::enums::CoreType;
use crate::error::{codes, DomainError};

/// Hash of a generated config/resource, hex encoded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(pub String);

impl ContentHash {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether a config is inline content or a controlled file reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigSource {
    /// Inline JSON/YAML body. net-host copies it into its private run dir.
    Inline { body: String },
    /// Reference to a file previously staged by AppEngine, by content hash.
    /// net-host verifies the hash and the path boundary before use.
    ControlledFile {
        /// Opaque staging token; never a caller-supplied absolute path.
        staged_token: String,
        sha256: ContentHash,
    },
}

/// A port the plan requires, with its transport and owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortRequest {
    pub port: u16,
    /// `tcp` / `udp` / `both`.
    pub transport: PortTransport,
    /// Node id that owns the port (for conflict reporting).
    pub owner: String,
    /// Whether the conflict must hard-fail the plan.
    pub exclusive: bool,
}

impl PortRequest {
    pub fn tcp(port: u16, owner: impl Into<String>) -> Self {
        Self {
            port,
            transport: PortTransport::Tcp,
            owner: owner.into(),
            exclusive: true,
        }
    }

    pub fn udp(port: u16, owner: impl Into<String>) -> Self {
        Self {
            port,
            transport: PortTransport::Udp,
            owner: owner.into(),
            exclusive: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortTransport {
    Tcp,
    Udp,
    Both,
}

impl PortTransport {
    pub fn overlaps(self, other: PortTransport) -> bool {
        self == other || self == PortTransport::Both || other == PortTransport::Both
    }
}

/// OS-level privileges the plan needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredPrivilege {
    /// No elevation; run in the normal user session.
    None,
    /// Create a TUN interface / change routes.
    Tun,
    /// Write system proxy settings for the current user.
    SystemProxy,
    /// Bind a privileged port (< 1024).
    PrivilegedPort,
}

/// Network policy the plan applies while running.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NetworkPolicy {
    /// System proxy mode the plan intends to set; `None` = leave untouched.
    pub system_proxy: Option<crate::enums::SysProxyType>,
    /// Whether the plan may create/modify a TUN device.
    pub tun_enabled: bool,
    /// Bypass list applied to the system proxy, if any.
    pub bypass: Vec<String>,
}

/// One node of the in-core outbound graph.
///
/// Nodes are referenced by `tag`; edges express a detour/dialer relation
/// (`from` dials through `to`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundNode {
    /// Unique outbound tag in the generated config.
    pub tag: String,
    /// Source profile id, when this outbound came from a profile.
    pub profile_id: Option<String>,
    /// In-core protocol token (`vless`, `vmess`, `direct`, ...).
    pub protocol: String,
}

/// A directed edge in the outbound graph: `from` detours through `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundEdge {
    pub from: String,
    pub to: String,
}

/// In-core outbound reference graph with cycle/dangling validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OutboundGraph {
    pub nodes: Vec<OutboundNode>,
    pub edges: Vec<OutboundEdge>,
}

impl OutboundGraph {
    pub fn add_node(&mut self, node: OutboundNode) {
        self.nodes.push(node);
    }

    pub fn connect(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.edges.push(OutboundEdge {
            from: from.into(),
            to: to.into(),
        });
    }

    fn tags(&self) -> BTreeSet<&str> {
        self.nodes.iter().map(|n| n.tag.as_str()).collect()
    }

    /// Detect a cycle; `Ok(())` when the graph is a DAG.
    pub fn validate_acyclic(&self) -> Result<(), DomainError> {
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            adj.entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
        }
        let mut visiting: HashSet<&str> = HashSet::new();
        let mut done: HashSet<&str> = HashSet::new();

        fn dfs<'a>(
            node: &'a str,
            adj: &HashMap<&'a str, Vec<&'a str>>,
            visiting: &mut HashSet<&'a str>,
            done: &mut HashSet<&'a str>,
            path: &mut Vec<&'a str>,
        ) -> Option<Vec<String>> {
            if done.contains(node) {
                return None;
            }
            if !visiting.insert(node) {
                let start = path.iter().position(|n| *n == node).unwrap_or(0);
                let mut cycle: Vec<String> = path[start..].iter().map(|s| s.to_string()).collect();
                cycle.push(node.to_string());
                return Some(cycle);
            }
            path.push(node);
            if let Some(neighbors) = adj.get(node) {
                for next in neighbors {
                    if let Some(cycle) = dfs(next, adj, visiting, done, path) {
                        return Some(cycle);
                    }
                }
            }
            path.pop();
            visiting.remove(node);
            done.insert(node);
            None
        }

        let mut sorted: Vec<&str> = adj.keys().copied().collect();
        sorted.sort_unstable();
        for node in sorted {
            let mut path = Vec::new();
            if let Some(cycle) = dfs(node, &adj, &mut visiting, &mut done, &mut path) {
                return Err(DomainError::new(codes::GRAPH_CYCLE, "error.graph_cycle")
                    .with_detail(format!("outbound cycle: {}", cycle.join(" -> "))));
            }
        }
        Ok(())
    }

    /// Detect edges pointing at a tag that has no node.
    pub fn validate_references(&self) -> Result<(), DomainError> {
        let tags = self.tags();
        for edge in &self.edges {
            if !tags.contains(edge.from.as_str()) {
                return Err(DomainError::new(
                    codes::DANGLING_REFERENCE,
                    "error.dangling_reference",
                )
                .with_field("outbound")
                .with_detail(format!("edge source `{}` has no node", edge.from)));
            }
            if !tags.contains(edge.to.as_str()) {
                return Err(DomainError::new(
                    codes::DANGLING_REFERENCE,
                    "error.dangling_reference",
                )
                .with_field("outbound")
                .with_detail(format!("edge target `{}` has no node", edge.to)));
            }
        }
        Ok(())
    }

    /// Duplicate-tag detection.
    pub fn validate_unique_tags(&self) -> Result<(), DomainError> {
        let mut seen = HashSet::new();
        for node in &self.nodes {
            if !seen.insert(node.tag.as_str()) {
                return Err(DomainError::new(codes::INVALID_PLAN, "error.duplicate_tag")
                    .with_field("outbound")
                    .with_detail(format!("duplicate outbound tag `{}`", node.tag)));
            }
        }
        Ok(())
    }
}

/// One process in the process graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessNode {
    /// Stable process id (core tag).
    pub id: String,
    pub core_type: CoreType,
    /// Config source for this process.
    pub config: ConfigSource,
    /// Ports this process binds.
    pub ports: Vec<PortRequest>,
    /// Privileges this process requires.
    pub privileges: Vec<RequiredPrivilege>,
}

/// A start-order dependency between processes: `before` must be up first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEdge {
    pub before: String,
    pub after: String,
}

/// The multi-process topology. Cycles are rejected; start order is a
/// topological order, stop order is its reverse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProcessGraph {
    pub nodes: Vec<ProcessNode>,
    pub edges: Vec<ProcessEdge>,
}

impl ProcessGraph {
    pub fn add_process(&mut self, node: ProcessNode) {
        self.nodes.push(node);
    }

    pub fn depends_on(&mut self, after: impl Into<String>, before: impl Into<String>) {
        self.edges.push(ProcessEdge {
            before: before.into(),
            after: after.into(),
        });
    }

    pub fn ids(&self) -> BTreeSet<&str> {
        self.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    /// Build a stable topological start order. Errors on cycles/duplicates.
    pub fn start_order(&self) -> Result<Vec<String>, DomainError> {
        let mut indegree: BTreeMap<&str, usize> = BTreeMap::new();
        let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for node in &self.nodes {
            indegree.entry(node.id.as_str()).or_insert(0);
            adj.entry(node.id.as_str()).or_default();
        }
        // duplicate id check
        if self.nodes.len() != indegree.len() {
            return Err(
                DomainError::new(codes::INVALID_PLAN, "error.duplicate_process")
                    .with_field("process"),
            );
        }
        for edge in &self.edges {
            if !indegree.contains_key(edge.before.as_str())
                || !indegree.contains_key(edge.after.as_str())
            {
                return Err(DomainError::new(
                    codes::DANGLING_REFERENCE,
                    "error.dangling_reference",
                )
                .with_field("process")
                .with_detail(format!(
                    "edge {}->{} references unknown process",
                    edge.before, edge.after
                )));
            }
            adj.entry(edge.before.as_str())
                .or_default()
                .push(edge.after.as_str());
            *indegree.get_mut(edge.after.as_str()).unwrap() += 1;
        }

        // Ordered set: always start the smallest ready id, so the order is
        // stable regardless of edge insertion order.
        let mut ready: BTreeSet<&str> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = ready.pop_first() {
            order.push(id.to_string());
            for next in adj.get(id).into_iter().flatten() {
                let d = indegree.get_mut(next).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.insert(next);
                }
            }
        }

        if order.len() != self.nodes.len() {
            let stuck: Vec<String> = indegree
                .iter()
                .filter(|(_, d)| **d > 0)
                .map(|(id, _)| (*id).to_string())
                .collect();
            return Err(DomainError::new(codes::GRAPH_CYCLE, "error.graph_cycle")
                .with_field("process")
                .with_detail(format!("cyclic processes: {}", stuck.join(", "))));
        }
        Ok(order)
    }

    /// Detect ports claimed by more than one exclusive owner.
    ///
    /// A conflict exists when two ports share the same number and their
    /// transport protocols overlap (`Both` overlaps `Tcp` and `Udp`). A
    /// repeated claim by the same owner counts too: a duplicate declaration
    /// is a caller error, not sharing.
    pub fn validate_ports(&self) -> Result<(), DomainError> {
        let mut claimed: Vec<(&str, &PortRequest)> = Vec::new();
        for node in &self.nodes {
            for port in &node.ports {
                if !port.exclusive {
                    continue;
                }
                for (prev_owner, prev) in &claimed {
                    if prev.port == port.port && prev.transport.overlaps(port.transport) {
                        return Err(
                            DomainError::new(codes::PORT_CONFLICT, "error.port_conflict")
                                .with_field("port")
                                .with_detail(format!(
                                    "port {} ({:?}) claimed by both `{}` and `{}`",
                                    port.port, port.transport, prev_owner, node.id
                                )),
                        );
                    }
                }
                claimed.push((node.id.as_str(), port));
            }
        }
        Ok(())
    }
}

/// Target kernel and version a plan runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeTarget {
    pub core_type: CoreType,
    pub version: Option<String>,
    /// Config for the primary core.
    pub config: ConfigSource,
    pub config_sha256: ContentHash,
}

/// The immutable runtime plan handed to net-host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimePlan {
    /// Plan identity, stable across retries of the same operation.
    pub plan_id: String,
    /// Desired revision this plan realizes.
    pub desired_revision: u64,
    pub target: RuntimeTarget,
    /// Dependency topology for multi-core runs.
    pub process_graph: ProcessGraph,
    /// In-core outbound reference graph.
    pub outbound_graph: OutboundGraph,
    /// All ports the plan requires, including inline target ports.
    pub ports: Vec<PortRequest>,
    /// Privileges the plan requires (union of process privileges).
    pub privileges: Vec<RequiredPrivilege>,
    pub network_policy: NetworkPolicy,
    /// Additional controlled resources (rule sets, geo files) by hash.
    pub resources: Vec<ContentHash>,
}

/// Upper bound for an inline plan config body (16 MiB). Generated core
/// configs are kilobytes; anything larger is a caller error, not a config.
pub const MAX_INLINE_CONFIG_BYTES: usize = 16 * 1024 * 1024;

impl RuntimePlan {
    /// Full structural validation. Returns the first structured error.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.validate_identity()?;
        self.validate_configs()?;
        self.outbound_graph.validate_unique_tags()?;
        self.outbound_graph.validate_references()?;
        self.outbound_graph.validate_acyclic()?;
        self.process_graph.start_order()?;
        self.process_graph.validate_ports()?;
        self.validate_port_set()?;
        self.validate_privileges()?;
        Ok(())
    }

    /// The plan must carry a non-empty identity (stable across retries of
    /// the same operation).
    fn validate_identity(&self) -> Result<(), DomainError> {
        if self.plan_id.trim().is_empty() {
            return Err(DomainError::new(codes::INVALID_PLAN, "error.invalid_plan")
                .with_field("plan_id")
                .with_detail("plan_id must not be empty"));
        }
        Ok(())
    }

    /// Every config source must be resolvable by net-host: a `ControlledFile`
    /// needs its staging token and content hash, an `Inline` body must exist
    /// and stay within a sane bound (16 MiB; generated configs are kilobytes).
    fn validate_configs(&self) -> Result<(), DomainError> {
        Self::validate_config_source("target.config", &self.target.config)?;
        for node in &self.process_graph.nodes {
            Self::validate_config_source(&format!("process.{}.config", node.id), &node.config)?;
        }
        Ok(())
    }

    fn validate_config_source(field: &str, source: &ConfigSource) -> Result<(), DomainError> {
        match source {
            ConfigSource::ControlledFile {
                staged_token,
                sha256,
            } => {
                if staged_token.trim().is_empty() {
                    return Err(DomainError::new(codes::INVALID_PLAN, "error.invalid_plan")
                        .with_field(field)
                        .with_detail("controlled file staged_token must not be empty"));
                }
                if sha256.as_str().trim().is_empty() {
                    return Err(DomainError::new(codes::INVALID_PLAN, "error.invalid_plan")
                        .with_field(field)
                        .with_detail("controlled file sha256 must not be empty"));
                }
            }
            ConfigSource::Inline { body } => {
                if body.trim().is_empty() {
                    return Err(DomainError::new(codes::INVALID_PLAN, "error.invalid_plan")
                        .with_field(field)
                        .with_detail("inline config body must not be empty"));
                }
                if body.len() > MAX_INLINE_CONFIG_BYTES {
                    return Err(DomainError::new(codes::INVALID_PLAN, "error.invalid_plan")
                        .with_field(field)
                        .with_detail(format!(
                            "inline config body exceeds {MAX_INLINE_CONFIG_BYTES} bytes"
                        )));
                }
            }
        }
        Ok(())
    }

    /// Privilege/port consistency: `PrivilegedPort` requires a <1024 port
    /// somewhere in the plan, and `Tun` requires `tun_enabled` in the
    /// network policy. Union of plan-level and per-process privileges.
    fn validate_privileges(&self) -> Result<(), DomainError> {
        let wants_privileged = self.privileges.contains(&RequiredPrivilege::PrivilegedPort)
            || self
                .process_graph
                .nodes
                .iter()
                .any(|n| n.privileges.contains(&RequiredPrivilege::PrivilegedPort));
        if wants_privileged {
            let has_low_port = self
                .ports
                .iter()
                .chain(self.process_graph.nodes.iter().flat_map(|n| n.ports.iter()))
                .any(|p| p.port < 1024);
            if !has_low_port {
                return Err(DomainError::new(codes::INVALID_PLAN, "error.invalid_plan")
                    .with_field("privileges")
                    .with_detail("PrivilegedPort requires a port below 1024"));
            }
        }
        let wants_tun = self.privileges.contains(&RequiredPrivilege::Tun)
            || self
                .process_graph
                .nodes
                .iter()
                .any(|n| n.privileges.contains(&RequiredPrivilege::Tun));
        if wants_tun && !self.network_policy.tun_enabled {
            return Err(DomainError::new(codes::INVALID_PLAN, "error.invalid_plan")
                .with_field("network_policy.tun_enabled")
                .with_detail("Tun privilege requires tun_enabled"));
        }
        Ok(())
    }

    /// Validate the plan-level port list for overlaps between exclusive
    /// owners (a repeated claim by the same owner counts as a conflict).
    pub fn validate_port_set(&self) -> Result<(), DomainError> {
        let mut claimed: Vec<(&str, &PortRequest)> = Vec::new();
        for port in &self.ports {
            if !port.exclusive {
                continue;
            }
            for (prev_owner, prev) in &claimed {
                if prev.port == port.port && prev.transport.overlaps(port.transport) {
                    return Err(
                        DomainError::new(codes::PORT_CONFLICT, "error.port_conflict")
                            .with_field("port")
                            .with_detail(format!(
                                "port {} claimed by both `{}` and `{}`",
                                port.port, prev_owner, port.owner
                            )),
                    );
                }
            }
            claimed.push((port.owner.as_str(), port));
        }
        Ok(())
    }

    /// Stop order is the reverse of the start order.
    pub fn stop_order(&self) -> Result<Vec<String>, DomainError> {
        let mut order = self.process_graph.start_order()?;
        order.reverse();
        Ok(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(tag: &str) -> OutboundNode {
        OutboundNode {
            tag: tag.into(),
            profile_id: None,
            protocol: "vless".into(),
        }
    }

    #[test]
    fn outbound_cycle_is_detected() {
        let mut g = OutboundGraph::default();
        g.add_node(node("a"));
        g.add_node(node("b"));
        g.connect("a", "b");
        g.connect("b", "a");
        assert_eq!(g.validate_acyclic().unwrap_err().code, codes::GRAPH_CYCLE);
    }

    #[test]
    fn dangling_edge_is_detected() {
        let mut g = OutboundGraph::default();
        g.add_node(node("a"));
        g.connect("a", "ghost");
        assert_eq!(
            g.validate_references().unwrap_err().code,
            codes::DANGLING_REFERENCE
        );
    }

    #[test]
    fn duplicate_tag_is_detected() {
        let mut g = OutboundGraph::default();
        g.add_node(node("a"));
        g.add_node(node("a"));
        assert_eq!(
            g.validate_unique_tags().unwrap_err().code,
            codes::INVALID_PLAN
        );
    }

    #[test]
    fn process_topological_order_and_reverse() {
        let mut g = ProcessGraph::default();
        for id in ["tun", "app", "proxy"] {
            g.add_process(ProcessNode {
                id: id.into(),
                core_type: CoreType::Xray,
                config: ConfigSource::Inline { body: "{}".into() },
                ports: vec![],
                privileges: vec![RequiredPrivilege::None],
            });
        }
        // `depends_on(after, before)` -> `before` must start first.
        g.depends_on("app", "proxy");
        g.depends_on("tun", "app");
        assert_eq!(g.start_order().unwrap(), vec!["proxy", "app", "tun"]);
        let plan = RuntimePlan {
            plan_id: "p".into(),
            desired_revision: 1,
            target: RuntimeTarget {
                core_type: CoreType::Xray,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("00"),
            },
            process_graph: g,
            outbound_graph: OutboundGraph::default(),
            ports: vec![],
            privileges: vec![RequiredPrivilege::None],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        };
        assert_eq!(plan.stop_order().unwrap(), vec!["tun", "app", "proxy"]);
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn process_cycle_is_detected() {
        let mut g = ProcessGraph::default();
        for id in ["a", "b"] {
            g.add_process(ProcessNode {
                id: id.into(),
                core_type: CoreType::Xray,
                config: ConfigSource::Inline { body: "{}".into() },
                ports: vec![],
                privileges: vec![],
            });
        }
        g.depends_on("a", "b");
        g.depends_on("b", "a");
        assert_eq!(g.start_order().unwrap_err().code, codes::GRAPH_CYCLE);
    }

    #[test]
    fn port_conflict_is_detected() {
        let mut g = ProcessGraph::default();
        g.add_process(ProcessNode {
            id: "a".into(),
            core_type: CoreType::Xray,
            config: ConfigSource::Inline { body: "{}".into() },
            ports: vec![PortRequest::tcp(12808, "a")],
            privileges: vec![],
        });
        g.add_process(ProcessNode {
            id: "b".into(),
            core_type: CoreType::SingBox,
            config: ConfigSource::Inline { body: "{}".into() },
            ports: vec![PortRequest::tcp(12808, "b")],
            privileges: vec![],
        });
        assert_eq!(g.validate_ports().unwrap_err().code, codes::PORT_CONFLICT);
    }

    fn valid_plan() -> RuntimePlan {
        let mut g = ProcessGraph::default();
        g.add_process(ProcessNode {
            id: "main".into(),
            core_type: CoreType::Xray,
            config: ConfigSource::Inline { body: "{}".into() },
            ports: vec![],
            privileges: vec![],
        });
        RuntimePlan {
            plan_id: "plan-1".into(),
            desired_revision: 1,
            target: RuntimeTarget {
                core_type: CoreType::Xray,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("ab"),
            },
            process_graph: g,
            outbound_graph: OutboundGraph::default(),
            ports: vec![],
            privileges: vec![],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        }
    }

    #[test]
    fn empty_plan_id_is_rejected() {
        // S1-07: an empty identity must not pass the IPC-applied gate.
        let mut plan = valid_plan();
        plan.plan_id.clear();
        assert_eq!(plan.validate().unwrap_err().code, codes::INVALID_PLAN);
    }

    #[test]
    fn controlled_file_with_empty_token_or_hash_is_rejected() {
        // S1-07: a `ControlledFile` reference without its staging token or
        // content hash cannot be resolved by net-host.
        let mut plan = valid_plan();
        plan.target.config = ConfigSource::ControlledFile {
            staged_token: String::new(),
            sha256: ContentHash::new("ab"),
        };
        assert_eq!(plan.validate().unwrap_err().code, codes::INVALID_PLAN);
        plan.target.config = ConfigSource::ControlledFile {
            staged_token: "t".into(),
            sha256: ContentHash::new(""),
        };
        assert_eq!(plan.validate().unwrap_err().code, codes::INVALID_PLAN);
    }

    #[test]
    fn empty_or_oversize_inline_body_is_rejected() {
        // S1-07: inline content must exist and stay within a sane bound.
        let mut plan = valid_plan();
        plan.target.config = ConfigSource::Inline {
            body: String::new(),
        };
        assert_eq!(plan.validate().unwrap_err().code, codes::INVALID_PLAN);
        plan.target.config = ConfigSource::Inline {
            body: "x".repeat(17 * 1024 * 1024),
        };
        assert_eq!(plan.validate().unwrap_err().code, codes::INVALID_PLAN);
    }

    #[test]
    fn privileged_port_claim_requires_a_sub_1024_port() {
        // S1-07: claiming `PrivilegedPort` without any <1024 port is
        // inconsistent and must not reach net-host.
        let mut plan = valid_plan();
        plan.privileges = vec![RequiredPrivilege::PrivilegedPort];
        plan.ports = vec![PortRequest::tcp(11808, "inbound")];
        assert_eq!(plan.validate().unwrap_err().code, codes::INVALID_PLAN);
        plan.ports = vec![PortRequest::tcp(443, "inbound")];
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn tun_privilege_requires_tun_enabled() {
        // S1-07: a plan asking for TUN while its network policy forbids it
        // is contradictory.
        let mut plan = valid_plan();
        plan.privileges = vec![RequiredPrivilege::Tun];
        assert!(!plan.network_policy.tun_enabled);
        assert_eq!(plan.validate().unwrap_err().code, codes::INVALID_PLAN);
        plan.network_policy.tun_enabled = true;
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn same_owner_duplicate_port_is_rejected() {
        // S1-07: the old owner exemption hid same-process double claims.
        let mut plan = valid_plan();
        plan.ports = vec![
            PortRequest::tcp(11808, "inbound"),
            PortRequest::tcp(11808, "inbound"),
        ];
        assert_eq!(plan.validate().unwrap_err().code, codes::PORT_CONFLICT);
        let mut g = ProcessGraph::default();
        g.add_process(ProcessNode {
            id: "a".into(),
            core_type: CoreType::Xray,
            config: ConfigSource::Inline { body: "{}".into() },
            ports: vec![PortRequest::tcp(11808, "a"), PortRequest::tcp(11808, "a")],
            privileges: vec![],
        });
        assert_eq!(g.validate_ports().unwrap_err().code, codes::PORT_CONFLICT);
    }
}
