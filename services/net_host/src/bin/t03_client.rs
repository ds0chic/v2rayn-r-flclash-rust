//! T03 fault-injection / evidence client.
//!
//! A *synchronous* named-pipe client used by the T03 fault tests. It is not
//! the product path (the product path is `application::NetHostClient`); it
//! exists so the failure cases (client kill, port conflict, core quick-exit)
//! can be driven deterministically from scripts. It submits the same
//! `RuntimePlan` shape the app uses.

#![allow(clippy::result_large_err)]

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::time::Duration;

use domain::runtime_plan::{
    ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, PortRequest, ProcessGraph,
    ProcessNode, RequiredPrivilege, RuntimePlan, RuntimeTarget,
};
use domain::CoreType;
use ipc_contract::{
    IpcOperation, IpcResult, RequestEnvelope, SessionIdentity, IPC_PROTOCOL_VERSION,
};
use runtime::{
    current_identity, decode_payload, encode_frame, frame_len, sha256_hex, ServerFrame,
    NET_HOST_PIPE_NAME, RUNTIME_DETAIL_EVENT,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("snapshot");
    let port: u16 = args.get(2).and_then(|p| p.parse().ok()).unwrap_or(11808);

    let mut conn = match connect() {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("CONNECT_FAILED {e}");
            std::process::exit(2);
        }
    };

    match command {
        "apply" => {
            let (body, label) = good_config(port);
            match conn.apply(&body, label, port, 1) {
                Ok(result) => {
                    print_result(&result);
                    conn.print_detail();
                    if is_error(&result) {
                        println!("APPLY_ERROR");
                        std::process::exit(3);
                    }
                    println!("APPLY_OK");
                }
                Err(e) => {
                    println!("APPLY_ERROR {e}");
                    std::process::exit(3);
                }
            }
        }
        "apply-exit" => {
            let (body, label) = good_config(port);
            match conn.apply(&body, label, port, 1) {
                Ok(result) => {
                    print_result(&result);
                    conn.print_detail();
                    if is_error(&result) {
                        println!("APPLY_ERROR");
                        std::process::exit(3);
                    }
                }
                Err(e) => {
                    println!("APPLY_ERROR {e}");
                    std::process::exit(3);
                }
            }
            // Simulate a GUI hard-kill: leave immediately without stopping.
            println!("CLIENT_EXITING_WITHOUT_STOP");
            std::process::exit(0);
        }
        "apply-hold" => {
            let secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(60);
            let (body, label) = good_config(port);
            match conn.apply(&body, label, port, 1) {
                Ok(result) => {
                    print_result(&result);
                    conn.print_detail();
                    if is_error(&result) {
                        println!("APPLY_ERROR");
                        std::process::exit(3);
                    }
                    println!("APPLY_OK");
                    println!("HOLDING {secs}");
                    std::thread::sleep(Duration::from_secs(secs));
                }
                Err(e) => {
                    println!("APPLY_ERROR {e}");
                    std::process::exit(3);
                }
            }
        }
        "apply-bad" => {
            let body = format!(
                "{{\"log\":{{\"loglevel\":\"warning\"}},\"inbounds\":[{{\"listen\":\"127.0.0.1\",\"port\":{port},\"protocol\":\"not-a-real-protocol\"}}]}}"
            );
            match conn.apply(&body, "t03-bad-core", port, 1) {
                Ok(result) => {
                    print_result(&result);
                    conn.print_detail();
                    if is_error(&result) {
                        println!("APPLY_ERROR");
                        std::process::exit(3);
                    }
                }
                Err(e) => {
                    println!("APPLY_ERROR {e}");
                    std::process::exit(3);
                }
            }
        }
        "snapshot" => match conn.snapshot() {
            Ok(result) => {
                print_result(&result);
                conn.print_detail();
            }
            Err(e) => println!("SNAPSHOT_ERROR {e}"),
        },
        "stop" => {
            let _ = conn.stop();
            println!("STOPPED");
        }
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(64);
        }
    }
}

fn good_config(port: u16) -> (String, &'static str) {
    let body = format!(
        "{{\"log\":{{\"loglevel\":\"warning\"}},\"inbounds\":[{{\"tag\":\"socks\",\"listen\":\"127.0.0.1\",\"port\":{port},\"protocol\":\"socks\",\"settings\":{{\"auth\":\"noauth\",\"udp\":true}}}}],\"outbounds\":[{{\"protocol\":\"freedom\"}}]}}"
    );
    (body, "t03-good-core")
}

struct Client {
    file: File,
    session: SessionIdentity,
    last_detail: Option<serde_json::Value>,
}

fn connect() -> std::io::Result<Client> {
    let path = std::env::var("V2RAYN_R_PIPE").unwrap_or_else(|_| NET_HOST_PIPE_NAME.to_string());
    let mut last_err = None;
    for _ in 0..40 {
        match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => {
                let id = current_identity();
                return Ok(Client {
                    file,
                    session: SessionIdentity {
                        protocol_version: IPC_PROTOCOL_VERSION,
                        session_token: format!("t03-client-{}", id.pid),
                        peer_pid: id.pid,
                        peer_created_at_ms: id.created_at_ms,
                    },
                    last_detail: None,
                });
            }
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("pipe unavailable")))
}

impl Client {
    fn call(&mut self, operation: IpcOperation) -> std::io::Result<IpcResult> {
        let request = RequestEnvelope {
            session: self.session.clone(),
            request_id: format!("req-{}", std::process::id()),
            operation,
        };
        let bytes = encode_frame(&request).map_err(|e| std::io::Error::other(e.to_string()))?;
        self.file.write_all(&bytes)?;
        self.file.flush()?;
        loop {
            let frame = self.read_frame()?;
            match frame {
                ServerFrame::Event(event) => {
                    if event.kind.as_str() == RUNTIME_DETAIL_EVENT {
                        self.last_detail = Some(event.payload);
                    }
                }
                ServerFrame::Response(response) => return Ok(response.result),
            }
        }
    }

    fn read_frame(&mut self) -> std::io::Result<ServerFrame> {
        let mut prefix = [0u8; 4];
        self.file.read_exact(&mut prefix)?;
        let len = frame_len(prefix);
        if !runtime::frame_len_ok(len) {
            return Err(std::io::Error::other("frame too large"));
        }
        let mut payload = vec![0u8; len as usize];
        self.file.read_exact(&mut payload)?;
        decode_payload::<ServerFrame>(&payload).map_err(|e| std::io::Error::other(e.to_string()))
    }

    fn apply(
        &mut self,
        body: &str,
        plan_id: &str,
        port: u16,
        revision: u64,
    ) -> Result<IpcResult, String> {
        let plan = build_plan(body, plan_id, port, revision);
        match self.call(IpcOperation::ApplyPlan {
            plan: Box::new(plan),
        }) {
            Ok(result) => Ok(result),
            Err(e) => Err(e.to_string()),
        }
    }

    fn snapshot(&mut self) -> Result<IpcResult, String> {
        self.call(IpcOperation::GetSnapshot)
            .map_err(|e| e.to_string())
    }

    fn stop(&mut self) -> Result<IpcResult, String> {
        self.call(IpcOperation::StopRuntime { operation_id: None })
            .map_err(|e| e.to_string())
    }

    fn print_detail(&self) {
        if let Some(detail) = &self.last_detail {
            println!("DETAIL {detail}");
        }
    }
}

fn is_error(result: &IpcResult) -> bool {
    matches!(result, IpcResult::Error(_))
}

fn print_result(result: &IpcResult) {
    println!(
        "RESULT {}",
        serde_json::to_string(result).unwrap_or_default()
    );
}

fn build_plan(body: &str, plan_id: &str, port: u16, revision: u64) -> RuntimePlan {
    let node = ProcessNode {
        id: "xray".into(),
        core_type: CoreType::Xray,
        config: ConfigSource::Inline {
            body: body.to_string(),
        },
        ports: vec![PortRequest::tcp(port, "inbound-socks")],
        privileges: vec![RequiredPrivilege::None],
    };
    let mut graph = ProcessGraph::default();
    graph.add_process(node);
    RuntimePlan {
        plan_id: plan_id.to_string(),
        desired_revision: revision,
        target: RuntimeTarget {
            core_type: CoreType::Xray,
            version: None,
            config: ConfigSource::Inline {
                body: body.to_string(),
            },
            config_sha256: ContentHash::new(sha256_hex(body.as_bytes())),
        },
        process_graph: graph,
        outbound_graph: OutboundGraph::default(),
        ports: vec![PortRequest::tcp(port, "inbound-socks")],
        privileges: vec![RequiredPrivilege::None],
        network_policy: NetworkPolicy::default(),
        resources: vec![],
    }
}
