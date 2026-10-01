//! Autostart backends (Windows Run key).
//!
//! Upstream `AutoStartupHandler` writes either a scheduled task (when running
//! elevated) or an `HKCU\...\Run` value. This task implements the non-elevated
//! Run-key path behind a trait, with an in-memory fake for tests. The
//! scheduled-task path is deferred to the wiring stage.

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub mod fake;
#[cfg(windows)]
pub mod windows;

pub use fake::FakeRegistry;

/// Value-name prefix used by the upstream Windows Run entry.
pub const AUTO_RUN_NAME: &str = "v2rayNAutoRun";

/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
pub const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// A name/command pair stored in the Run key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoStartEntry {
    pub name: String,
    pub command: String,
}

impl AutoStartEntry {
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
        }
    }
}

/// Decoded form of a Run value command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCommand {
    pub exe: String,
    pub args: String,
}

/// Quote the executable and append raw arguments, mirroring upstream
/// `AppendQuotes`.
pub fn encode_run_command(exe: &str, args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        format!("\"{exe}\"")
    } else {
        format!("\"{exe}\" {args}")
    }
}

/// Parse a Run value command produced by [`encode_run_command`]. Supports both
/// the quoted form and a bare, space-separated executable.
pub fn decode_run_command(command: &str) -> Option<RunCommand> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    if let Some(rest) = command.strip_prefix('"') {
        let end = rest.find('"')?;
        let exe = rest[..end].to_string();
        let args = rest[end + 1..].trim().to_string();
        Some(RunCommand { exe, args })
    } else {
        let mut parts = command.splitn(2, ' ');
        let exe = parts.next().unwrap_or_default().to_string();
        let args = parts.next().unwrap_or_default().trim().to_string();
        if exe.is_empty() {
            None
        } else {
            Some(RunCommand { exe, args })
        }
    }
}

/// Build the Run value name `v2rayNAutoRun_<md5(startup_path)>`, matching the
/// upstream `GetAutoRunNameWindows` scheme.
pub fn run_value_name(startup_path: &str) -> String {
    format!(
        "{AUTO_RUN_NAME}_{}",
        crate::hash::md5_hex(startup_path.as_bytes())
    )
}

/// Registry-backed autostart storage.
pub trait AutoStartBackend {
    /// Raw value for `name`, or `None` when absent/empty.
    fn query(&self, name: &str) -> Result<Option<String>>;

    /// Write `command` under `name`.
    fn set(&self, name: &str, command: &str) -> Result<()>;

    /// Remove `name`. Must be idempotent.
    fn remove(&self, name: &str) -> Result<()>;

    /// Whether a non-empty value exists.
    fn is_enabled(&self, name: &str) -> Result<bool> {
        Ok(self.query(name)?.is_some_and(|value| !value.is_empty()))
    }

    /// Enable autostart with a Run command. `exe` is quoted automatically.
    fn enable(&self, name: &str, exe: &str, args: &str) -> Result<()> {
        self.set(name, &encode_run_command(exe, args))
    }

    /// Disable autostart.
    fn disable(&self, name: &str) -> Result<()> {
        self.remove(name)
    }
}
