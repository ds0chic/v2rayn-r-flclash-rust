//! T03 probe: exercise the real `NetHostClient` without Flutter/FRB.

use application::{NetHostClient, RuntimeClient};

fn main() {
    let client = NetHostClient::new();
    eprintln!("[probe] pipe={}", client.pipe_name());
    match client.snapshot() {
        Ok(snapshot) => println!(
            "SNAPSHOT state={:?} host_alive={} pid={:?} ports={:?}",
            snapshot.state, snapshot.host_alive, snapshot.pid, snapshot.ports
        ),
        Err(error) => println!("SNAPSHOT_ERROR {} {}", error.code, error.message_key),
    }
    match client.stop() {
        Ok(()) => println!("STOP_OK"),
        Err(error) => println!("STOP_ERROR {} {}", error.code, error.message_key),
    }
}
