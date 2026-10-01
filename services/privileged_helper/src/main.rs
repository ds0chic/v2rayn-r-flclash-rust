//! Privileged helper daemon entry point.
//!
//! Running it does not itself elevate: elevation is a deployment property. The
//! daemon only starts with the explicit `--serve` flag and a shared token from
//! the environment, so an accidental invocation is a no-op.

#[cfg(not(windows))]
fn main() {
    eprintln!("privileged_helper: only supported on Windows (T14)");
}

#[cfg(windows)]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use ipc_contract::{HELPER_PIPE_NAME, IPC_REQUEST_TIMEOUT_MS};
    use privileged_helper::server::{
        serve_connection, ConnectionLease, HelperServer, HelperServerConfig, LeasePolicy,
    };
    use privileged_helper::windows::{client_sid, current_user_sid_string, WindowsBackend};

    if !std::env::args().any(|arg| arg == "--serve") {
        eprintln!(
            "privileged_helper: helper daemon; run with --serve and V2RAYN_R_HELPER_TOKEN set"
        );
        return Ok(());
    }
    let token = std::env::var("V2RAYN_R_HELPER_TOKEN").unwrap_or_default();
    if token.is_empty() {
        eprintln!("privileged_helper: V2RAYN_R_HELPER_TOKEN is required");
        return Ok(());
    }
    let allowed_run_roots: Vec<String> = std::env::var("V2RAYN_R_HELPER_RUN_ROOTS")
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();

    let backend = Arc::new(WindowsBackend::new(allowed_run_roots));
    let config = HelperServerConfig {
        session_token: token,
        expected_sid: current_user_sid_string().ok(),
        require_sid_match: true,
        request_timeout: Duration::from_millis(IPC_REQUEST_TIMEOUT_MS),
        lease_policy: LeasePolicy::CleanOwned,
        ..HelperServerConfig::default()
    };
    let server = Arc::new(HelperServer::new(backend, config));
    let session_counter = AtomicU64::new(1);

    let mut first = true;
    loop {
        let pipe = match create_pipe(HELPER_PIPE_NAME, first) {
            Ok(pipe) => pipe,
            Err(error) if first => {
                eprintln!("[helper] cannot create pipe {HELPER_PIPE_NAME}: {error}");
                return Err(error);
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        first = false;
        if let Err(error) = pipe.connect().await {
            eprintln!("[helper] connect error: {error}");
            continue;
        }
        if server.is_shutdown() {
            break;
        }
        let raw = pipe.as_raw_handle();
        let peer_sid = if raw.is_null() {
            None
        } else {
            client_sid(raw).ok()
        };
        let session_id = format!("s{}", session_counter.fetch_add(1, Ordering::SeqCst));
        let server = server.clone();
        tokio::spawn(async move {
            let lease = ConnectionLease::new(session_id);
            if let Err(error) = serve_connection(pipe, server, lease, peer_sid).await {
                eprintln!("[helper] session ended: {error}");
            }
        });
    }
    Ok(())
}

#[cfg(windows)]
fn create_pipe(
    name: &str,
    first: bool,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use privileged_helper::windows::PipeSecurity;
    use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};

    let security = PipeSecurity::current_user_only()?;
    let mut options = ServerOptions::new();
    options.pipe_mode(PipeMode::Byte);
    options.reject_remote_clients(true);
    options.max_instances(16);
    options.first_pipe_instance(first);
    unsafe { options.create_with_security_attributes_raw(name, security.as_mut_ptr()) }
}
