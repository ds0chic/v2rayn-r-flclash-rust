//! `SocksFmt` — `socks://`, `socks5://`, `socks4://`.

use domain::{ConfigType, Profile};

use super::base::{self, SOCKS};
use crate::error::SubError;
use crate::util::{base64_decode, base64_encode_nopad, url_decode, url_encode};

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let trimmed = input.trim();
    // A URI-shaped `socks://` link gets a usable port from the authority; the
    // legacy base64 form has no `@` so the authority is not a host at all and
    // is handed to the legacy decoder.
    if let Some(item) = parse_new(trimmed)? {
        if !item.address.is_empty() && item.port > 0 {
            return Ok(item);
        }
    }
    if let Some(item) = parse_legacy(trimmed) {
        if item.address.is_empty() || item.port == 0 {
            return Err(SubError::InvalidConfig("socks fields".into()));
        }
        return Ok(item);
    }
    Err(SubError::InvalidConfig("socks".into()))
}

pub fn emit(item: &Profile) -> Result<String, SubError> {
    let remark = if item.remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(&item.remarks))
    };
    let pw = base64_encode_nopad(&format!("{}:{}", item.username, item.password));
    Ok(base::build_uri(
        SOCKS,
        &item.address,
        item.port,
        &pw,
        &[],
        &remark,
    ))
}

fn parse_new(input: &str) -> Result<Option<Profile>, SubError> {
    let Ok(url) = url::Url::parse(input) else {
        return Ok(None);
    };
    let scheme = url.scheme();
    if scheme != "socks" && scheme != "socks5" && scheme != "socks4" {
        return Ok(None);
    }
    let mut item = Profile {
        config_type: ConfigType::Socks,
        remarks: url_decode(url.fragment().unwrap_or("")),
        address: base::host_addr(&url),
        port: url.port().map(i32::from).unwrap_or(0),
        ..Profile::default()
    };
    let mut raw_user = url.username().to_string();
    if let Some(password) = url.password() {
        raw_user.push(':');
        raw_user.push_str(password);
    }
    let raw_user = url_decode(&raw_user);
    if !raw_user.is_empty() {
        let parts = if raw_user.contains(':') {
            raw_user
                .split_once(':')
                .map(|(u, p)| (u.to_string(), p.to_string()))
        } else {
            base64_decode(&raw_user).ok().and_then(|decoded| {
                decoded
                    .split_once(':')
                    .map(|(u, p)| (u.to_string(), p.to_string()))
            })
        };
        if let Some((user, pass)) = parts {
            item.username = user;
            item.password = pass;
        }
    }
    Ok(Some(item))
}

fn parse_legacy(input: &str) -> Option<Profile> {
    let mut result = input.strip_prefix(SOCKS)?.to_string();
    let mut item = Profile {
        config_type: ConfigType::Socks,
        ..Profile::default()
    };
    if let Some(index) = result.find('#') {
        item.remarks = url_decode(&result[index + 1..]);
        result.truncate(index);
    }
    if !result.contains('@') {
        result = base64_decode(&result).ok()?;
    }
    let parts: Vec<&str> = result.split('@').collect();
    if parts.len() != 2 {
        return None;
    }
    let user_parts: Vec<&str> = parts[0].split(':').collect();
    let index_port = parts[1].rfind(':')?;
    if user_parts.len() != 2 {
        return None;
    }
    item.address = parts[1][..index_port].to_string();
    item.port = parts[1][index_port + 1..].parse().ok()?;
    item.username = user_parts[0].to_string();
    item.password = user_parts[1].to_string();
    Some(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_userinfo() {
        let item = Profile {
            config_type: ConfigType::Socks,
            remarks: "socks demo".into(),
            address: "127.0.0.1".into(),
            port: 1080,
            username: "user".into(),
            password: "pass".into(),
            ..Profile::default()
        };
        let uri = emit(&item).unwrap();
        assert!(uri.starts_with("socks://"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.address, "127.0.0.1");
        assert_eq!(parsed.username, "user");
        assert_eq!(parsed.password, "pass");
        assert_eq!(parsed.remarks, "socks demo");
    }

    #[test]
    fn legacy_base64_socks_is_parsed() {
        let payload = crate::util::base64_encode("u:p@example.com:1080");
        let parsed = parse(&format!("socks://{payload}#tag")).unwrap();
        assert_eq!(parsed.address, "example.com");
        assert_eq!(parsed.port, 1080);
        assert_eq!(parsed.username, "u");
    }
}
