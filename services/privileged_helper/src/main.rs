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

    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|arg| arg == "--serve") {
        eprintln!(
            "privileged_helper: helper daemon; run with --serve and a token \
             (--token or V2RAYN_R_HELPER_TOKEN)"
        );
        return Ok(());
    }
    let arg_value = |flag: &str| -> Option<String> {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
            .filter(|value| !value.is_empty())
    };
    // ShellExecuteW does not forward the launching environment. The elevated
    // launch uses a short-lived token file instead of exposing the secret in
    // the process command line; environment and --token remain for service
    // installs and compatibility with existing launch scripts.
    let token = if let Some(path) = arg_value("--token-file") {
        let token = std::fs::read_to_string(&path)?;
        let _ = std::fs::remove_file(path);
        token
    } else {
        arg_value("--token")
            .or_else(|| std::env::var("V2RAYN_R_HELPER_TOKEN").ok())
            .unwrap_or_default()
    };
    if token.is_empty() {
        eprintln!("privileged_helper: a helper token is required (--token)");
        return Ok(());
    }
    let allowed_run_roots: Vec<String> = arg_value("--run-roots")
        .or_else(|| std::env::var("V2RAYN_R_HELPER_RUN_ROOTS").ok())
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();

    let backend = Arc::new(WindowsBackend::new(allowed_run_roots.clone()));
    let config = HelperServerConfig {
        session_token: token,
        expected_sid: current_user_sid_string().ok(),
        require_sid_match: true,
        // Dispatch validates `RunElevatedCore` against this list; the backend
        // keeps its own copy for execution. Both must carry the parsed roots.
        allowed_run_roots: allowed_run_roots.clone(),
        request_timeout: Duration::from_millis(IPC_REQUEST_TIMEOUT_MS),
        idle_timeout: Duration::from_secs(24 * 60 * 60),
        lease_policy: LeasePolicy::CleanOwned,
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
            // SAFETY: `raw` is a live handle obtained from `pipe` above.
            unsafe { client_sid(raw) }.ok()
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
