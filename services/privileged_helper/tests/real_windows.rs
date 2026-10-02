//! Real privileged Windows operations (opt-in via `cargo test -- --ignored`).
//!
//! Adds and removes one TEST-NET-2 route (`198.51.100.0/24`) on the loopback
//! interface through the production `WindowsBackend`, then verifies with
//! `Get-NetRoute` that it is gone. Requires an elevated shell.

#![cfg(windows)]

use std::process::Command;
use std::time::Duration;

use ipc_contract::helper::{AddressFamily, RouteEntry};
use privileged_helper::backend::HelperBackend;
use privileged_helper::windows::WindowsBackend;

fn run_powershell(script: &str) -> (bool, String) {
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn powershell");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                return (false, "timeout".into());
            }
        }
    }
    let output = child.wait_with_output().expect("collect");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    )
}

fn loopback_if_index() -> u32 {
    let (ok, out) = run_powershell(
        "(Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -like '*Loopback*' } | Select-Object -First 1).ifIndex",
    );
    assert!(ok, "loopback ifindex query failed");
    out.trim().parse().expect("ifindex parse")
}

struct RouteGuard<'a> {
    backend: &'a WindowsBackend,
    entry: RouteEntry,
}

impl Drop for RouteGuard<'_> {
    fn drop(&mut self) {
        let _ = self
            .backend
            .remove_routes(std::slice::from_ref(&self.entry));
    }
}

#[test]
#[ignore = "requires elevation; adds and removes one TEST-NET route"]
fn real_route_add_remove_roundtrip() {
    let backend = WindowsBackend::new(vec![]);
    assert!(backend.is_elevated(), "test must run elevated");

    let ifindex = loopback_if_index();
    println!("loopback ifindex: {ifindex}");
    let entry = RouteEntry {
        destination: "198.51.100.0/24".to_string(),
        next_hop: "0.0.0.0".to_string(),
        interface_index: ifindex,
        metric: 5000,
        family: AddressFamily::V4,
    };
    let guard = RouteGuard {
        backend: &backend,
        entry: entry.clone(),
    };

    backend
        .add_routes(std::slice::from_ref(&entry))
        .expect("add route");
    let (ok, present) = run_powershell(
        "(Get-NetRoute -DestinationPrefix '198.51.100.0/24' -ErrorAction SilentlyContinue | Measure-Object).Count",
    );
    assert!(ok, "route query failed");
    println!("routes present after add: {present}");
    assert_eq!(present.trim(), "1", "route must exist after add");

    backend
        .remove_routes(std::slice::from_ref(&entry))
        .expect("remove route");
    let (ok, present) = run_powershell(
        "(Get-NetRoute -DestinationPrefix '198.51.100.0/24' -ErrorAction SilentlyContinue | Measure-Object).Count",
    );
    assert!(ok, "route query failed");
    println!("routes present after remove: {present}");
    assert_eq!(present.trim(), "0", "route must be gone after remove");

    drop(guard);
}
