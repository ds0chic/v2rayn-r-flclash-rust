use std::sync::Arc;
use std::time::Instant;
use ipc_contract::{CidrAddress, ElevatedCoreSpec, HelperError, HelperOp, HelperRequest, HelperResponse, HelperResult, SessionIdentity, TunAddressConfig, HELPER_PROTOCOL_VERSION};
use privileged_helper::backend::FakeBackend;
use privileged_helper::server::{serve_connection, ConnectionLease, HelperServer, HelperServerConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn send(client: &mut tokio::io::DuplexStream, operation: HelperOp) -> HelperResult {
    let request = HelperRequest {
        session: SessionIdentity { protocol_version: HELPER_PROTOCOL_VERSION, session_token: "synthetic-audit-only".into(), peer_pid: 1, peer_created_at_ms: 1 },
        request_id: "synthetic-request".into(), operation,
    };
    let body = serde_json::to_vec(&request).unwrap();
    client.write_all(&(body.len() as u32).to_le_bytes()).await.unwrap();
    client.write_all(&body).await.unwrap();
    response(client).await.result
}

async fn response(client: &mut tokio::io::DuplexStream) -> HelperResponse {
    let mut prefix = [0u8; 4];
    client.read_exact(&mut prefix).await.unwrap();
    let mut body = vec![0u8; u32::from_le_bytes(prefix) as usize];
    client.read_exact(&mut body).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::main]
async fn main() {
    println!("synthetic-only: tokio duplex + FakeBackend; no socket, WindowsBackend or OS write");
    let fake = Arc::new(FakeBackend::new().elevated(true));
    let config = HelperServerConfig {
        session_token: "synthetic-audit-only".into(),
        allowed_run_roots: vec![r"C:\synthetic-tun-audit".into()],
        ..HelperServerConfig::default()
    };
    println!("default idle/request timeout: {}ms", config.request_timeout.as_millis());
    let server = Arc::new(HelperServer::new(fake.clone(), config));
    let (mut client, stream) = tokio::io::duplex(65536);
    let task = tokio::spawn(async move { serve_connection(stream, server, ConnectionLease::new("audit-lease"), None).await.unwrap(); });
    let address_result = send(&mut client, HelperOp::SetTunAdapterAddress { config: TunAddressConfig {
        adapter_name: "synthetic-audit-tun".into(), interface_index: 7,
        addresses: vec![CidrAddress { address: "198.18.0.1".into(), prefix_len: 30 }], mtu: Some(1280),
    }}).await;
    assert!(matches!(address_result, HelperResult::TunAddressSet { .. }));
    let core_result = send(&mut client, HelperOp::RunElevatedCore { spec: ElevatedCoreSpec {
        core: "sing-box".into(), exe_path: r"C:\synthetic-tun-audit\s1\sing-box.exe".into(),
        args: vec!["run".into(), "-c".into(), "config.json".into()], run_dir: r"C:\synthetic-tun-audit\s1".into(),
    }}).await;
    let handle = match core_result { HelperResult::CoreStarted { handle, .. } => handle, other => panic!("unexpected {other:?}") };
    println!("before idle: interfaces={:?}, elevated_core_running={}", fake.tun_interfaces(), fake.is_running(handle));
    let start = Instant::now();
    let idle = tokio::time::timeout(std::time::Duration::from_secs(8), response(&mut client)).await.unwrap();
    assert!(matches!(idle.result, HelperResult::Error { error: HelperError::Timeout { .. } }));
    task.await.unwrap();
    println!("after {}ms idle: interfaces={:?}, elevated_core_running={}, stopped_handles={:?}", start.elapsed().as_millis(), fake.tun_interfaces(), fake.is_running(handle), fake.stopped_handles());
    assert!(fake.tun_interfaces().is_empty());
    assert!(!fake.is_running(handle));
    println!("REPRODUCED: an open idle helper connection cleans its live TUN lease and kills the elevated core");

    let engine = application::AppEngine::in_memory();
    let mut node = application::synthetic::synthetic_full_profile(1);
    node.core_type = Some(domain::CoreType::Xray);
    engine.seed(vec![node.clone()]);
    let loaded = engine.load_settings().unwrap();
    let mut settings = loaded.settings;
    settings.tun_mode_item.enable_tun = true;
    settings.tun_mode_item.enable_legacy_protect = true;
    settings.inbound[0].local_port = 11970;
    engine.save_settings(settings, loaded.revision).unwrap();
    let hints = application::tun_plan::TunPlanHints { interface_index: 9, ..Default::default() };
    let plan = engine.build_runtime_plan_with_hints(&node.index_id, engine.desired_revision(), &hints).unwrap();
    let mut providers = 0;
    for process in &plan.process_graph.nodes {
        if let domain::runtime_plan::ConfigSource::Inline { body } = &process.config {
            let config: serde_json::Value = serde_json::from_str(body).unwrap();
            if let Some(inbounds) = config["inbounds"].as_array() {
                for inbound in inbounds {
                    if inbound["protocol"] == "tun" || inbound["type"] == "tun" {
                        providers += 1;
                        println!("TUN provider: node={}, protocol={}, type={}, name={}, interface_name={}", process.id, inbound["protocol"], inbound["type"], inbound["settings"]["name"], inbound["interface_name"]);
                    }
                }
            }
        }
    }
    println!("legacy-protect TUN provider count={providers} (frozen upstream expects 1)");
    assert_eq!(providers, 2, "pin current duplicate-provider defect, not intended product behavior");
    println!("REPRODUCED: legacy protect leaves both the main Xray and pre-SOCKS sing-box configured as TUN providers");
}
