//! Process-topology execution order (plan §12).
//!
//! `RuntimePlan` already rejects cycles, dangling references and port
//! conflicts in the domain layer. The runtime layer turns a validated plan
//! into a deterministic start order, its reverse stop order, and the exact
//! rollback order for the subset of processes that actually started.

use std::collections::BTreeMap;

use domain::runtime_plan::{ProcessNode, RuntimePlan};
use domain::DomainError;

/// A validated, ready-to-run process topology.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    nodes: BTreeMap<String, ProcessNode>,
    start: Vec<String>,
}

impl ExecutionPlan {
    /// Validate the plan and freeze the start order.
    pub fn build(plan: &RuntimePlan) -> Result<Self, DomainError> {
        plan.validate()?;
        let start = plan.process_graph.start_order()?;
        let nodes = plan
            .process_graph
            .nodes
            .iter()
            .cloned()
            .map(|node| (node.id.clone(), node))
            .collect();
        Ok(Self { nodes, start })
    }

    pub fn start_order(&self) -> &[String] {
        &self.start
    }

    /// Stop in reverse start order.
    pub fn stop_order(&self) -> Vec<String> {
        let mut order = self.start.clone();
        order.reverse();
        order
    }

    /// Rollback order for the processes that had already started before a
    /// failure: the reverse of the started prefix.
    pub fn rollback_order(&self, started: &[String]) -> Vec<String> {
        let mut order = started.to_vec();
        order.reverse();
        order
    }

    pub fn node(&self, id: &str) -> Option<&ProcessNode> {
        self.nodes.get(id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::runtime_plan::{
        ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, PortRequest, ProcessGraph,
        ProcessNode, RequiredPrivilege, RuntimeTarget,
    };
    use domain::CoreType;

    fn node(id: &str, ports: Vec<PortRequest>) -> ProcessNode {
        ProcessNode {
            id: id.into(),
            core_type: CoreType::Xray,
            config: ConfigSource::Inline { body: "{}".into() },
            ports,
            privileges: vec![RequiredPrivilege::None],
        }
    }

    fn plan_with(graph: ProcessGraph) -> RuntimePlan {
        RuntimePlan {
            plan_id: "p".into(),
            desired_revision: 1,
            target: RuntimeTarget {
                core_type: CoreType::Xray,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("00"),
            },
            process_graph: graph,
            outbound_graph: OutboundGraph::default(),
            ports: vec![],
            privileges: vec![RequiredPrivilege::None],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        }
    }

    #[test]
    fn start_order_is_dependency_ordered() {
        let mut graph = ProcessGraph::default();
        for id in ["tun", "app", "proxy"] {
            graph.add_process(node(id, vec![]));
        }
        graph.depends_on("app", "proxy");
        graph.depends_on("tun", "app");
        let exec = ExecutionPlan::build(&plan_with(graph)).unwrap();
        assert_eq!(exec.start_order(), &["proxy", "app", "tun"]);
        assert_eq!(exec.stop_order(), vec!["tun", "app", "proxy"]);
    }

    #[test]
    fn cycle_is_rejected() {
        let mut graph = ProcessGraph::default();
        graph.add_process(node("a", vec![]));
        graph.add_process(node("b", vec![]));
        graph.depends_on("a", "b");
        graph.depends_on("b", "a");
        let err = ExecutionPlan::build(&plan_with(graph)).unwrap_err();
        assert_eq!(err.code, domain::codes::GRAPH_CYCLE);
    }

    #[test]
    fn port_conflict_is_rejected() {
        let mut graph = ProcessGraph::default();
        graph.add_process(node("a", vec![PortRequest::tcp(11808, "a")]));
        graph.add_process(node("b", vec![PortRequest::tcp(11808, "b")]));
        let err = ExecutionPlan::build(&plan_with(graph)).unwrap_err();
        assert_eq!(err.code, domain::codes::PORT_CONFLICT);
    }

    #[test]
    fn rollback_reverses_only_started_prefix() {
        let mut graph = ProcessGraph::default();
        for id in ["a", "b", "c"] {
            graph.add_process(node(id, vec![]));
        }
        // Force a deterministic lexical order a -> b -> c.
        graph.depends_on("b", "a");
        graph.depends_on("c", "b");
        let exec = ExecutionPlan::build(&plan_with(graph)).unwrap();
        assert_eq!(exec.start_order(), &["a", "b", "c"]);
        // Failure while starting `c`: only a,b started.
        let started = ["a".to_string(), "b".to_string()];
        assert_eq!(exec.rollback_order(&started), vec!["b", "a"]);
    }

    #[test]
    fn empty_topology_is_allowed() {
        let exec = ExecutionPlan::build(&plan_with(ProcessGraph::default())).unwrap();
        assert!(exec.is_empty());
        assert!(exec.node("missing").is_none());
    }
}
