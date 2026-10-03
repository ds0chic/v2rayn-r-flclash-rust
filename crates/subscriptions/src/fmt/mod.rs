//! `FmtHandler` dispatch and the per-protocol codecs (upstream `Handler/Fmt`).

pub mod anytls;
pub mod base;
pub mod batch;
pub mod hysteria2;
pub mod inner;
pub mod naive;
pub mod shadowsocks;
pub mod socks;
pub mod trojan;
pub mod tuic;
pub mod vless;
pub mod vmess;
pub mod wire;
pub mod wireguard;

use domain::ConfigType;

use crate::error::SubError;

pub use hysteria2::HyRealm;

/// The upstream `fmt_formats` families covered by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FmtKind {
    Vmess,
    Vless,
    Shadowsocks,
    Socks,
    Trojan,
    Hysteria2,
    Tuic,
    WireGuard,
    Anytls,
    Naive,
    Inner,
    Sip008,
    V2ray,
    Singbox,
    Clash,
    HtmlPage,
    Base64List,
}

impl FmtKind {
    pub const ALL: [FmtKind; 17] = [
        FmtKind::Vmess,
        FmtKind::Vless,
        FmtKind::Shadowsocks,
        FmtKind::Socks,
        FmtKind::Trojan,
        FmtKind::Hysteria2,
        FmtKind::Tuic,
        FmtKind::WireGuard,
        FmtKind::Anytls,
        FmtKind::Naive,
        FmtKind::Inner,
        FmtKind::Sip008,
        FmtKind::V2ray,
        FmtKind::Singbox,
        FmtKind::Clash,
        FmtKind::HtmlPage,
        FmtKind::Base64List,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            FmtKind::Vmess => "vmess",
            FmtKind::Vless => "vless",
            FmtKind::Shadowsocks => "shadowsocks",
            FmtKind::Socks => "socks",
            FmtKind::Trojan => "trojan",
            FmtKind::Hysteria2 => "hysteria2",
            FmtKind::Tuic => "tuic",
            FmtKind::WireGuard => "wireguard",
            FmtKind::Anytls => "anytls",
            FmtKind::Naive => "naive",
            FmtKind::Inner => "inner",
            FmtKind::Sip008 => "sip008",
            FmtKind::V2ray => "v2ray",
            FmtKind::Singbox => "singbox",
            FmtKind::Clash => "clash",
            FmtKind::HtmlPage => "htmlpage",
            FmtKind::Base64List => "base64list",
        }
    }

    /// The share-URI kind for a profile's config type, when exportable.
    pub const fn of_config_type(config_type: ConfigType) -> Option<FmtKind> {
        Some(match config_type {
            ConfigType::Vmess => FmtKind::Vmess,
            ConfigType::Shadowsocks => FmtKind::Shadowsocks,
            ConfigType::Socks => FmtKind::Socks,
            ConfigType::Vless => FmtKind::Vless,
            ConfigType::Trojan => FmtKind::Trojan,
            ConfigType::Hysteria2 => FmtKind::Hysteria2,
            ConfigType::Tuic => FmtKind::Tuic,
            ConfigType::WireGuard => FmtKind::WireGuard,
            ConfigType::Anytls => FmtKind::Anytls,
            ConfigType::Naive => FmtKind::Naive,
            _ => return None,
        })
    }
}

/// `FmtHandler.ResolveConfig`: parse a single share URI / line.
pub fn resolve_uri(input: &str) -> Result<domain::Profile, SubError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(SubError::Empty);
    }
    if trimmed.starts_with(base::VMESS) {
        vmess::parse(trimmed)
    } else if trimmed.starts_with(base::SS) {
        shadowsocks::parse(trimmed)
    } else if trimmed.starts_with(base::SOCKS)
        || trimmed.starts_with(base::SOCKS5)
        || trimmed.starts_with(base::SOCKS4)
    {
        socks::parse(trimmed)
    } else if trimmed.starts_with(base::TROJAN) {
        trojan::parse(trimmed)
    } else if trimmed.starts_with(base::VLESS) {
        vless::parse(trimmed)
    } else if trimmed.starts_with(base::HYSTERIA2) || trimmed.starts_with(base::HY2_ALIAS) {
        hysteria2::parse(trimmed)
    } else if trimmed.starts_with(base::HY2_REALM) || trimmed.starts_with(base::HY2_REALM_HTTP) {
        hysteria2::parse_realm(trimmed)
    } else if trimmed.starts_with(base::TUIC) {
        tuic::parse(trimmed)
    } else if trimmed.starts_with(base::WIREGUARD) {
        wireguard::parse(trimmed)
    } else if trimmed.starts_with(base::ANYTLS) {
        anytls::parse(trimmed)
    } else if trimmed.starts_with(base::NAIVE)
        || trimmed.starts_with(base::NAIVE_HTTPS)
        || trimmed.starts_with(base::NAIVE_QUIC)
    {
        naive::parse(trimmed)
    } else {
        Err(SubError::Unsupported("not a share URI".into()))
    }
}

/// `FmtHandler.GetShareUri`: encode a profile back to its share URI.
pub fn to_uri(item: &domain::Profile) -> Result<String, SubError> {
    match item.config_type {
        ConfigType::Vmess => vmess::emit(item),
        ConfigType::Shadowsocks => shadowsocks::emit(item),
        ConfigType::Socks => socks::emit(item),
        ConfigType::Vless => vless::emit(item),
        ConfigType::Trojan => trojan::emit(item),
        ConfigType::Hysteria2 => hysteria2::emit(item),
        ConfigType::Tuic => tuic::emit(item),
        ConfigType::WireGuard => wireguard::emit(item),
        ConfigType::Anytls => anytls::emit(item),
        ConfigType::Naive => naive::emit(item),
        other => Err(SubError::Unsupported(format!(
            "config type {} has no share URI",
            other.as_str()
        ))),
    }
}

/// `InnerFmt` export: encode profiles as `v2rayn://` inner URIs.
///
/// Returns the newline-joined URI list, or `None` when nothing is exportable.
pub fn to_inner_uri(items: &[domain::Profile]) -> Option<String> {
    inner::emit(items)
}

/// [`to_inner_uri`] with a resolver for file-backed `Outbound` addresses.
///
/// The loader maps an `Address` file path to its parsed JSON content; the
/// pure [`to_inner_uri`] skips such nodes, like upstream's `null` when the
/// file is missing.
pub fn to_inner_uri_with_outbound_loader(
    items: &[domain::Profile],
    outbound_loader: wire::OutboundLoader<'_>,
) -> Option<String> {
    inner::emit_with(items, outbound_loader)
}

/// Best-effort identification of a single line's format.
pub fn fmt_kind_of(input: &str) -> Option<FmtKind> {
    let trimmed = input.trim();
    if trimmed.starts_with(base::VMESS) {
        Some(FmtKind::Vmess)
    } else if trimmed.starts_with(base::SS) {
        Some(FmtKind::Shadowsocks)
    } else if trimmed.starts_with(base::SOCKS)
        || trimmed.starts_with(base::SOCKS5)
        || trimmed.starts_with(base::SOCKS4)
    {
        Some(FmtKind::Socks)
    } else if trimmed.starts_with(base::TROJAN) {
        Some(FmtKind::Trojan)
    } else if trimmed.starts_with(base::VLESS) {
        Some(FmtKind::Vless)
    } else if trimmed.starts_with(base::HYSTERIA2)
        || trimmed.starts_with(base::HY2_ALIAS)
        || trimmed.starts_with(base::HY2_REALM)
        || trimmed.starts_with(base::HY2_REALM_HTTP)
    {
        Some(FmtKind::Hysteria2)
    } else if trimmed.starts_with(base::TUIC) {
        Some(FmtKind::Tuic)
    } else if trimmed.starts_with(base::WIREGUARD) {
        Some(FmtKind::WireGuard)
    } else if trimmed.starts_with(base::ANYTLS) {
        Some(FmtKind::Anytls)
    } else if trimmed.starts_with(base::NAIVE)
        || trimmed.starts_with(base::NAIVE_HTTPS)
        || trimmed.starts_with(base::NAIVE_QUIC)
    {
        Some(FmtKind::Naive)
    } else if trimmed
        .get(..base::INNER.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(base::INNER))
    {
        Some(FmtKind::Inner)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_line_is_rejected() {
        assert!(matches!(
            resolve_uri("not-a-share-uri"),
            Err(SubError::Unsupported(_))
        ));
        assert!(matches!(resolve_uri("   "), Err(SubError::Empty)));
    }

    #[test]
    fn group_and_http_have_no_share_uri() {
        let group = domain::Profile {
            config_type: ConfigType::PolicyGroup,
            remarks: "group".into(),
            ..domain::Profile::default()
        };
        assert!(to_uri(&group).is_err());
    }

    #[test]
    fn kind_detection_covers_prefixes() {
        assert_eq!(fmt_kind_of("hy2://a@b:443"), Some(FmtKind::Hysteria2));
        assert_eq!(
            fmt_kind_of("hysteria2+realm://t@h/r"),
            Some(FmtKind::Hysteria2)
        );
        assert_eq!(fmt_kind_of("socks4://a:1"), Some(FmtKind::Socks));
        assert_eq!(fmt_kind_of("v2rayn://vless/AAA"), Some(FmtKind::Inner));
        assert_eq!(fmt_kind_of("http://example.com"), None);
    }
}
