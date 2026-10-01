//! Privileged-helper IPC contract (T14).
//!
//! The helper exists to perform a *finite, enumerated* set of operations that
//! require elevation (plan §5): add/remove routes, configure a TUN adapter
//! address, and start/stop an allow-listed core. It is deliberately **not** a
//! general administrator: there is no shell operation, no arbitrary program
//! execution, and no operation taking an arbitrary command line.
//!
//! This module is a backward-compatible addition to [`crate`]: it has its own
//! protocol version and does not touch the frozen net-host message set. Every
//! input is validated here (path boundary, parameter enumeration, argument
//! bounds) so the helper's dispatch code can trust the request shape.

use std::net::IpAddr;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{SessionIdentity, IPC_APPLY_TIMEOUT_MS, IPC_MAX_MESSAGE_BYTES, IPC_REQUEST_TIMEOUT_MS};
use domain::{codes, DomainError};

/// Helper protocol version. Bump on any incompatible helper message change.
pub const HELPER_PROTOCOL_VERSION: u32 = 1;

/// Well-known named pipe the privileged helper listens on (Windows).
pub const HELPER_PIPE_NAME: &str = r"\\.\pipe\v2rayn-r-helper";

/// Hard cap on a single helper frame. Reuses the net-host framing convention;
/// helper control messages are small and never carry config bodies.
pub const HELPER_MAX_MESSAGE_BYTES: usize = IPC_MAX_MESSAGE_BYTES;

/// Maximum route entries accepted in one `AddRoutes` / `RemoveRoutes` request.
pub const HELPER_MAX_ROUTE_ENTRIES: usize = 256;

/// Maximum addresses accepted in one `SetTunAdapterAddress` request.
pub const HELPER_MAX_TUN_ADDRESSES: usize = 64;

/// Maximum number of arguments accepted for an elevated core.
pub const HELPER_MAX_ARGS: usize = 64;

/// Maximum byte length of a single argument.
pub const HELPER_MAX_ARG_BYTES: usize = 4096;

/// Maximum total command-line byte length (Windows limit is 32,767).
pub const HELPER_MAX_COMMAND_LINE_BYTES: usize = 32_000;

/// Executable base names (without `.exe`) the helper may start elevated.
/// This is a closed allow-list: anything else is rejected before dispatch.
pub const ALLOWED_ELEVATED_CORES: &[&str] = &[
    "xray",
    "v2ray",
    "v2fly",
    "sing-box",
    "mihomo",
    "hysteria",
    "hysteria2",
    "tuic",
    "naive",
    "naiveproxy",
    "brook",
    "overtls",
    "shadowquic",
    "mieru",
    "juicity",
];

/// A request envelope for the helper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelperRequest {
    /// Session identity; version and token are checked, SID is transport-checked.
    pub session: SessionIdentity,
    /// Correlation id echoed on the response.
    pub request_id: String,
    pub operation: HelperOp,
}

/// A response envelope for the helper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelperResponse {
    pub request_id: String,
    pub result: HelperResult,
}

/// The closed helper operation set. Unknown operations cannot be represented,
/// so no arbitrary command can be smuggled through the protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum HelperOp {
    /// Liveness probe; reports elevation status.
    Ping,
    /// Read whether the helper token is elevated and its session count.
    GetElevationStatus,
    /// Add the enumerated route entries (all-or-nothing per entry list).
    AddRoutes { entries: Vec<RouteEntry> },
    /// Remove the enumerated route entries.
    RemoveRoutes { entries: Vec<RouteEntry> },
    /// Configure addresses/MTU on a TUN adapter by interface index.
    SetTunAdapterAddress { config: TunAddressConfig },
    /// Start an allow-listed core elevated, from a controlled run directory.
    RunElevatedCore { spec: ElevatedCoreSpec },
    /// Stop a previously started elevated core by opaque helper handle.
    StopElevatedCore { handle: u64 },
    /// Graceful shutdown of the helper after cleanup.
    Shutdown,
}

/// IP address family of a route entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressFamily {
    V4,
    V6,
}

impl AddressFamily {
    pub fn of(addr: &IpAddr) -> Self {
        match addr {
            IpAddr::V4(_) => AddressFamily::V4,
            IpAddr::V6(_) => AddressFamily::V6,
        }
    }
}

/// One route to add/remove, mirroring the fields of `MIB_IPFORWARD_ROW2` the
/// helper actually uses. No raw SDK struct crosses the IPC boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteEntry {
    /// Destination prefix in CIDR form (`0.0.0.0/0`, `fd00::/8`).
    pub destination: String,
    /// Next hop address; family must match `destination`.
    pub next_hop: String,
    /// Egress interface index (must be non-zero).
    pub interface_index: u32,
    /// Route metric.
    pub metric: u32,
    /// Address family, validated against the parsed addresses.
    pub family: AddressFamily,
}

/// A CIDR address applied to a TUN adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CidrAddress {
    pub address: String,
    pub prefix_len: u8,
}

/// TUN adapter address configuration request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunAddressConfig {
    /// Adapter name; a label, never a path.
    pub adapter_name: String,
    /// Adapter interface index as reported by the OS.
    pub interface_index: u32,
    /// Addresses to assign.
    pub addresses: Vec<CidrAddress>,
    /// Optional MTU.
    pub mtu: Option<u16>,
}

/// Elevated core launch request. The executable is identified by an allow-list
/// entry plus a path that must resolve under a caller-provided controlled root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevatedCoreSpec {
    /// Declared core id; must be in [`ALLOWED_ELEVATED_CORES`].
    pub core: String,
    /// Executable path; its file name must match `core` and its parent must be
    /// exactly `run_dir`.
    pub exe_path: String,
    /// Arguments passed as an array; never a single shell command string.
    pub args: Vec<String>,
    /// Controlled run directory; must be under an allowed root.
    pub run_dir: String,
}

/// Elevation facts reported by the helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevationStatus {
    pub elevated: bool,
    pub session_count: u32,
    pub protocol_version: u32,
}

/// Result payload of a helper call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HelperResult {
    Pong { elevation: ElevationStatus },
    ElevationStatus { status: ElevationStatus },
    RoutesAdded { count: u32 },
    RoutesRemoved { count: u32 },
    TunAddressSet { interface_index: u32 },
    CoreStarted { handle: u64, pid: u32 },
    CoreStopped { handle: u64 },
    Shutdown,
    Error { error: HelperError },
}

/// Structured helper errors. Kept separate from [`crate::IpcError`] because the
/// helper has security-boundary outcomes (`NotAllowlisted`, `PathOutOfBounds`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HelperError {
    VersionMismatch {
        peer: u32,
        host: u32,
    },
    MessageTooLarge {
        size: usize,
        limit: usize,
    },
    Malformed {
        detail: String,
    },
    Unauthorized {
        detail: String,
    },
    NotAllowlisted {
        detail: String,
    },
    PathOutOfBounds {
        detail: String,
    },
    UnknownHandle {
        handle: u64,
    },
    Backend {
        detail: String,
    },
    /// The managed core could not be bound into the ownership Job Object.
    /// The launch is aborted rather than leaving a process running unowned.
    JobAssignFailed {
        detail: String,
    },
    Timeout {
        timeout_ms: u64,
    },
}

impl HelperError {
    pub fn malformed(detail: impl Into<String>) -> Self {
        HelperError::Malformed {
            detail: detail.into(),
        }
    }

    pub fn to_domain(&self) -> DomainError {
        match self {
            HelperError::VersionMismatch { peer, host } => {
                DomainError::new(codes::IPC_VERSION_MISMATCH, "error.ipc_version_mismatch")
                    .with_detail(format!("helper peer speaks v{peer}, host speaks v{host}"))
            }
            HelperError::MessageTooLarge { size, limit } => {
                DomainError::new(codes::IPC_MESSAGE_TOO_LARGE, "error.ipc_message_too_large")
                    .with_detail(format!("{size} bytes exceeds helper limit {limit}"))
            }
            HelperError::Malformed { detail } => {
                DomainError::new(codes::INVALID_ARGUMENT, "error.helper_malformed")
                    .with_detail(detail.clone())
            }
            HelperError::Unauthorized { detail } => {
                DomainError::new(codes::PERMISSION_DENIED, "error.helper_unauthorized")
                    .with_detail(detail.clone())
            }
            HelperError::NotAllowlisted { detail } => {
                DomainError::new(codes::PERMISSION_DENIED, "error.helper_not_allowlisted")
                    .with_detail(detail.clone())
            }
            HelperError::PathOutOfBounds { detail } => {
                DomainError::new(codes::PERMISSION_DENIED, "error.helper_path_out_of_bounds")
                    .with_detail(detail.clone())
            }
            HelperError::UnknownHandle { handle } => {
                DomainError::not_found("elevated_core", &handle.to_string())
            }
            HelperError::Backend { detail } => {
                DomainError::new(codes::UNAVAILABLE, "error.helper_backend")
                    .with_detail(detail.clone())
            }
            HelperError::JobAssignFailed { detail } => {
                DomainError::new(codes::JOB_ASSIGN_FAILED, "error.job_assign_failed")
                    .with_detail(detail.clone())
            }
            HelperError::Timeout { timeout_ms } => {
                DomainError::new(codes::TIMEOUT, "error.timeout")
                    .retryable()
                    .with_detail(format!("helper request exceeded {timeout_ms} ms"))
            }
        }
    }
}

/// Validate the byte length of an encoded helper frame against the size cap.
pub fn check_helper_frame_size(len: usize) -> Result<(), HelperError> {
    if len > HELPER_MAX_MESSAGE_BYTES {
        Err(HelperError::MessageTooLarge {
            size: len,
            limit: HELPER_MAX_MESSAGE_BYTES,
        })
    } else {
        Ok(())
    }
}

/// Validate a helper peer session (protocol version and non-empty token).
pub fn check_helper_session(session: &SessionIdentity) -> Result<(), HelperError> {
    if session.protocol_version != HELPER_PROTOCOL_VERSION {
        return Err(HelperError::VersionMismatch {
            peer: session.protocol_version,
            host: HELPER_PROTOCOL_VERSION,
        });
    }
    if session.session_token.is_empty() {
        return Err(HelperError::Unauthorized {
            detail: "empty session token".to_string(),
        });
    }
    Ok(())
}

/// Request timeout for a helper operation. Process start/stop may be slow.
pub fn helper_timeout_for(op: &HelperOp) -> u64 {
    match op {
        HelperOp::RunElevatedCore { .. }
        | HelperOp::StopElevatedCore { .. }
        | HelperOp::Shutdown => IPC_APPLY_TIMEOUT_MS,
        _ => IPC_REQUEST_TIMEOUT_MS,
    }
}

/// Parse a CIDR string into an address plus a validated prefix length.
pub fn parse_cidr(value: &str) -> Result<(IpAddr, u8), HelperError> {
    let (addr_part, prefix_part) = value.split_once('/').ok_or_else(|| {
        HelperError::malformed(format!("cidr `{value}` is missing a prefix length"))
    })?;
    let addr = IpAddr::from_str(addr_part.trim())
        .map_err(|_| HelperError::malformed(format!("cidr `{value}` has an invalid address")))?;
    let prefix: u8 = prefix_part
        .trim()
        .parse()
        .map_err(|_| HelperError::malformed(format!("cidr `{value}` has an invalid prefix")))?;
    let max = match addr {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if prefix > max {
        return Err(HelperError::malformed(format!(
            "cidr `{value}` prefix {prefix} exceeds {max}"
        )));
    }
    Ok((addr, prefix))
}

/// Validate a route entry list. All-or-nothing: any bad entry rejects the set.
pub fn validate_route_entries(entries: &[RouteEntry]) -> Result<(), HelperError> {
    if entries.is_empty() {
        return Err(HelperError::malformed("route entry list is empty"));
    }
    if entries.len() > HELPER_MAX_ROUTE_ENTRIES {
        return Err(HelperError::malformed(format!(
            "{} route entries exceed the limit of {HELPER_MAX_ROUTE_ENTRIES}",
            entries.len()
        )));
    }
    for (index, entry) in entries.iter().enumerate() {
        let (destination, _) =
            parse_cidr(&entry.destination).map_err(|e| relabel(e, index, "destination"))?;
        let next_hop = IpAddr::from_str(entry.next_hop.trim()).map_err(|_| {
            HelperError::malformed(format!("route[{index}].next_hop is not an IP address"))
        })?;
        if AddressFamily::of(&destination) != entry.family {
            return Err(HelperError::malformed(format!(
                "route[{index}].destination family does not match family"
            )));
        }
        if AddressFamily::of(&next_hop) != entry.family {
            return Err(HelperError::malformed(format!(
                "route[{index}].next_hop family does not match family"
            )));
        }
        if entry.interface_index == 0 {
            return Err(HelperError::malformed(format!(
                "route[{index}].interface_index must be non-zero"
            )));
        }
        if entry.metric > 9_999 {
            return Err(HelperError::malformed(format!(
                "route[{index}].metric {} exceeds 9999",
                entry.metric
            )));
        }
    }
    Ok(())
}

/// Validate a TUN adapter address request.
pub fn validate_tun_address(config: &TunAddressConfig) -> Result<(), HelperError> {
    let name = config.adapter_name.trim();
    if name.is_empty() || name.len() > 256 {
        return Err(HelperError::malformed(
            "adapter_name must be 1..=256 characters",
        ));
    }
    if name.contains('\\') || name.contains('/') || name.contains('\0') {
        return Err(HelperError::malformed(
            "adapter_name must not contain path separators",
        ));
    }
    if config.interface_index == 0 {
        return Err(HelperError::malformed("interface_index must be non-zero"));
    }
    if config.addresses.is_empty() || config.addresses.len() > HELPER_MAX_TUN_ADDRESSES {
        return Err(HelperError::malformed(format!(
            "address count must be 1..={HELPER_MAX_TUN_ADDRESSES}"
        )));
    }
    for (index, address) in config.addresses.iter().enumerate() {
        let (parsed, prefix) = parse_cidr(&format!("{}/{}", address.address, address.prefix_len))
            .map_err(|e| relabel(e, index, "address"))?;
        let max = match parsed {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if prefix > max {
            return Err(HelperError::malformed(format!(
                "address[{index}] prefix exceeds {max}"
            )));
        }
    }
    if let Some(mtu) = config.mtu {
        if !(576..=9_000).contains(&mtu) {
            return Err(HelperError::malformed(format!(
                "mtu {mtu} is outside 576..=9000"
            )));
        }
    }
    Ok(())
}

/// Validate the argument array of an elevated core spec.
pub fn validate_args(args: &[String]) -> Result<(), HelperError> {
    if args.len() > HELPER_MAX_ARGS {
        return Err(HelperError::malformed(format!(
            "{} arguments exceed the limit of {HELPER_MAX_ARGS}",
            args.len()
        )));
    }
    let mut total = 0usize;
    for (index, arg) in args.iter().enumerate() {
        if arg.len() > HELPER_MAX_ARG_BYTES {
            return Err(HelperError::malformed(format!(
                "arg[{index}] exceeds {HELPER_MAX_ARG_BYTES} bytes"
            )));
        }
        if arg
            .chars()
            .any(|c| c == '\0' || (c.is_control() && c != '\t'))
        {
            return Err(HelperError::malformed(format!(
                "arg[{index}] contains a control character"
            )));
        }
        total += arg.len() + 1;
    }
    if total > HELPER_MAX_COMMAND_LINE_BYTES {
        return Err(HelperError::malformed(format!(
            "command line of {total} bytes exceeds {HELPER_MAX_COMMAND_LINE_BYTES}"
        )));
    }
    Ok(())
}

/// Whether a core executable base name is allow-listed (case-insensitive,
/// `.exe` extension optional).
pub fn is_allowed_core_name(name: &str) -> bool {
    let stem = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name);
    ALLOWED_ELEVATED_CORES
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(stem))
}

/// Validate an elevated-core launch against a set of controlled run roots.
///
/// Rejects: a core not in [`ALLOWED_ELEVATED_CORES`], an executable whose file
/// name does not match the declared core, an executable not directly inside
/// `run_dir`, a `run_dir` not under an allowed root, and any path traversal.
pub fn validate_elevated_core(
    spec: &ElevatedCoreSpec,
    allowed_run_roots: &[String],
) -> Result<(), HelperError> {
    let core = spec.core.trim();
    if !is_allowed_core_name(core) {
        return Err(HelperError::NotAllowlisted {
            detail: format!("core `{core}` is not in the elevated allow-list"),
        });
    }

    let exe =
        normalize_windows_path(&spec.exe_path).ok_or_else(|| HelperError::PathOutOfBounds {
            detail: format!(
                "exe_path `{}` is not an absolute, normalized path",
                spec.exe_path
            ),
        })?;
    let run_dir =
        normalize_windows_path(&spec.run_dir).ok_or_else(|| HelperError::PathOutOfBounds {
            detail: format!(
                "run_dir `{}` is not an absolute, normalized path",
                spec.run_dir
            ),
        })?;

    let exe_name = normalized_file_name(&exe).ok_or_else(|| HelperError::PathOutOfBounds {
        detail: "exe_path has no file name".to_string(),
    })?;
    let exe_stem = exe_name
        .strip_suffix(".exe")
        .or_else(|| exe_name.strip_suffix(".EXE"))
        .unwrap_or(exe_name);
    if !exe_stem.eq_ignore_ascii_case(core) && !is_alias(exe_stem, core) {
        return Err(HelperError::NotAllowlisted {
            detail: format!("executable `{exe_name}` does not match declared core `{core}`"),
        });
    }
    if !exe_name.to_ascii_lowercase().ends_with(".exe") {
        return Err(HelperError::NotAllowlisted {
            detail: format!("executable `{exe_name}` must have a .exe extension"),
        });
    }

    let exe_parent = normalized_parent(&exe).ok_or_else(|| HelperError::PathOutOfBounds {
        detail: "exe_path has no parent directory".to_string(),
    })?;
    if !path_equal(exe_parent, &run_dir) {
        return Err(HelperError::PathOutOfBounds {
            detail: "exe_path must live directly inside run_dir".to_string(),
        });
    }

    if !allowed_run_roots.iter().any(|root| {
        normalize_windows_path(root)
            .map(|normalized| path_under(&run_dir, &normalized))
            .unwrap_or(false)
    }) {
        return Err(HelperError::PathOutOfBounds {
            detail: "run_dir is not under any controlled run root".to_string(),
        });
    }

    // Filesystem re-verification: defeat symlink / junction / 8.3 escapes
    // that are invisible to the lexical checks above.
    validate_elevated_core_canonical(&spec.exe_path, &spec.run_dir, allowed_run_roots)?;

    validate_args(&spec.args)
}

/// Re-verify the lexical containment of [`validate_elevated_core`] against
/// canonicalized paths when the entries exist on disk.
///
/// `std::fs::canonicalize` resolves symlinks, junctions, 8.3 short names and
/// mount points, so an `exe_path` that is lexically inside `run_dir` but
/// resolves outside is rejected here. Entries that do not exist yet (staging
/// layout) are skipped: the lexical checks already passed and there is
/// nothing to resolve. On non-Windows hosts Windows-style paths cannot be
/// canonicalized and the check is skipped as well.
pub fn validate_elevated_core_canonical(
    exe_path: &str,
    run_dir: &str,
    allowed_run_roots: &[String],
) -> Result<(), HelperError> {
    let canon_exe = match std::fs::canonicalize(exe_path) {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    let canon_run = match std::fs::canonicalize(run_dir) {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };
    let canon_parent = canon_exe
        .parent()
        .ok_or_else(|| HelperError::PathOutOfBounds {
            detail: "canonical exe_path has no parent directory".to_string(),
        })?;
    if canon_parent != canon_run {
        return Err(HelperError::PathOutOfBounds {
            detail: "canonical exe_path is not directly inside run_dir (possible symlink escape)"
                .to_string(),
        });
    }
    let mut under_root = false;
    for root in allowed_run_roots {
        match std::fs::canonicalize(root) {
            Ok(canon_root) => {
                if canon_run == canon_root || canon_run.starts_with(&canon_root) {
                    under_root = true;
                    break;
                }
            }
            Err(_) => continue,
        }
    }
    if !under_root {
        return Err(HelperError::PathOutOfBounds {
            detail: "canonical run_dir is not under any controlled run root".to_string(),
        });
    }
    Ok(())
}

/// Alias table for core ids whose executable name differs (e.g. `naive`).
fn is_alias(exe_stem: &str, core: &str) -> bool {
    matches!(
        (
            core.to_ascii_lowercase().as_str(),
            exe_stem.to_ascii_lowercase().as_str()
        ),
        ("naive", "naiveproxy") | ("naiveproxy", "naive")
    )
}

/// Lexically normalize an absolute Windows path. Returns `None` for relative
/// paths, UNC-less paths, NUL bytes, or any path containing `..`.
pub fn normalize_windows_path(path: &str) -> Option<String> {
    let replaced = path.trim().replace('/', "\\");
    if replaced.is_empty() || replaced.contains('\0') {
        return None;
    }
    let (root, rest) = if let Some(stripped) = replaced.strip_prefix("\\\\") {
        let mut parts = stripped.splitn(3, '\\');
        let server = parts.next()?;
        let share = parts.next()?;
        if server.is_empty() || share.is_empty() {
            return None;
        }
        (
            format!("\\\\{server}\\{share}"),
            parts.next().unwrap_or("").to_string(),
        )
    } else {
        let bytes = replaced.as_bytes();
        if bytes.len() < 3
            || !bytes[0].is_ascii_alphabetic()
            || bytes[1] != b':'
            || bytes[2] != b'\\'
        {
            return None;
        }
        (replaced[..2].to_string(), replaced[3..].to_string())
    };
    let mut components: Vec<String> = Vec::new();
    for component in rest.split('\\') {
        match component {
            "" | "." => continue,
            ".." => return None,
            other => components.push(other.to_string()),
        }
    }
    let mut normalized = root;
    for component in components {
        normalized.push('\\');
        normalized.push_str(&component);
    }
    Some(normalized)
}

/// File-name component of an already-normalized absolute Windows path.
pub fn normalized_file_name(path: &str) -> Option<&str> {
    let index = path.rfind('\\')?;
    let name = &path[index + 1..];
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Parent-directory component of an already-normalized absolute Windows path.
pub fn normalized_parent(path: &str) -> Option<&str> {
    let index = path.rfind('\\')?;
    let parent = &path[..index];
    if parent.is_empty() || parent.ends_with(':') {
        None
    } else {
        Some(parent)
    }
}

/// Case-insensitive equality of two already-normalized paths.
pub fn path_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Whether `candidate` is `root` itself or lexically below it.
pub fn path_under(candidate: &str, root: &str) -> bool {
    if path_equal(candidate, root) {
        return true;
    }
    let prefix = if root.ends_with('\\') {
        root.to_string()
    } else {
        format!("{root}\\")
    };
    candidate
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

fn relabel(error: HelperError, index: usize, field: &str) -> HelperError {
    match error {
        HelperError::Malformed { detail } => {
            HelperError::malformed(format!("[{index}].{field}: {detail}"))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(version: u32) -> SessionIdentity {
        SessionIdentity {
            protocol_version: version,
            session_token: "tok".into(),
            peer_pid: 7,
            peer_created_at_ms: 1,
        }
    }

    fn sample_spec() -> ElevatedCoreSpec {
        ElevatedCoreSpec {
            core: "xray".into(),
            exe_path: r"C:\v2rayn-r\core\xray.exe".into(),
            args: vec!["run".into(), "-c".into(), "config.json".into()],
            run_dir: r"C:\v2rayn-r\core".into(),
        }
    }

    #[test]
    fn helper_request_roundtrips_through_json() {
        let request = HelperRequest {
            session: session(HELPER_PROTOCOL_VERSION),
            request_id: "h1".into(),
            operation: HelperOp::AddRoutes {
                entries: vec![RouteEntry {
                    destination: "0.0.0.0/0".into(),
                    next_hop: "192.168.1.1".into(),
                    interface_index: 12,
                    metric: 5,
                    family: AddressFamily::V4,
                }],
            },
        };
        let json = serde_json::to_string(&request).unwrap();
        let back: HelperRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, back);
    }

    #[test]
    fn helper_response_carries_structured_error() {
        let response = HelperResponse {
            request_id: "h2".into(),
            result: HelperResult::Error {
                error: HelperError::UnknownHandle { handle: 99 },
            },
        };
        let json = serde_json::to_string(&response).unwrap();
        let back: HelperResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, back);
    }

    #[test]
    fn helper_version_mismatch_is_rejected() {
        let err = check_helper_session(&session(HELPER_PROTOCOL_VERSION + 1)).unwrap_err();
        assert!(matches!(err, HelperError::VersionMismatch { .. }));
        assert_eq!(err.to_domain().code, codes::IPC_VERSION_MISMATCH);
    }

    #[test]
    fn helper_empty_token_is_rejected() {
        let mut identity = session(HELPER_PROTOCOL_VERSION);
        identity.session_token.clear();
        assert!(matches!(
            check_helper_session(&identity).unwrap_err(),
            HelperError::Unauthorized { .. }
        ));
    }

    #[test]
    fn helper_frame_size_is_enforced() {
        assert!(check_helper_frame_size(HELPER_MAX_MESSAGE_BYTES).is_ok());
        assert!(matches!(
            check_helper_frame_size(HELPER_MAX_MESSAGE_BYTES + 1).unwrap_err(),
            HelperError::MessageTooLarge { .. }
        ));
    }

    #[test]
    fn elevated_core_uses_long_timeout() {
        let op = HelperOp::RunElevatedCore {
            spec: sample_spec(),
        };
        assert_eq!(helper_timeout_for(&op), IPC_APPLY_TIMEOUT_MS);
        assert_eq!(helper_timeout_for(&HelperOp::Ping), IPC_REQUEST_TIMEOUT_MS);
    }

    #[test]
    fn allowed_core_names_are_closed() {
        assert!(is_allowed_core_name("xray"));
        assert!(is_allowed_core_name("XRAY.EXE"));
        assert!(!is_allowed_core_name("cmd"));
        assert!(!is_allowed_core_name("powershell.exe"));
    }

    #[test]
    fn elevated_core_accepts_allowlisted_path() {
        let roots = vec![r"C:\v2rayn-r".to_string()];
        assert!(validate_elevated_core(&sample_spec(), &roots).is_ok());
    }

    #[test]
    fn elevated_core_rejects_arbitrary_executable() {
        let roots = vec![r"C:\v2rayn-r".to_string()];
        let mut spec = sample_spec();
        spec.core = "cmd".into();
        spec.exe_path = r"C:\v2rayn-r\core\cmd.exe".into();
        assert!(matches!(
            validate_elevated_core(&spec, &roots).unwrap_err(),
            HelperError::NotAllowlisted { .. }
        ));
    }

    #[test]
    fn elevated_core_rejects_mismatched_executable() {
        let roots = vec![r"C:\v2rayn-r".to_string()];
        let mut spec = sample_spec();
        spec.exe_path = r"C:\v2rayn-r\core\sing-box.exe".into();
        assert!(validate_elevated_core(&spec, &roots).is_err());
    }

    #[test]
    fn elevated_core_rejects_traversal_and_out_of_root() {
        let roots = vec![r"C:\v2rayn-r".to_string()];
        let mut spec = sample_spec();
        spec.exe_path = r"C:\v2rayn-r\core\..\..\Windows\xray.exe".into();
        assert!(matches!(
            validate_elevated_core(&spec, &roots).unwrap_err(),
            HelperError::PathOutOfBounds { .. }
        ));

        let mut spec = sample_spec();
        spec.exe_path = r"C:\elsewhere\xray.exe".into();
        spec.run_dir = r"C:\elsewhere".into();
        assert!(matches!(
            validate_elevated_core(&spec, &roots).unwrap_err(),
            HelperError::PathOutOfBounds { .. }
        ));
    }

    #[test]
    fn elevated_core_rejects_exe_outside_run_dir() {
        let roots = vec![r"C:\v2rayn-r".to_string()];
        let mut spec = sample_spec();
        spec.exe_path = r"C:\v2rayn-r\core\nested\xray.exe".into();
        assert!(matches!(
            validate_elevated_core(&spec, &roots).unwrap_err(),
            HelperError::PathOutOfBounds { .. }
        ));
    }

    #[test]
    fn elevated_core_rejects_bad_args() {
        let roots = vec![r"C:\v2rayn-r".to_string()];
        let mut spec = sample_spec();
        spec.args = vec!["bad\0arg".into()];
        assert!(validate_elevated_core(&spec, &roots).is_err());

        let mut spec = sample_spec();
        spec.args = vec!["a\n".into()];
        assert!(validate_args(&spec.args).is_err());
    }

    #[test]
    fn route_entries_validate_families() {
        let good = vec![
            RouteEntry {
                destination: "0.0.0.0/0".into(),
                next_hop: "10.0.0.1".into(),
                interface_index: 3,
                metric: 1,
                family: AddressFamily::V4,
            },
            RouteEntry {
                destination: "fd00::/8".into(),
                next_hop: "fe80::1".into(),
                interface_index: 3,
                metric: 2,
                family: AddressFamily::V6,
            },
        ];
        assert!(validate_route_entries(&good).is_ok());

        let mut bad = good.clone();
        bad[0].family = AddressFamily::V6;
        assert!(validate_route_entries(&bad).is_err());

        let mut bad = good.clone();
        bad[0].interface_index = 0;
        assert!(validate_route_entries(&bad).is_err());

        let mut bad = good;
        bad[0].destination = "10.0.0.0/33".into();
        assert!(validate_route_entries(&bad).is_err());
    }

    #[test]
    fn route_entries_reject_empty_and_oversized() {
        assert!(validate_route_entries(&[]).is_err());
        let entry = RouteEntry {
            destination: "0.0.0.0/0".into(),
            next_hop: "10.0.0.1".into(),
            interface_index: 1,
            metric: 1,
            family: AddressFamily::V4,
        };
        let many = vec![entry; HELPER_MAX_ROUTE_ENTRIES + 1];
        assert!(validate_route_entries(&many).is_err());
    }

    #[test]
    fn tun_address_validates_bounds() {
        let good = TunAddressConfig {
            adapter_name: "v2rayn-tun".into(),
            interface_index: 4,
            addresses: vec![CidrAddress {
                address: "198.18.0.1".into(),
                prefix_len: 16,
            }],
            mtu: Some(1500),
        };
        assert!(validate_tun_address(&good).is_ok());

        let mut bad = good.clone();
        bad.mtu = Some(100);
        assert!(validate_tun_address(&bad).is_err());

        let mut bad = good.clone();
        bad.adapter_name = r"..\evil".into();
        assert!(validate_tun_address(&bad).is_err());

        let mut bad = good;
        bad.interface_index = 0;
        assert!(validate_tun_address(&bad).is_err());
    }

    #[test]
    fn path_normalization_rejects_relative_and_traversal() {
        assert!(normalize_windows_path(r"relative\path").is_none());
        assert!(normalize_windows_path(r"C:\a\..\b").is_none());
        assert_eq!(normalize_windows_path(r"C:\a\.\b\").unwrap(), r"C:\a\b");
        assert!(path_under(r"C:\a\b", r"C:\a"));
        assert!(!path_under(r"C:\ab", r"C:\a"));
    }

    #[cfg(windows)]
    #[test]
    fn canonical_recheck_accepts_real_layout() {
        let root = tempfile::tempdir().expect("temp root");
        let core_dir = root.path().join("core");
        std::fs::create_dir_all(&core_dir).unwrap();
        let exe = core_dir.join("xray.exe");
        std::fs::write(&exe, b"fake-exe").unwrap();
        let spec = ElevatedCoreSpec {
            core: "xray".into(),
            exe_path: exe.to_string_lossy().into_owned(),
            args: vec!["run".into()],
            run_dir: core_dir.to_string_lossy().into_owned(),
        };
        let roots = vec![root.path().to_string_lossy().into_owned()];
        assert!(validate_elevated_core(&spec, &roots).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn canonical_recheck_rejects_symlink_escape() {
        let root = tempfile::tempdir().expect("temp root");
        let core_dir = root.path().join("core");
        std::fs::create_dir_all(&core_dir).unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let real_exe = outside.join("xray.exe");
        std::fs::write(&real_exe, b"fake-exe").unwrap();
        let link = core_dir.join("xray.exe");
        if std::os::windows::fs::symlink_file(&real_exe, &link).is_err() {
            // Creating symlinks needs Developer Mode / privilege; without it
            // the escape cannot be staged, so there is nothing to reject.
            return;
        }
        let spec = ElevatedCoreSpec {
            core: "xray".into(),
            exe_path: link.to_string_lossy().into_owned(),
            args: vec!["run".into()],
            run_dir: core_dir.to_string_lossy().into_owned(),
        };
        let roots = vec![root.path().to_string_lossy().into_owned()];
        assert!(matches!(
            validate_elevated_core(&spec, &roots).unwrap_err(),
            HelperError::PathOutOfBounds { .. }
        ));
    }

    #[test]
    fn job_assign_failed_maps_to_stable_code() {
        assert_eq!(
            HelperError::JobAssignFailed {
                detail: "assign".into()
            }
            .to_domain()
            .code,
            codes::JOB_ASSIGN_FAILED
        );
    }

    #[test]
    fn helper_error_maps_to_stable_codes() {
        assert_eq!(
            HelperError::NotAllowlisted { detail: "x".into() }
                .to_domain()
                .code,
            codes::PERMISSION_DENIED
        );
        assert_eq!(
            HelperError::UnknownHandle { handle: 1 }.to_domain().code,
            codes::NOT_FOUND
        );
        assert_eq!(
            HelperError::Timeout { timeout_ms: 10 }.to_domain().code,
            codes::TIMEOUT
        );
    }
}
