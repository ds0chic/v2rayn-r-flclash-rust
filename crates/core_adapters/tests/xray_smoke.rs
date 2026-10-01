//! Optional real-kernel smoke test (ignored by default).
//!
//! Run explicitly with:
//! `cargo test -p core_adapters --test xray_smoke -- --ignored --nocapture`
//!
//! It starts the pinned Xray binary, generates loopback traffic through a
//! minimal SOCKS inbound, polls `/debug/vars` through [`XrayStatsSource`] and
//! then kills only the PID it started. The 60s cap is enforced around the whole
//! body.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::pick_port;
use core_adapters::stats::{StatsSource, XrayStatsSource};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;

fn xray_binary() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest
        .join("..")
        .join("..")
        .join("tools/cores/xray/v26.3.27/xray.exe");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

/// Minimal SOCKS5 no-auth CONNECT to `127.0.0.1:target_port`.
async fn socks5_get(proxy_port: u16, target_port: u16) -> std::io::Result<usize> {
    let mut stream = TcpStream::connect(("127.0.0.1", proxy_port)).await?;
    stream.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut greeting = [0u8; 2];
    stream.read_exact(&mut greeting).await?;
    assert_eq!(greeting, [0x05, 0x00], "socks method selection");

    let mut request = vec![0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1];
    request.extend_from_slice(&target_port.to_be_bytes());
    stream.write_all(&request).await?;
    let mut reply = [0u8; 10];
    stream.read_exact(&mut reply).await?;
    assert_eq!(reply[1], 0x00, "socks connect reply");

    let http = format!(
        "GET /debug/vars HTTP/1.1\r\nHost: 127.0.0.1:{target_port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(http.as_bytes()).await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    Ok(response.len())
}

#[tokio::test]
#[ignore = "starts the real pinned Xray binary; run manually"]
async fn real_xray_stats_smoke() {
    let outcome = tokio::time::timeout(Duration::from_secs(60), {
        async {
            let Some(binary) = xray_binary() else {
                eprintln!("SKIP: pinned xray.exe not found");
                return;
            };
            let socks_port = pick_port();
            let metrics_port = pick_port();
            let temp = tempfile::tempdir().expect("tempdir");
            let config_path = temp.path().join("xray-config.json");
            let config = serde_json::json!({
                "log": {"loglevel": "warning"},
                "inbounds": [{
                    "tag": "socks",
                    "listen": "127.0.0.1",
                    "port": socks_port,
                    "protocol": "socks",
                    "settings": {"udp": false}
                }],
                "outbounds": [
                    {"tag": "proxy", "protocol": "freedom", "settings": {}},
                    {"tag": "direct", "protocol": "freedom", "settings": {}}
                ],
                "stats": {},
                "metrics": {"listen": format!("127.0.0.1:{metrics_port}")},
                "policy": {"system": {"statsOutboundUplink": true, "statsOutboundDownlink": true}}
            });
            std::fs::write(&config_path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();

            let mut child = Command::new(&binary)
                .arg("run")
                .arg("-c")
                .arg(&config_path)
                .current_dir(binary.parent().unwrap())
                .kill_on_drop(true)
                .spawn()
                .expect("spawn xray");
            let pid = child.id();
            let command = format!("{} run -c {}", binary.display(), config_path.display());
            eprintln!("xray pid={pid:?} command={command}");

            let result = async {
                let mut source =
                    XrayStatsSource::new(metrics_port, Duration::from_secs(2)).unwrap();
                // Wait for the metrics endpoint to come up.
                for _ in 0..100 {
                    if source.poll().await.is_ok() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                // Generate proxy traffic through the SOCKS inbound.
                for _ in 0..3 {
                    let _ = socks5_get(socks_port, metrics_port).await;
                }
                for _ in 0..50 {
                    let samples = source.poll().await.expect("poll");
                    if let Some(proxy) = samples.iter().find(|s| s.tag == "proxy") {
                        if proxy.up > 0 || proxy.down > 0 {
                            eprintln!("smoke samples: {samples:?}");
                            assert_eq!(source.generation(), 0);
                            return;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                panic!("real Xray did not report proxy traffic");
            }
            .await;

            let _ = child.kill().await;
            let _ = child.wait().await;
            eprintln!("xray pid={pid:?} stopped");
            result
        }
    })
    .await;
    outcome.expect("smoke test exceeded 60s");
}
