//! Subscription download adapter (`reqwest` + rustls).
//!
//! The client never consults the environment proxy: `no_proxy()` is set unless
//! an explicit [`ProxyConfig`] is supplied. Redirects are followed manually so
//! authentication/cookie headers are dropped when the origin changes, and the
//! body read is capped and observes cooperative cancellation.

use std::time::Duration;

use domain::CancellationToken;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, COOKIE, LOCATION,
    PROXY_AUTHORIZATION, USER_AGENT,
};
use reqwest::redirect::Policy;
use reqwest::Url;

use crate::error::SubError;
use crate::tls::{self, HttpsTrust};
use crate::util::{decode_body_bytes, CancellationWatcher};

/// An explicit proxy endpoint (e.g. `http://127.0.0.1:7890`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub url: String,
}

impl ProxyConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

/// Download behaviour. All fields have safe defaults; none read the
/// environment.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    /// Extra request headers (name, value). Validated at client build time.
    pub headers: Vec<(String, String)>,
    /// User-Agent value; defaults to a generic client token.
    pub user_agent: Option<String>,
    /// Explicit proxy, when the caller wants one.
    pub proxy: Option<ProxyConfig>,
    /// Whole-download timeout, redirects included.
    pub timeout: Duration,
    /// TCP/TLS connect timeout.
    pub connect_timeout: Option<Duration>,
    /// Maximum accepted body size in bytes.
    pub max_bytes: usize,
    /// Maximum number of redirects before failing.
    pub max_redirects: usize,
    /// Accept invalid TLS certificates (opt-in; off by default).
    pub accept_invalid_certs: bool,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            headers: Vec::new(),
            user_agent: None,
            proxy: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Some(Duration::from_secs(10)),
            max_bytes: 16 * 1024 * 1024,
            max_redirects: 10,
            accept_invalid_certs: false,
        }
    }
}

/// A decoded download result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    pub final_url: String,
    pub content_type: Option<String>,
    pub body: String,
    pub bytes: usize,
}

/// Reusable client wrapper.
#[derive(Debug, Clone)]
pub struct Downloader {
    client: reqwest::Client,
    headers: HeaderMap,
    user_agent: Option<HeaderValue>,
    max_bytes: usize,
    max_redirects: usize,
    timeout: Duration,
}

/// Build a [`Downloader`], validating headers and proxy configuration.
///
/// Trust is the OS/native store (upstream `system`); see
/// [`build_client_with_trust`] for the `RootCertProvider` selection.
pub fn build_client(options: &DownloadOptions) -> Result<Downloader, SubError> {
    build_client_with_trust(options, &HttpsTrust::System)
}

/// Build a [`Downloader`] trusting `trust` (SP-25 `RootCertProvider`
/// consumer). `BundledPem` trusts exactly the bundle, never the OS store.
pub fn build_client_with_trust(
    options: &DownloadOptions,
    trust: &HttpsTrust,
) -> Result<Downloader, SubError> {
    let mut headers = HeaderMap::new();
    for (name, value) in &options.headers {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| SubError::HeaderInvalid("name".into()))?;
        if value.chars().any(|c| c.is_control() && c != '\t') {
            return Err(SubError::HeaderInvalid("control character in value".into()));
        }
        let value = HeaderValue::from_str(value)
            .map_err(|_| SubError::HeaderInvalid("invalid value".into()))?;
        headers.append(name, value);
    }

    let user_agent = match options.user_agent.as_deref().filter(|v| !v.is_empty()) {
        Some(value) => {
            if value.chars().any(|c| c.is_control() && c != '\t') {
                return Err(SubError::HeaderInvalid(
                    "control character in user agent".into(),
                ));
            }
            Some(
                HeaderValue::from_str(value)
                    .map_err(|_| SubError::HeaderInvalid("user agent".into()))?,
            )
        }
        None => None,
    };

    let mut builder = tls::apply_trust(
        reqwest::Client::builder()
            .redirect(Policy::none())
            .connect_timeout(options.connect_timeout.unwrap_or(Duration::from_secs(10)))
            .no_proxy(),
        trust,
    )
    .map_err(SubError::Http)?;
    builder = builder.danger_accept_invalid_certs(options.accept_invalid_certs);
    if let Some(proxy) = &options.proxy {
        if !proxy.url.starts_with("http://") && !proxy.url.starts_with("https://") {
            return Err(SubError::InvalidUri("proxy scheme".into()));
        }
        let parsed = reqwest::Proxy::all(&proxy.url)
            .map_err(|e| SubError::InvalidUri(format!("proxy: {e}")))?;
        builder = builder.proxy(parsed);
    }

    let client = builder
        .build()
        .map_err(|e| SubError::Http(format!("client build: {e}")))?;

    Ok(Downloader {
        client,
        headers,
        user_agent,
        max_bytes: options.max_bytes,
        max_redirects: options.max_redirects,
        timeout: options.timeout,
    })
}

fn origin_differs(a: &Url, b: &Url) -> bool {
    a.scheme() != b.scheme()
        || a.host_str() != b.host_str()
        || a.port_or_known_default() != b.port_or_known_default()
}

impl Downloader {
    /// Download `url` and decode the body using its advertised charset.
    pub async fn download(
        &self,
        url: &str,
        cancellation: &CancellationToken,
    ) -> Result<Downloaded, SubError> {
        let parsed = Url::parse(url).map_err(|e| SubError::InvalidUri(e.to_string()))?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(SubError::InvalidUri("scheme must be http(s)".into()));
        }
        let watcher = CancellationWatcher::new(Some(cancellation.clone()));

        let work = self.follow(parsed, &watcher);
        tokio::select! {
            result = tokio::time::timeout(self.timeout, work) => match result {
                Ok(inner) => inner,
                Err(_) => Err(SubError::Timeout),
            },
            _ = wait_for_cancel(&watcher) => Err(SubError::Cancelled),
        }
    }

    async fn follow(
        &self,
        start: Url,
        watcher: &CancellationWatcher,
    ) -> Result<Downloaded, SubError> {
        let mut url = start;
        let mut headers = self.headers.clone();
        for hop in 0..=self.max_redirects {
            watcher.check()?;
            let mut builder = self.client.get(url.clone()).header(ACCEPT, "*/*");
            for (name, value) in headers.iter() {
                builder = builder.header(name, value);
            }
            if let Some(user_agent) = &self.user_agent {
                builder = builder.header(USER_AGENT, user_agent.clone());
            }
            let response = builder.send().await.map_err(|e| map_reqwest_error(&e))?;

            if response.status().is_redirection() {
                if hop == self.max_redirects {
                    return Err(SubError::Http("too many redirects".into()));
                }
                let Some(location) = response.headers().get(LOCATION) else {
                    return Err(SubError::Http("redirect without Location".into()));
                };
                let location = location
                    .to_str()
                    .map_err(|_| SubError::Http("invalid Location".into()))?;
                let next = url
                    .join(location)
                    .map_err(|e| SubError::Http(format!("bad redirect: {e}")))?;
                if origin_differs(&url, &next) {
                    // Do not leak credentials across origins.
                    headers.remove(AUTHORIZATION);
                    headers.remove(COOKIE);
                    headers.remove(PROXY_AUTHORIZATION);
                }
                url = next;
                continue;
            }

            if !response.status().is_success() {
                return Err(SubError::Http(format!(
                    "status {}",
                    response.status().as_u16()
                )));
            }
            let final_url = response.url().to_string();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let body = read_body_capped(response, self.max_bytes, watcher).await?;
            let text = decode_body_bytes(&body, content_type.as_deref());
            return Ok(Downloaded {
                final_url,
                content_type,
                body: text,
                bytes: body.len(),
            });
        }
        Err(SubError::Http("too many redirects".into()))
    }
}

/// Download a string with a freshly built client (convenience wrapper).
pub async fn download_string(
    url: &str,
    options: &DownloadOptions,
    cancellation: &CancellationToken,
) -> Result<Downloaded, SubError> {
    let downloader = build_client(options)?;
    downloader.download(url, cancellation).await
}

async fn read_body_capped(
    mut response: reqwest::Response,
    max_bytes: usize,
    watcher: &CancellationWatcher,
) -> Result<Vec<u8>, SubError> {
    if let Some(length) = response.content_length() {
        if length > max_bytes as u64 {
            return Err(SubError::TooLarge);
        }
    }
    let mut body = Vec::new();
    loop {
        watcher.check()?;
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if body.len() + chunk.len() > max_bytes {
                    return Err(SubError::TooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(err) => return Err(map_reqwest_error(&err)),
        }
    }
    Ok(body)
}

async fn wait_for_cancel(watcher: &CancellationWatcher) {
    while !watcher.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn map_reqwest_error(err: &reqwest::Error) -> SubError {
    if err.is_timeout() {
        SubError::Timeout
    } else if let Some(detail) = tls::trust_failure_of(err) {
        // A rejected peer certificate under the selected trust roots. The
        // detail carries only the certificate error, never URLs or secrets.
        SubError::Http(format!("tls trust rejected: {detail}"))
    } else if err.is_redirect() {
        SubError::Http("redirect".into())
    } else if err.is_decode() {
        SubError::Decode("response".into())
    } else {
        SubError::Http(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_header_is_rejected() {
        let options = DownloadOptions {
            headers: vec![("bad name".into(), "x".into())],
            ..DownloadOptions::default()
        };
        assert!(matches!(
            build_client(&options),
            Err(SubError::HeaderInvalid(_))
        ));
    }

    #[test]
    fn control_char_in_value_is_rejected() {
        let options = DownloadOptions {
            headers: vec![("X-Test".into(), "a\nb".into())],
            ..DownloadOptions::default()
        };
        assert!(matches!(
            build_client(&options),
            Err(SubError::HeaderInvalid(_))
        ));
    }

    #[test]
    fn non_http_proxy_is_rejected() {
        let options = DownloadOptions {
            proxy: Some(ProxyConfig::new("socks5://127.0.0.1:1080")),
            ..DownloadOptions::default()
        };
        assert!(matches!(
            build_client(&options),
            Err(SubError::InvalidUri(_))
        ));
    }

    #[test]
    fn origin_comparison_detects_scheme_host_port_changes() {
        let a = Url::parse("https://a.example/x").unwrap();
        let b = Url::parse("https://b.example/x").unwrap();
        assert!(origin_differs(&a, &b));
        assert!(!origin_differs(
            &a,
            &Url::parse("https://a.example/y").unwrap()
        ));
    }
}
