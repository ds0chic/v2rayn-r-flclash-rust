//! Deterministic synthetic data used by the T01 table harness and T02 tests.
//!
//! No real subscription, credential or network access is performed.

use std::time::{Duration, Instant};

use domain::{ConfigType, CoreType, ProfileSummary};

/// RFC 5737 documentation address blocks. Never routable, safe as fixtures.
const IP_BLOCKS: [[u8; 3]; 3] = [[192, 0, 2], [198, 51, 100], [203, 0, 113]];

/// RFC 2606 reserved example domains.
const DOMAINS: [&str; 3] = ["example.com", "example.org", "example.net"];

const NETWORKS: [&str; 4] = ["tcp", "ws", "grpc", "http"];
const SECURITIES: [&str; 3] = ["none", "tls", "reality"];

/// The subset of config types the T01 probe table renders.
pub const CONFIG_TYPES: [ConfigType; 8] = [
    ConfigType::Vmess,
    ConfigType::Vless,
    ConfigType::Trojan,
    ConfigType::Shadowsocks,
    ConfigType::Socks,
    ConfigType::Http,
    ConfigType::Hysteria2,
    ConfigType::Tuic,
];

/// The subset of cores the T01 probe table renders.
pub const CORE_TYPES: [CoreType; 3] = [CoreType::Xray, CoreType::SingBox, CoreType::Mihomo];

/// SplitMix64 finalizer, used as a cheap deterministic per-row hash so paging
/// is order independent and reproducible across calls.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn pick<T>(items: &[T], h: u64) -> &T {
    &items[(h as usize) % items.len()]
}

/// Build a single deterministic synthetic row. Pure function of `index`.
pub fn synthetic_profile(index: u32) -> ProfileSummary {
    let h0 = mix(index as u64);
    let h1 = mix(h0);
    let h2 = mix(h1);
    let h3 = mix(h2);

    let block = &IP_BLOCKS[(h0 as usize) % IP_BLOCKS.len()];
    let octet = 1 + (h1 % 254) as u8;
    let address = match h2 % 2 {
        0 => format!("{}.{}.{}.{}", block[0], block[1], block[2], octet),
        _ => format!("node{:05}.{}", index, pick(&DOMAINS, h2)),
    };

    let config_type = *pick(&CONFIG_TYPES, h0);
    let core_type = *pick(&CORE_TYPES, h1);
    let network = *pick(&NETWORKS, h2);
    let security = *pick(&SECURITIES, h3);

    let delay = if h1.is_multiple_of(11) {
        -1
    } else {
        (h3 % 400) as i32
    };
    let speed = if delay < 0 {
        "-".to_string()
    } else {
        format!("{:.2} MB/s", (h2 % 5000) as f64 / 100.0)
    };

    let gb = 1_073_741_824u64;
    let today_up = (h0 % 50) * gb / 100;
    let today_down = (h1 % 500) * gb / 10;
    let total_up = (h2 % 400) * gb;
    let total_down = (h3 % 4000) * gb;
    let ip_info = if h2.is_multiple_of(5) {
        "-".to_string()
    } else {
        format!("{}.{}.{}.{}", block[0], block[1], block[2], octet)
    };

    ProfileSummary {
        id: format!("syn-{index:06}"),
        config_type,
        remarks: format!("Synthetic-{index:05}"),
        address,
        port: 10_000 + (h1 % 50_000) as u16,
        network: network.to_string(),
        stream_security: security.to_string(),
        sub_remarks: format!("sub-{:03}", index / 100),
        delay,
        speed,
        today_up,
        ip_info,
        today_down,
        total_up,
        total_down,
        core_type,
    }
}

/// Generate `count` synthetic rows.
pub fn generate_synthetic_profiles(count: u32) -> Vec<ProfileSummary> {
    (0..count).map(synthetic_profile).collect()
}

/// Generate one page of synthetic rows with the same deterministic layout as
/// [`generate_synthetic_profiles`].
pub fn generate_profiles_page(offset: u32, limit: u32) -> Vec<ProfileSummary> {
    (offset..offset.saturating_add(limit))
        .map(synthetic_profile)
        .collect()
}

/// Deterministic ping simulation. Returns the same latency as the generated
/// delay so UI and Rust can be cross-checked without touching the network.
pub fn ping(profile: &ProfileSummary) -> i32 {
    if profile.delay < 0 {
        0
    } else {
        profile.delay
    }
}

/// Occupy a thread for `ms` milliseconds and return the measured elapsed time.
/// Used to prove an FRB call runs off the UI thread.
pub fn blocking_probe(ms: u64) -> u64 {
    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(ms));
    start.elapsed().as_millis() as u64
}

/// Build a full [`domain::Profile`] from a synthetic summary index. Used to
/// seed the in-memory repository contract tests.
pub fn synthetic_full_profile(index: u32) -> domain::Profile {
    let summary = synthetic_profile(index);
    let mut profile = domain::Profile {
        index_id: summary.id,
        config_type: summary.config_type,
        core_type: Some(summary.core_type),
        remarks: summary.remarks,
        address: summary.address,
        port: summary.port as i32,
        network: summary.network,
        ..Default::default()
    };
    profile.security.stream_security = if summary.stream_security == "none" {
        None
    } else {
        Some(summary.stream_security)
    };
    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_requested_count() {
        let rows = generate_synthetic_profiles(10_000);
        assert_eq!(rows.len(), 10_000);
    }

    #[test]
    fn synthetic_data_uses_reserved_ranges_only() {
        for row in generate_synthetic_profiles(2_000) {
            assert!(row.id.starts_with("syn-"));
            let address_ok = row.address.starts_with("192.0.2.")
                || row.address.starts_with("198.51.100.")
                || row.address.starts_with("203.0.113.")
                || row.address.contains(".example.com")
                || row.address.contains(".example.org")
                || row.address.contains(".example.net");
            assert!(address_ok, "unexpected synthetic address: {}", row.address);
        }
    }

    #[test]
    fn paging_matches_full_generation() {
        let full = generate_synthetic_profiles(100);
        let page = generate_profiles_page(10, 20);
        assert_eq!(page, full[10..30].to_vec());
    }

    #[test]
    fn blocking_probe_measures_elapsed() {
        let elapsed = blocking_probe(30);
        assert!(elapsed >= 25, "elapsed too small: {elapsed}");
    }

    #[test]
    fn synthetic_full_profile_is_valid() {
        let p = synthetic_full_profile(7);
        assert!(p.validate().is_ok(), "{:?}", p.validate());
    }
}
