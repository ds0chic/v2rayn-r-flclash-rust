//! `upgrade_runner` — external self-update helper for v2rayN-R (T21).
//!
//! Windows cannot replace files that the running application has open, so the
//! app hands the swap to this separate process. The runner:
//!
//! 1. waits for the target PID to exit (bounded timeout);
//! 2. loads the [`updater::InstallPlan`] the app produced;
//! 3. performs the atomic swap with [`updater::apply_atomic`], keeping the old
//!    version as a rollback copy;
//! 4. on failure, restores the previous version (no partially-applied state);
//! 5. restarts the newly installed executable;
//! 6. writes a machine-readable result JSON.
//!
//! It never touches anything outside the plan's managed root, never reads the
//! environment proxy, and only waits on the PID it was given.

#![cfg_attr(not(windows), allow(dead_code))]

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use updater::install::{
    apply_atomic, apply_atomic_inject, restore_previous, FailPoint, InstallPlan,
};

const DEFAULT_WAIT_SECS: u64 = 60;
const DEFAULT_MANIFEST_NAME: &str = "install-manifest.json";

/// Parsed command line.
#[derive(Debug, Default)]
struct Args {
    plan: Option<PathBuf>,
    result: Option<PathBuf>,
    pid: Option<u32>,
    wait_timeout_secs: u64,
    restart_exe: Option<PathBuf>,
    restart_cwd: Option<PathBuf>,
    manifest_name: String,
    fail_inject: String,
    no_restart: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        wait_timeout_secs: DEFAULT_WAIT_SECS,
        manifest_name: DEFAULT_MANIFEST_NAME.to_string(),
        fail_inject: "none".to_string(),
        ..Args::default()
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("missing value for {flag}"));
        match flag.as_str() {
            "--plan" => args.plan = Some(PathBuf::from(value()?)),
            "--result" => args.result = Some(PathBuf::from(value()?)),
            "--pid" => {
                args.pid = Some(value()?.parse().map_err(|e| format!("bad --pid: {e}"))?);
            }
            "--wait-timeout-secs" => {
                args.wait_timeout_secs = value()?
                    .parse()
                    .map_err(|e| format!("bad --wait-timeout-secs: {e}"))?;
            }
            "--restart-exe" => args.restart_exe = Some(PathBuf::from(value()?)),
            "--restart-cwd" => args.restart_cwd = Some(PathBuf::from(value()?)),
            "--manifest-name" => args.manifest_name = value()?,
            "--fail-inject" => args.fail_inject = value()?,
            "--no-restart" => args.no_restart = true,
            "-h" | "--help" => {
                println!(
                    "upgrade_runner --plan <json> --result <json> [--pid <n>] \
                     [--wait-timeout-secs <n>] [--restart-exe <path>] [--restart-cwd <dir>] \
                     [--manifest-name <name>] [--fail-inject none|stage|commit] [--no-restart]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if args.plan.is_none() {
        return Err("--plan is required".to_string());
    }
    if args.result.is_none() {
        return Err("--result is required".to_string());
    }
    Ok(args)
}

/// Outcome of waiting on the target process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WaitOutcome {
    Exited,
    AlreadyExited,
    TimedOut,
}

impl WaitOutcome {
    fn timed_out(self) -> bool {
        matches!(self, WaitOutcome::TimedOut)
    }
}

#[cfg(windows)]
fn wait_for_process_exit(pid: u32, timeout: Duration) -> WaitOutcome {
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    // SAFETY: the handle is opened only for synchronisation, waited on and then
    // closed; no pointer is dereferenced.
    unsafe {
        let handle = match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(handle) => handle,
            // A failed open almost always means the process is already gone.
            Err(_) => return WaitOutcome::AlreadyExited,
        };
        let millis = timeout.as_millis().min(u32::MAX as u128) as u32;
        let waited = WaitForSingleObject(handle, millis);
        let _ = CloseHandle(handle);
        if waited == WAIT_TIMEOUT {
            WaitOutcome::TimedOut
        } else if waited == WAIT_OBJECT_0 {
            WaitOutcome::Exited
        } else {
            WaitOutcome::AlreadyExited
        }
    }
}

#[cfg(not(windows))]
fn wait_for_process_exit(pid: u32, timeout: Duration) -> WaitOutcome {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            return WaitOutcome::Exited;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    WaitOutcome::TimedOut
}

fn fail_point(name: &str) -> Result<FailPoint, String> {
    match name {
        "none" => Ok(FailPoint::None),
        "stage" => Ok(FailPoint::StageRename),
        "commit" => Ok(FailPoint::CommitRename),
        other => Err(format!("bad --fail-inject: {other}")),
    }
}

fn spawn_restart(exe: &PathBuf, cwd: &PathBuf) -> Result<u32, String> {
    let mut command = Command::new(exe);
    command.current_dir(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: the restarted app must
        // outlive this helper and not receive its console control events.
        command.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    let child = command
        .spawn()
        .map_err(|e| format!("restart {exe:?}: {e}"))?;
    Ok(child.id())
}

/// Result document written to `--result`.
#[derive(Debug, Serialize)]
struct RunnerResult {
    schema: u32,
    ok: bool,
    stage: String,
    version: String,
    waited_for_pid: Option<u32>,
    wait_outcome: WaitOutcome,
    install_root: String,
    current_dir: String,
    staged_dir: String,
    kept_previous: Option<String>,
    manifest_written: Option<String>,
    restarted: bool,
    restarted_pid: Option<u32>,
    restart_exe: Option<String>,
    restored_previous: bool,
    error: Option<String>,
    started_utc: String,
    finished_utc: String,
    duration_ms: u128,
}

fn write_result(path: &PathBuf, result: &RunnerResult) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_vec_pretty(result) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(path, bytes) {
                eprintln!(
                    "upgrade_runner: cannot write result {}: {e}",
                    path.display()
                );
            }
        }
        Err(e) => eprintln!("upgrade_runner: cannot serialise result: {e}"),
    }
}

fn main() {
    let started = SystemTime::now();
    let start_instant = Instant::now();
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("upgrade_runner: {e}");
            std::process::exit(64);
        }
    };
    let result_path = args.result.clone().expect("result checked");
    let plan_path = args.plan.clone().expect("plan checked");

    let mut result = RunnerResult {
        schema: 1,
        ok: false,
        stage: "init".to_string(),
        version: String::new(),
        waited_for_pid: args.pid,
        wait_outcome: WaitOutcome::Exited,
        install_root: String::new(),
        current_dir: String::new(),
        staged_dir: String::new(),
        kept_previous: None,
        manifest_written: None,
        restarted: false,
        restarted_pid: None,
        restart_exe: args.restart_exe.as_ref().map(|p| p.display().to_string()),
        restored_previous: false,
        error: None,
        started_utc: utc_iso(started),
        finished_utc: String::new(),
        duration_ms: 0,
    };

    let plan: InstallPlan = match std::fs::read(&plan_path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
    {
        Ok(plan) => plan,
        Err(e) => {
            result.stage = "load-plan".into();
            result.error = Some(format!("load plan {}: {e}", plan_path.display()));
            finish(&result_path, &mut result, start_instant);
        }
    };
    result.version = plan.version.clone();
    result.install_root = plan.root.display().to_string();
    result.current_dir = plan.current_dir.display().to_string();
    result.staged_dir = plan.staged_dir.display().to_string();

    if let Some(pid) = args.pid.filter(|p| *p != 0) {
        result.stage = "wait".into();
        let outcome =
            wait_for_process_exit(pid, Duration::from_secs(args.wait_timeout_secs.max(1)));
        result.wait_outcome = outcome;
        if outcome.timed_out() {
            result.error = Some(format!(
                "target pid {pid} still running after {}s",
                args.wait_timeout_secs
            ));
            finish(&result_path, &mut result, start_instant);
        }
    }

    result.stage = "apply".into();
    let injected = match fail_point(&args.fail_inject) {
        Ok(point) => point,
        Err(e) => {
            result.error = Some(e);
            finish(&result_path, &mut result, start_instant);
        }
    };
    let apply_result = if injected == FailPoint::None {
        apply_atomic(&plan)
    } else {
        apply_atomic_inject(&plan, injected)
    };

    match apply_result {
        Ok(outcome) => {
            result.kept_previous = outcome
                .kept_previous
                .as_ref()
                .map(|p| p.display().to_string());
            if !args.manifest_name.is_empty() {
                let manifest_path = plan.current_dir.join(&args.manifest_name);
                match serde_json::to_vec_pretty(&outcome.manifest) {
                    Ok(bytes) => match std::fs::write(&manifest_path, bytes) {
                        Ok(()) => {
                            result.manifest_written = Some(manifest_path.display().to_string())
                        }
                        Err(e) => {
                            result.error = Some(format!("write manifest: {e}"));
                            finish(&result_path, &mut result, start_instant);
                        }
                    },
                    Err(e) => {
                        result.error = Some(format!("serialise manifest: {e}"));
                        finish(&result_path, &mut result, start_instant);
                    }
                }
            }
            if !args.no_restart {
                if let Some(exe) = args.restart_exe.clone() {
                    let cwd = args
                        .restart_cwd
                        .clone()
                        .unwrap_or_else(|| plan.current_dir.clone());
                    result.stage = "restart".into();
                    match spawn_restart(&exe, &cwd) {
                        Ok(pid) => {
                            result.restarted = true;
                            result.restarted_pid = Some(pid);
                        }
                        Err(e) => {
                            result.error = Some(e);
                            finish(&result_path, &mut result, start_instant);
                        }
                    }
                }
            }
            result.stage = "done".into();
            result.ok = true;
            finish(&result_path, &mut result, start_instant);
        }
        Err(error) => {
            result.stage = "apply".into();
            match restore_previous(&plan) {
                Ok(true) => result.restored_previous = true,
                Ok(false) => {}
                Err(e) => result.error = Some(format!("{error}; rollback failed: {e}")),
            }
            if result.error.is_none() {
                result.error = Some(error.to_string());
            }
            finish(&result_path, &mut result, start_instant);
        }
    }
}

fn finish(result_path: &PathBuf, result: &mut RunnerResult, start: Instant) -> ! {
    result.finished_utc = utc_iso(SystemTime::now());
    result.duration_ms = start.elapsed().as_millis();
    write_result(result_path, result);
    if result.ok {
        std::process::exit(0);
    }
    std::process::exit(1);
}

/// Minimal UTC ISO-8601 formatter (no chrono dependency).
fn utc_iso(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's `civil_from_days` (days since 1970-01-01).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn iso_formats_epoch() {
        assert_eq!(
            utc_iso(UNIX_EPOCH + Duration::from_secs(0)),
            "1970-01-01T00:00:00Z"
        );
    }
}
