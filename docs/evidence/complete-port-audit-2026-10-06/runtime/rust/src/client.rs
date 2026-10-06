use domain::runtime_plan::{
    ConfigSource, ContentHash, PortRequest, ProcessNode, RequiredPrivilege, RuntimeTarget,
};
use domain::{CoreType, RuntimePlan};
use ipc_contract::{
    IpcOperation, IpcResult, RequestEnvelope, SessionIdentity, IPC_PROTOCOL_VERSION,
};
use runtime::{current_identity, decode_payload, encode_frame, ServerFrame, RUNTIME_DETAIL_EVENT};
use std::fs::OpenOptions;
use std::io::{Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("snapshot");
    let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(11977);
    assert!(port >= 11808 && port != 10808);
    let wire = std::env::var("V2RAYN_R_PIPE").expect("audit must set its own unique pipe");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(wire)
        .expect("open audit pipe");
    let identity = current_identity();
    let session = SessionIdentity {
        protocol_version: IPC_PROTOCOL_VERSION,
        session_token: "synthetic-audit-only".into(),
        peer_pid: identity.pid,
        peer_created_at_ms: identity.created_at_ms,
    };
    let operation = match command {
        "snapshot" => IpcOperation::GetSnapshot,
        "stop" => IpcOperation::StopRuntime { operation_id: None },
        "shutdown" => IpcOperation::Shutdown,
        "apply" | "apply-bad" => {
            let protocol = if command == "apply" {
                "socks"
            } else {
                "synthetic-invalid-protocol"
            };
            let body = serde_json::json!({"log":{"loglevel":"warning"}, "inbounds":[{"listen":"127.0.0.1","port":port,"protocol":protocol,"settings":{"auth":"noauth","udp":false}}],"outbounds":[{"protocol":"freedom","settings":{}}]}).to_string();
            let config = ConfigSource::Inline { body: body.clone() };
            let ports = vec![PortRequest::tcp(port, "inbound")];
            let core = CoreType::Xray;
            let mut graph = domain::runtime_plan::ProcessGraph::default();
            graph.add_process(ProcessNode {
                id: core.as_str().to_string(),
                core_type: core,
                config: config.clone(),
                ports: ports.clone(),
                privileges: vec![RequiredPrivilege::None],
            });
            let plan = RuntimePlan {
                plan_id: format!("synthetic-audit-{command}-{port}"),
                desired_revision: 1,
                target: RuntimeTarget {
                    core_type: core,
                    version: None,
                    config,
                    config_sha256: ContentHash::new(runtime::sha256_hex(body.as_bytes())),
                },
                process_graph: graph,
                outbound_graph: Default::default(),
                ports,
                privileges: vec![RequiredPrivilege::None],
                network_policy: Default::default(),
                resources: vec![],
            };
            IpcOperation::ApplyPlan {
                plan: Box::new(plan),
            }
        }
        other => panic!("unknown audit command {other}"),
    };
    let request = RequestEnvelope {
        session,
        request_id: format!("synthetic-{}", identity.pid),
        operation,
    };
    file.write_all(&encode_frame(&request).unwrap()).unwrap();
    file.flush().unwrap();
    loop {
        let mut prefix = [0u8; 4];
        file.read_exact(&mut prefix).unwrap();
        let size = runtime::frame_len(prefix);
        assert!(runtime::frame_len_ok(size));
        let mut bytes = vec![0; size as usize];
        file.read_exact(&mut bytes).unwrap();
        match decode_payload::<ServerFrame>(&bytes).unwrap() {
            ServerFrame::Event(event) if event.kind.as_str() == RUNTIME_DETAIL_EVENT => {
                println!("DETAIL {}", event.payload)
            }
            ServerFrame::Response(response) => {
                println!(
                    "RESULT {}",
                    serde_json::to_string(&response.result).unwrap()
                );
                if matches!(response.result, IpcResult::Error(_)) {
                    std::process::exit(3);
                }
                return;
            }
            _ => {}
        }
    }
}
