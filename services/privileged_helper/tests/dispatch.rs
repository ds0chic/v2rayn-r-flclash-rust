//! Synchronous dispatch, validation, audit and lease tests (T14).
//!
//! Everything runs against the in-memory `FakeBackend`; no OS privilege, route
//! or TUN operation is executed.

use std::sync::Arc;
use std::time::Duration;

use ipc_contract::{
    AddressFamily, CidrAddress, ElevatedCoreSpec, HelperError, HelperOp, HelperRequest,
    HelperResult, LeaseState, RouteEntry, SessionIdentity, TunAddressConfig,
    HELPER_PROTOCOL_VERSION,
};
use privileged_helper::backend::{FakeBackend, FakeCall, FakeOp};
use privileged_helper::server::{
    sid_matches, ConnectionLease, HelperServer, HelperServerConfig, LeasePolicy,
};

const ALLOWED_ROOT: &str = r"C:\v2rayn-r";

fn session() -> SessionIdentity {
    SessionIdentity {
        protocol_version: HELPER_PROTOCOL_VERSION,
        session_token: "tok".to_string(),
        peer_pid: 42,
        peer_created_at_ms: 1,
    }
}

fn request(operation: HelperOp) -> HelperRequest {
    HelperRequest {
        session: session(),
        request_id: "r1".to_string(),
        operation,
    }
}

fn server_with(backend: Arc<FakeBackend>, policy: LeasePolicy) -> HelperServer<FakeBackend> {
    HelperServer::new(
        backend,
        HelperServerConfig {
            session_token: "tok".to_string(),
            allowed_run_roots: vec![ALLOWED_ROOT.to_string()],
            request_timeout: Duration::from_millis(100),
            lease_policy: policy,
            ..HelperServerConfig::default()
        },
    )
}

fn route(destination: &str, next_hop: &str, family: AddressFamily) -> RouteEntry {
    RouteEntry {
        destination: destination.to_string(),
        next_hop: next_hop.to_string(),
        interface_index: 5,
        metric: 1,
        family,
    }
}

fn v4_route() -> RouteEntry {
    route("0.0.0.0/0", "10.0.0.1", AddressFamily::V4)
}

fn core_spec() -> ElevatedCoreSpec {
    ElevatedCoreSpec {
        core: "xray".to_string(),
        exe_path: format!(r"{ALLOWED_ROOT}\core\xray.exe"),
        args: vec![
            "run".to_string(),
            "-c".to_string(),
            "config.json".to_string(),
        ],
        run_dir: format!(r"{ALLOWED_ROOT}\core"),
    }
}

fn tun_config() -> TunAddressConfig {
    TunAddressConfig {
        adapter_name: "v2rayn-tun".to_string(),
        interface_index: 9,
        addresses: vec![CidrAddress {
            address: "198.18.0.1".to_string(),
            prefix_len: 16,
        }],
        mtu: Some(1500),
    }
}

fn lease(id: &str) -> ConnectionLease {
    ConnectionLease::new(id)
}

#[test]
fn ping_reports_elevation() {
    let fake = Arc::new(FakeBackend::new().elevated(true));
    let server = server_with(fake, LeasePolicy::CleanOwned);
    let response = server.handle(&mut lease("s1"), &request(HelperOp::Ping));
    match response.result {
        HelperResult::Pong { elevation } => {
            assert!(elevation.elevated);
            assert_eq!(elevation.protocol_version, HELPER_PROTOCOL_VERSION);
        }
        other => panic!("expected pong, got {other:?}"),
    }
}

#[test]
fn elevation_status_reports_session_count() {
    let fake = Arc::new(FakeBackend::new().elevated(true));
    let server = server_with(fake, LeasePolicy::CleanOwned);
    server.connection_opened();
    server.connection_opened();
    let response = server.handle(&mut lease("s1"), &request(HelperOp::GetElevationStatus));
    match response.result {
        HelperResult::ElevationStatus { status } => assert_eq!(status.session_count, 2),
        other => panic!("expected status, got {other:?}"),
    }
    server.connection_closed();
    server.connection_closed();
}

#[test]
fn add_routes_records_and_counts() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    assert!(matches!(
        response.result,
        HelperResult::RoutesAdded { count: 1 }
    ));
    assert!(matches!(fake.calls()[0], FakeCall::AddRoutes(_)));
}

#[test]
fn remove_routes_records() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::RemoveRoutes {
            entries: vec![v4_route()],
        }),
    );
    assert!(matches!(
        response.result,
        HelperResult::RoutesRemoved { count: 1 }
    ));
    assert!(matches!(fake.calls()[0], FakeCall::RemoveRoutes(_)));
}

#[test]
fn set_tun_records() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::SetTunAdapterAddress {
            config: tun_config(),
        }),
    );
    assert!(matches!(
        response.result,
        HelperResult::TunAddressSet { interface_index: 9 }
    ));
    assert_eq!(fake.tun_interfaces(), vec![9]);
}

#[test]
fn run_core_returns_handle_and_records() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::RunElevatedCore { spec: core_spec() }),
    );
    match response.result {
        HelperResult::CoreStarted { handle, pid } => {
            assert_eq!(pid, 2000);
            assert!(fake.is_running(handle));
        }
        other => panic!("expected started core, got {other:?}"),
    }
}

#[test]
fn stop_core_is_idempotent() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s1");
    let start = server.handle(
        &mut lease,
        &request(HelperOp::RunElevatedCore { spec: core_spec() }),
    );
    let handle = match start.result {
        HelperResult::CoreStarted { handle, .. } => handle,
        other => panic!("expected started core, got {other:?}"),
    };
    let first = server.handle(&mut lease, &request(HelperOp::StopElevatedCore { handle }));
    let second = server.handle(&mut lease, &request(HelperOp::StopElevatedCore { handle }));
    assert!(matches!(first.result, HelperResult::CoreStopped { .. }));
    assert!(matches!(second.result, HelperResult::CoreStopped { .. }));
    assert!(!fake.is_running(handle));
}

#[test]
fn stop_unknown_handle_is_not_found() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake, LeasePolicy::CleanOwned);
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::StopElevatedCore { handle: 999_999 }),
    );
    match response.result {
        HelperResult::Error {
            error: HelperError::UnknownHandle { handle },
        } => assert_eq!(handle, 999_999),
        other => panic!("expected unknown handle, got {other:?}"),
    }
}

#[test]
fn shutdown_sets_flag() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let response = server.handle(&mut lease("s1"), &request(HelperOp::Shutdown));
    assert!(matches!(response.result, HelperResult::Shutdown));
    assert!(server.is_shutdown());
    assert_eq!(fake.shutdown_count(), 1);
}

#[test]
fn reject_arbitrary_executable() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut spec = core_spec();
    spec.core = "cmd".to_string();
    spec.exe_path = format!(r"{ALLOWED_ROOT}\core\cmd.exe");
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::RunElevatedCore { spec }),
    );
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::NotAllowlisted { .. }
        }
    ));
    assert!(fake.calls().is_empty());
}

#[test]
fn reject_path_out_of_bounds() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut spec = core_spec();
    spec.exe_path = r"C:\elsewhere\xray.exe".to_string();
    spec.run_dir = r"C:\elsewhere".to_string();
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::RunElevatedCore { spec }),
    );
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::PathOutOfBounds { .. }
        }
    ));
    assert!(fake.calls().is_empty());
}

#[test]
fn reject_bad_args() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut spec = core_spec();
    spec.args = vec!["bad\0arg".to_string()];
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::RunElevatedCore { spec }),
    );
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::Malformed { .. }
        }
    ));
    assert!(fake.calls().is_empty());
}

#[test]
fn reject_invalid_routes_before_backend() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::AddRoutes {
            entries: vec![route("10.0.0.0/33", "10.0.0.1", AddressFamily::V4)],
        }),
    );
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::Malformed { .. }
        }
    ));
    assert!(fake.calls().is_empty());
}

#[test]
fn version_mismatch_response() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake, LeasePolicy::CleanOwned);
    let mut req = request(HelperOp::Ping);
    req.session.protocol_version = HELPER_PROTOCOL_VERSION + 7;
    let response = server.handle(&mut lease("s1"), &req);
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::VersionMismatch { .. }
        }
    ));
}

#[test]
fn empty_token_rejected() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake, LeasePolicy::CleanOwned);
    let mut req = request(HelperOp::Ping);
    req.session.session_token.clear();
    let response = server.handle(&mut lease("s1"), &req);
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::Unauthorized { .. }
        }
    ));
}

#[test]
fn audit_records_success_fields() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake, LeasePolicy::CleanOwned);
    server.handle(&mut lease("s1"), &request(HelperOp::Ping));
    let records = server.audit().records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].session_id, "s1");
    assert_eq!(records[0].operation, "ping");
    assert_eq!(records[0].summary, "ping");
    assert_eq!(records[0].outcome, privileged_helper::AuditOutcome::Ok);
}

#[test]
fn audit_marks_rejection() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake, LeasePolicy::CleanOwned);
    let mut spec = core_spec();
    spec.core = "cmd".to_string();
    spec.exe_path = format!(r"{ALLOWED_ROOT}\core\cmd.exe");
    server.handle(
        &mut lease("s1"),
        &request(HelperOp::RunElevatedCore { spec }),
    );
    let records = server.audit().records();
    let last = records.last().expect("one record");
    assert_eq!(last.outcome, privileged_helper::AuditOutcome::Rejected);
    assert!(last.summary.contains("not_allowlisted"));
}

#[test]
fn audit_summary_excludes_args_and_paths() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake, LeasePolicy::CleanOwned);
    server.handle(
        &mut lease("s1"),
        &request(HelperOp::RunElevatedCore { spec: core_spec() }),
    );
    let summary = server.audit().records()[0].summary.clone();
    assert!(!summary.contains("config.json"));
    assert!(!summary.contains(ALLOWED_ROOT));
    assert!(summary.contains("core=xray"));
}

#[test]
fn lease_cleanup_removes_routes() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s1");
    server.handle(
        &mut lease,
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    assert_eq!(lease.owned_route_count(), 1);
    server.on_disconnect(&mut lease);
    let calls = fake.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, FakeCall::RemoveRoutes(_))));
    assert_eq!(lease.owned_route_count(), 0);
}

#[test]
fn lease_cleanup_resets_tun() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s1");
    server.handle(
        &mut lease,
        &request(HelperOp::SetTunAdapterAddress {
            config: tun_config(),
        }),
    );
    server.on_disconnect(&mut lease);
    assert!(fake.tun_interfaces().is_empty());
    assert!(fake
        .calls()
        .iter()
        .any(|call| matches!(call, FakeCall::ResetTunAddress(9))));
}

#[test]
fn lease_cleanup_stops_cores() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s1");
    let start = server.handle(
        &mut lease,
        &request(HelperOp::RunElevatedCore { spec: core_spec() }),
    );
    let handle = match start.result {
        HelperResult::CoreStarted { handle, .. } => handle,
        other => panic!("expected started core, got {other:?}"),
    };
    server.on_disconnect(&mut lease);
    assert!(!fake.is_running(handle));
    assert_eq!(fake.stopped_handles(), vec![handle]);
}

#[test]
fn lease_policy_stop_cores_only_leaves_routes() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::StopCoresOnly);
    let mut lease = lease("s1");
    server.handle(
        &mut lease,
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    server.handle(
        &mut lease,
        &request(HelperOp::RunElevatedCore { spec: core_spec() }),
    );
    server.on_disconnect(&mut lease);
    let calls = fake.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, FakeCall::StopElevatedCore(_))));
    assert!(!calls
        .iter()
        .any(|call| matches!(call, FakeCall::RemoveRoutes(_))));
}

#[test]
fn lease_policy_leave_running_skips_cleanup() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::LeaveRunning);
    let mut lease = lease("s1");
    server.handle(
        &mut lease,
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    server.handle(
        &mut lease,
        &request(HelperOp::SetTunAdapterAddress {
            config: tun_config(),
        }),
    );
    server.on_disconnect(&mut lease);
    assert_eq!(fake.call_count(), 2);
    assert_eq!(fake.tun_interfaces(), vec![9]);
}

#[test]
fn lease_cleanup_is_idempotent() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s1");
    server.handle(
        &mut lease,
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    fake.clear_calls();
    server.on_disconnect(&mut lease);
    server.on_disconnect(&mut lease);
    let removals = fake
        .calls()
        .iter()
        .filter(|call| matches!(call, FakeCall::RemoveRoutes(_)))
        .count();
    assert_eq!(removals, 1);
    assert!(lease.is_closed());
}

#[test]
fn sessions_are_isolated() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease_a = lease("a");
    let mut lease_b = lease("b");
    server.handle(
        &mut lease_a,
        &request(HelperOp::AddRoutes {
            entries: vec![route("10.1.0.0/16", "10.0.0.1", AddressFamily::V4)],
        }),
    );
    server.handle(
        &mut lease_b,
        &request(HelperOp::AddRoutes {
            entries: vec![route("10.2.0.0/16", "10.0.0.2", AddressFamily::V4)],
        }),
    );
    fake.clear_calls();
    server.on_disconnect(&mut lease_a);
    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    match &calls[0] {
        FakeCall::RemoveRoutes(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].destination, "10.1.0.0/16");
        }
        other => panic!("expected remove of A routes, got {other:?}"),
    }
    assert_eq!(lease_b.owned_route_count(), 1);
}

#[test]
fn backend_failure_propagates() {
    let fake = Arc::new(FakeBackend::new().fail_on(
        FakeOp::AddRoutes,
        HelperError::Backend {
            detail: "boom".to_string(),
        },
    ));
    let server = server_with(fake, LeasePolicy::CleanOwned);
    let response = server.handle(
        &mut lease("s1"),
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::Backend { .. }
        }
    ));
    assert_eq!(
        server.audit().records()[0].outcome,
        privileged_helper::AuditOutcome::Error
    );
}

#[test]
fn call_order_is_preserved() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s1");
    server.handle(
        &mut lease,
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    server.handle(
        &mut lease,
        &request(HelperOp::RemoveRoutes {
            entries: vec![v4_route()],
        }),
    );
    let calls = fake.calls();
    assert_eq!(calls[0].op(), FakeOp::AddRoutes);
    assert_eq!(calls[1].op(), FakeOp::RemoveRoutes);
}

#[test]
fn sid_comparison_is_case_insensitive_and_strict() {
    assert!(sid_matches("S-1-5-21-1-2-3", "s-1-5-21-1-2-3"));
    assert!(!sid_matches("S-1-5-21-1-2-3", "S-1-5-21-9-9-9"));
    assert!(!sid_matches("", "S-1-5-21-1-2-3"));
}

/// Start one elevated core through dispatch and return its handle.
fn start_core(server: &HelperServer<FakeBackend>, lease: &mut ConnectionLease) -> u64 {
    match server
        .handle(
            lease,
            &request(HelperOp::RunElevatedCore { spec: core_spec() }),
        )
        .result
    {
        HelperResult::CoreStarted { handle, .. } => handle,
        other => panic!("expected CoreStarted, got {other:?}"),
    }
}

#[test]
fn sp09_renew_lease_extends_owned_handle_only() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake, LeasePolicy::CleanOwned);
    let mut lease = lease("s-renew");
    let handle = start_core(&server, &mut lease);

    let first = match server
        .handle(&mut lease, &request(HelperOp::GetLeaseStatus { handle }))
        .result
    {
        HelperResult::LeaseStatus { status } => status,
        other => panic!("expected LeaseStatus, got {other:?}"),
    };
    assert_eq!(first.state, LeaseState::Active);
    assert!(first.expires_at_ms > 0, "an owned handle has an expiry");

    let renewed = match server
        .handle(&mut lease, &request(HelperOp::RenewLease { handle }))
        .result
    {
        HelperResult::LeaseRenewed { expires_at_ms, .. } => expires_at_ms,
        other => panic!("expected LeaseRenewed, got {other:?}"),
    };
    assert!(renewed >= first.expires_at_ms);

    let unknown = server
        .handle(
            &mut lease,
            &request(HelperOp::RenewLease { handle: 424_242 }),
        )
        .result;
    assert!(matches!(
        unknown,
        HelperResult::Error {
            error: HelperError::UnknownHandle { handle: 424_242 }
        }
    ));

    let zero = server
        .handle(&mut lease, &request(HelperOp::RenewLease { handle: 0 }))
        .result;
    assert!(matches!(
        zero,
        HelperResult::Error {
            error: HelperError::Malformed { .. }
        }
    ));
}

#[test]
fn sp10_poll_core_exits_drains_once_and_status_reflects_it() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s-poll");
    let handle = start_core(&server, &mut lease);

    let quiet = server
        .handle(
            &mut lease,
            &request(HelperOp::PollCoreExits {
                handles: vec![handle],
            }),
        )
        .result;
    assert!(matches!(
        quiet,
        HelperResult::CoreExits { ref exits } if exits.is_empty()
    ));

    fake.inject_exit(handle, Some(3));
    let first = match server
        .handle(
            &mut lease,
            &request(HelperOp::PollCoreExits {
                handles: vec![handle],
            }),
        )
        .result
    {
        HelperResult::CoreExits { exits } => exits,
        other => panic!("expected CoreExits, got {other:?}"),
    };
    assert_eq!(first.len(), 1, "one owned exit observed");
    assert_eq!(first[0].handle, handle);
    assert_eq!(first[0].exit_code, Some(3));
    assert!(first[0].at_ms > 0);

    let second = server
        .handle(
            &mut lease,
            &request(HelperOp::PollCoreExits {
                handles: vec![handle],
            }),
        )
        .result;
    assert!(
        matches!(second, HelperResult::CoreExits { ref exits } if exits.is_empty()),
        "an observed exit is never reported twice"
    );

    let status = match server
        .handle(&mut lease, &request(HelperOp::GetLeaseStatus { handle }))
        .result
    {
        HelperResult::LeaseStatus { status } => status,
        other => panic!("expected LeaseStatus, got {other:?}"),
    };
    assert_eq!(status.state, LeaseState::Exited);

    server.handle(&mut lease, &request(HelperOp::StopElevatedCore { handle }));
    let released = match server
        .handle(&mut lease, &request(HelperOp::GetLeaseStatus { handle }))
        .result
    {
        HelperResult::LeaseStatus { status } => status,
        other => panic!("expected LeaseStatus, got {other:?}"),
    };
    assert_eq!(released.state, LeaseState::Released);
}

#[test]
fn sp10_poll_handles_validate_before_backend() {
    let fake = Arc::new(FakeBackend::new());
    let server = server_with(fake.clone(), LeasePolicy::CleanOwned);
    let mut lease = lease("s-poll-bad");

    for handles in [vec![], vec![0], vec![1, 1]] {
        let result = server
            .handle(&mut lease, &request(HelperOp::PollCoreExits { handles }))
            .result;
        assert!(matches!(
            result,
            HelperResult::Error {
                error: HelperError::Malformed { .. }
            }
        ));
    }
    assert_eq!(fake.call_count(), 0, "no backend work for invalid lists");
}
