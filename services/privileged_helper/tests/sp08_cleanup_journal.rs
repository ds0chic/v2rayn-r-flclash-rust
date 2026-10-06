//! SP-08 helper-side regression: failed stop/TUN cleanup must retain
//! resources and allow retry (CP-04). FakeBackend fault injection only; no OS
//! routes, proxy, autostart or TUN writes.

use std::sync::Arc;
use std::time::Duration;

use ipc_contract::{
    AddressFamily, CidrAddress, ElevatedCoreSpec, HelperError, HelperOp, HelperRequest,
    HelperResult, RouteEntry, SessionIdentity, TunAddressConfig, HELPER_PROTOCOL_VERSION,
};
use privileged_helper::backend::{FakeBackend, FakeOp};
use privileged_helper::server::{ConnectionLease, HelperServer, HelperServerConfig, LeasePolicy};

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

fn server_with(backend: Arc<FakeBackend>) -> HelperServer<FakeBackend> {
    HelperServer::new(
        backend,
        HelperServerConfig {
            lease_policy: LeasePolicy::CleanOwned,
            allowed_run_roots: vec![ALLOWED_ROOT.to_string()],
            request_timeout: Duration::from_millis(100),
            ..HelperServerConfig::default()
        },
    )
}

fn v4_route() -> RouteEntry {
    RouteEntry {
        destination: "10.9.0.0/16".into(),
        next_hop: "10.9.0.1".into(),
        interface_index: 7,
        metric: 1,
        family: AddressFamily::V4,
    }
}

fn tun_config(index: u32) -> TunAddressConfig {
    TunAddressConfig {
        adapter_name: "v2rayn-tun".to_string(),
        interface_index: index,
        addresses: vec![CidrAddress {
            address: "198.18.0.1".to_string(),
            prefix_len: 16,
        }],
        mtu: Some(1500),
    }
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

fn start_core(server: &HelperServer<FakeBackend>, lease: &mut ConnectionLease) -> u64 {
    let response = server.handle(
        lease,
        &request(HelperOp::RunElevatedCore { spec: core_spec() }),
    );
    match response.result {
        HelperResult::CoreStarted { handle, .. } => handle,
        other => panic!("expected started core, got {other:?}"),
    }
}

#[test]
fn sp08_failed_route_cleanup_retains_ownership_for_retry() {
    let fake = Arc::new(FakeBackend::new().fail_on(
        FakeOp::RemoveRoutes,
        HelperError::Backend {
            detail: "injected route removal failure".into(),
        },
    ));
    let server = server_with(fake);
    let mut lease = ConnectionLease::new("sp08-routes");
    let response = server.handle(
        &mut lease,
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    assert!(matches!(
        response.result,
        HelperResult::RoutesAdded { count: 1 }
    ));

    let failures = server.on_disconnect(&mut lease);
    assert_eq!(
        failures.len(),
        1,
        "cleanup failure must be reported, not swallowed"
    );
    assert_eq!(
        lease.owned_route_count(),
        1,
        "failed routes must remain owned for retry"
    );
    assert!(
        !lease.is_closed(),
        "a lease with unconfirmed cleanup must not report closed"
    );
    // Retry must re-attempt the retained resource instead of reporting
    // idempotent success with no backend work.
    assert!(
        !server.on_disconnect(&mut lease).is_empty(),
        "retry must surface the still-failing resource, not fake success"
    );
}

#[test]
fn sp08_failed_tun_reset_retains_interface_and_journal() {
    let fake = Arc::new(FakeBackend::new().fail_on(
        FakeOp::ResetTunAddress,
        HelperError::Backend {
            detail: "injected tun reset failure".into(),
        },
    ));
    let server = server_with(fake);
    let mut lease = ConnectionLease::new("sp08-tun");
    let response = server.handle(
        &mut lease,
        &request(HelperOp::SetTunAdapterAddress {
            config: tun_config(7),
        }),
    );
    assert!(matches!(
        response.result,
        HelperResult::TunAddressSet { interface_index: 7 }
    ));

    let failures = server.on_disconnect(&mut lease);
    assert_eq!(failures.len(), 1);
    assert_eq!(
        lease.owned_tun_count(),
        1,
        "failed TUN addresses must remain owned for retry"
    );
    assert!(!lease.is_closed());
    assert_eq!(
        lease.pending_cleanup_count(),
        1,
        "journal must expose the unconfirmed resource"
    );
}

#[test]
fn sp08_failed_core_stop_retains_handle() {
    let fake = Arc::new(FakeBackend::new().fail_on(
        FakeOp::StopElevatedCore,
        HelperError::Backend {
            detail: "injected core stop failure".into(),
        },
    ));
    let server = server_with(fake);
    let mut lease = ConnectionLease::new("sp08-core");
    let handle = start_core(&server, &mut lease);

    let failures = server.on_disconnect(&mut lease);
    assert_eq!(failures.len(), 1);
    assert_eq!(lease.owned_core_count(), 1);
    assert!(
        lease.owned_cores().contains(&handle),
        "failed core handle must remain owned for retry"
    );
    assert!(!lease.is_closed());
}

#[test]
fn sp08_partial_failure_releases_only_confirmed_resources() {
    let fake = Arc::new(FakeBackend::new().fail_on(
        FakeOp::ResetTunAddress,
        HelperError::Backend {
            detail: "injected tun reset failure".into(),
        },
    ));
    let server = server_with(fake);
    let mut lease = ConnectionLease::new("sp08-partial");
    server.handle(
        &mut lease,
        &request(HelperOp::AddRoutes {
            entries: vec![v4_route()],
        }),
    );
    server.handle(
        &mut lease,
        &request(HelperOp::SetTunAdapterAddress {
            config: tun_config(7),
        }),
    );
    let _handle = start_core(&server, &mut lease);

    let failures = server.on_disconnect(&mut lease);
    assert_eq!(failures.len(), 1, "only the TUN reset was injected to fail");
    assert_eq!(lease.owned_route_count(), 0, "confirmed routes release");
    assert_eq!(lease.owned_core_count(), 0, "confirmed cores release");
    assert_eq!(lease.owned_tun_count(), 1, "failed TUN stays owned");
    assert!(!lease.is_closed());
}
