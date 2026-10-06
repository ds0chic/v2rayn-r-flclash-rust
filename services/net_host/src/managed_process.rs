//! Observed exits of managed processes (SP-06).
//!
//! Every managed core is owned through an OS handle (`Child` or a helper
//! handle); observation is handle-authoritative, never a PID scan, so a
//! recycled PID can never cause a miskill. This module builds the structured
//! exit facts; `lifecycle` decides the state transition and `session` applies
//! it to the published detail.

use domain::{codes, DomainError};

/// One observed process exit: the owned identity plus the wait status.
/// `exit_code` is `None` when the platform reports termination without a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedExit {
    pub pid: u32,
    pub exit_code: Option<i32>,
    pub at_ms: i64,
}

/// Redacted one-line exit identity for logs and events (numbers only).
pub fn exit_detail(pid: u32, exit_code: Option<i32>) -> String {
    match exit_code {
        Some(code) => format!("pid={pid} code={code}"),
        None => format!("pid={pid} code=unknown"),
    }
}

/// Structured error for a main core that exited after readiness. Fatal for the
/// session: the caller withdraws the published endpoint.
pub fn main_exit_error(pid: u32, exit_code: Option<i32>) -> DomainError {
    DomainError::new(codes::INTERNAL, "error.core_exited").with_detail(format!(
        "managed core exited ({})",
        exit_detail(pid, exit_code)
    ))
}

/// Structured error for a sidecar that exited while the main core lives. The
/// session degrades; it must not silently keep reporting Running.
pub fn sidecar_exit_error(sidecar_id: &str, exit_code: Option<i32>) -> DomainError {
    let code = match exit_code {
        Some(code) => format!("code={code}"),
        None => "code=unknown".to_string(),
    };
    DomainError::new(codes::INTERNAL, "error.sidecar_exited")
        .with_detail(format!("sidecar `{sidecar_id}` exited ({code})"))
}

#[allow(dead_code)]
fn _codes_anchor() {
    let _ = codes::INTERNAL;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_detail_carries_pid_and_code_without_secrets() {
        assert_eq!(exit_detail(4242, Some(1)), "pid=4242 code=1");
        assert_eq!(exit_detail(4242, None), "pid=4242 code=unknown");
    }

    #[test]
    fn main_exit_is_a_structured_fatal_fact() {
        let error = main_exit_error(42756, Some(1));
        assert_eq!(error.code, codes::INTERNAL);
        assert_eq!(error.message_key, "error.core_exited");
        let detail = error.detail.expect("exit keeps its identity");
        assert!(detail.contains("42756"), "pid is recorded: {detail}");
        assert!(detail.contains('1'), "code is recorded: {detail}");
        assert!(!error.retryable, "an exited core is not retried blindly");
    }

    #[test]
    fn sidecar_exit_names_the_sidecar() {
        let error = sidecar_exit_error("pre-socks", Some(3));
        assert_eq!(error.code, codes::INTERNAL);
        assert_eq!(error.message_key, "error.sidecar_exited");
        let detail = error.detail.expect("exit keeps its identity");
        assert!(
            detail.contains("pre-socks"),
            "sidecar id recorded: {detail}"
        );
    }
}
