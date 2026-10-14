//! Shared outbound HTTP facility (Wave B: G-05 + async fetch for FLD-CFG-087).
//!
//! Single owner for every application HTTPS client: timeouts, user-agent
//! policy, proxy handling, TLS trust roots and redirect behaviour are fixed
//! here. Callers (`subscriptions`, `updater`, `application`) build or reuse a
//! [`SharedHttpClient`] through this module instead of calling
//! `reqwest::Client::builder()` directly, and use [`fetch_bytes`] /
//! [`fetch_text`] for bounded, cancellable one-shot downloads (e.g. the
//! external routing-template source of FLD-CFG-087).
//!
//! Rules preserved from the pre-existing call sites:
//! - the environment proxy is never read; `None` means direct, an explicit
//!   `http(s)://` endpoint is the only proxy;
//! - `System` trust uses the OS/native roots, `BundledPem` trusts exactly the
//!   bundle and disables built-in roots; nothing writes the OS store;
//! - redirects are followed manually so auth headers are dropped on origin
//!   change; per-fetch `max_redirects` bounds the chain;
//! - bodies are capped (`max_bytes`) and observe cooperative cancellation.
//!
//! Test seam: [`construction_count`] counts real `reqwest::Client` builds,
//! [`shared`] reuses a cached client per equal [`HttpPolicy`], and
//! [`reset_construction_count_for_tests`] / [`clear_cache_for_tests`] reset
//! the counters. Loopback-only tests must probe ports `>= 11808` and never
//! touch `10808`.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use domain::CancellationToken;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION,
    USER_AGENT,
};
use reqwest::redirect::Policy;
use reqwest::Url;

/// Default user-agent token applied when a policy/fetch sets one explicitly.
///
/// The facility never injects a header on its own: `None` means "send no
/// `User-Agent`", preserving the pre-existing per-caller defaults (e.g.
/// subscriptions sends none, updater sends `v2rayN-updater`). Callers that
/// want the generic token set `user_agent: Some(DEFAULT_USER_AGENT.into())`.
pub const DEFAULT_USER_AGENT: &str = "v2rayN";

/// Default whole-operation timeout for [`HttpPolicy`].
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// Default TCP/TLS connect timeout for [`HttpPolicy`].
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default body cap for [`FetchOptions`] (16 MiB, matches subscriptions).
pub const DEFAULT_MAX_BYTES: usize = 16 * 1024 * 1024;
/// Default redirect bound for [`FetchOptions`].
pub const DEFAULT_MAX_REDIRECTS: usize = 10;

/// Root trust for outbound HTTPS (mirrors `platform::cert` selection).
/// `subscriptions::tls` and `updater::tls` re-export this type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HttpsTrust {
    /// OS/native roots only (upstream `system`).
    #[default]
    System,
    /// Exactly this PEM bundle, no OS roots (upstream `chrome`/`mozilla`).
    BundledPem(Vec<u8>),
}

impl HttpsTrust {
    /// Trust exactly `pem`, no OS roots.
    pub fn bundled(pem: &[u8]) -> Self {
        Self::BundledPem(pem.to_vec())
    }

    /// Map a `RootCertProvider` string onto trust roots. Unknown values fall
    /// back to `System`, matching upstream `ConfigHandler.LoadConfig`.
    pub fn from_provider(provider: &str, chrome_pem: &[u8], mozilla_pem: &[u8]) -> Self {
        match provider.trim().to_ascii_lowercase().as_str() {
            "chrome" => Self::bundled(chrome_pem),
            "mozilla" => Self::bundled(mozilla_pem),
            _ => Self::System,
        }
    }

    /// Upstream `CertPemManager.IsSystemRootCertProvider`.
    pub fn uses_system_store(&self) -> bool {
        matches!(self, Self::System)
    }

    /// Best-effort transport-string check for a TLS trust rejection.
    pub fn is_trust_failure(text: &str) -> bool {
        trust_failure_in_text(text)
    }
}

/// Apply `trust` to a `reqwest` client builder.
pub fn apply_trust(
    builder: reqwest::ClientBuilder,
    trust: &HttpsTrust,
) -> Result<reqwest::ClientBuilder, String> {
    match trust {
        HttpsTrust::System => Ok(builder),
        HttpsTrust::BundledPem(pem) => {
            let certs = reqwest::Certificate::from_pem_bundle(pem)
                .map_err(|e| format!("trust bundle parse: {e}"))?;
            if certs.is_empty() {
                return Err("trust bundle holds no certificate".to_string());
            }
            let mut builder = builder.tls_built_in_root_certs(false);
            for cert in certs {
                builder = builder.add_root_certificate(cert);
            }
            Ok(builder)
        }
    }
}

/// Transport-string check behind [`HttpsTrust::is_trust_failure`].
pub fn trust_failure_in_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "certificate",
        "unknownissuer",
        "notvalidforname",
        "handshake",
        "expired",
        "revoked",
        "untrusted",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// Walk a `reqwest` error's source chain for TLS trust evidence.
pub fn trust_failure_of(err: &reqwest::Error) -> Option<String> {
    let mut current: Option<&dyn std::error::Error> = Some(err);
    while let Some(source) = current {
        let text = source.to_string();
        if trust_failure_in_text(&text) {
            return Some(text);
        }
        current = source.source();
    }
    None
}

/// Redirect behaviour of a [`SharedHttpClient`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RedirectPolicy {
    /// Never follow automatically; [`fetch_bytes`] follows manually.
    #[default]
    None,
    /// Follow up to `n` redirects automatically (reqwest built-in).
    Limited(usize),
}

/// Construction policy for one [`SharedHttpClient`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpPolicy {
    /// Whole-operation timeout (also set on the reqwest client).
    pub timeout: Duration,
    /// TCP/TLS connect timeout.
    pub connect_timeout: Duration,
    /// Explicit `User-Agent`; `None` sends no header.
    pub user_agent: Option<String>,
    /// Explicit proxy endpoint (`http(s)://host:port`); `None` is direct.
    pub proxy: Option<String>,
    /// TLS trust roots.
    pub trust: HttpsTrust,
    /// Redirect behaviour.
    pub redirect: RedirectPolicy,
}

impl HttpPolicy {
    /// Direct loopback-friendly policy with short timeouts (tests).
    pub fn test_direct() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(5),
            user_agent: None,
            proxy: None,
            trust: HttpsTrust::System,
            redirect: RedirectPolicy::None,
        }
    }

    fn validate(&self) -> Result<Option<HeaderValue>, String> {
        let user_agent = match self.user_agent.as_deref().filter(|v| !v.is_empty()) {
            Some(value) => {
                if value.chars().any(|c| c.is_control() && c != '\t') {
                    return Err("control character in user agent".to_string());
                }
                Some(HeaderValue::from_str(value).map_err(|_| "invalid user agent".to_string())?)
            }
            None => None,
        };
        if let Some(proxy) = &self.proxy {
            if !proxy.starts_with("http://") && !proxy.starts_with("https://") {
                return Err("proxy scheme must be http(s)".to_string());
            }
            reqwest::Proxy::all(proxy).map_err(|e| format!("proxy: {e}"))?;
        }
        Ok(user_agent)
    }
}

static CONSTRUCTION_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn cache() -> &'static Mutex<Vec<(HttpPolicy, SharedHttpClient)>> {
    static CACHE: OnceLock<Mutex<Vec<(HttpPolicy, SharedHttpClient)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Number of real `reqwest::Client` builds through this module.
pub fn construction_count() -> u64 {
    CONSTRUCTION_COUNT.load(std::sync::atomic::Ordering::SeqCst)
}

/// Reset [`construction_count`] (tests only).
pub fn reset_construction_count_for_tests() {
    CONSTRUCTION_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
}

/// Drop all cached shared clients (tests only).
pub fn clear_cache_for_tests() {
    if let Ok(mut guard) = cache().lock() {
        guard.clear();
    }
}

/// Reusable shared HTTP client. Clones share the same `reqwest::Client`.
#[derive(Debug, Clone)]
pub struct SharedHttpClient {
    inner: Arc<reqwest::Client>,
    policy: HttpPolicy,
    user_agent: Option<HeaderValue>,
}

impl SharedHttpClient {
    /// Build a new client (counts one construction).
    pub fn new(policy: HttpPolicy) -> Result<Self, String> {
        let user_agent = policy.validate()?;
        let reqwest_policy = match policy.redirect {
            RedirectPolicy::None => Policy::none(),
            RedirectPolicy::Limited(n) => Policy::limited(n),
        };
        let mut builder = apply_trust(
            reqwest::Client::builder()
                .redirect(reqwest_policy)
                .timeout(policy.timeout)
                .connect_timeout(policy.connect_timeout)
                .no_proxy(),
            &policy.trust,
        )?;
        if let Some(proxy) = &policy.proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|e| format!("proxy: {e}"))?);
        }
        let inner = builder.build().map_err(|e| format!("client build: {e}"))?;
        CONSTRUCTION_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self {
            inner: Arc::new(inner),
            policy,
            user_agent,
        })
    }

    /// Return the cached client for an equal policy, building once.
    pub fn shared(policy: HttpPolicy) -> Result<Self, String> {
        if let Ok(guard) = cache().lock() {
            if let Some((_, cached)) = guard.iter().find(|(p, _)| p == &policy) {
                return Ok(cached.clone());
            }
        }
        let client = Self::new(policy.clone())?;
        if let Ok(mut guard) = cache().lock() {
            if let Some((_, cached)) = guard.iter().find(|(p, _)| p == &policy) {
                return Ok(cached.clone());
            }
            guard.push((policy, client.clone()));
        }
        Ok(client)
    }

    /// The underlying client.
    pub fn client(&self) -> &reqwest::Client {
        &self.inner
    }

    /// The policy this client was built with.
    pub fn policy(&self) -> &HttpPolicy {
        &self.policy
    }

    /// True when both handles share the same underlying client.
    pub fn same_inner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Per-fetch bounds for [`fetch_bytes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchOptions {
    /// Maximum accepted body size in bytes.
    pub max_bytes: usize,
    /// Maximum redirects followed manually.
    pub max_redirects: usize,
    /// Extra request headers (name, value).
    pub headers: Vec<(String, String)>,
    /// Per-fetch `User-Agent` override; `None` falls back to the client policy.
    pub user_agent: Option<String>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            max_redirects: DEFAULT_MAX_REDIRECTS,
            headers: Vec::new(),
            user_agent: None,
        }
    }
}

/// A bounded fetch result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// URL after redirects.
    pub final_url: String,
    /// Advertised content type, if any.
    pub content_type: Option<String>,
    /// Raw body bytes (length `<=` the fetch `max_bytes`).
    pub body: Vec<u8>,
}

impl Fetched {
    /// Lossy text view of the body.
    pub fn text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Strict UTF-8 view of the body.
    pub fn text(&self) -> Result<String, HttpError> {
        std::str::from_utf8(&self.body)
            .map(str::to_owned)
            .map_err(|e| HttpError::Http(format!("invalid utf-8: {e}")))
    }

    /// Strict UTF-8 text, taking the body without a copy.
    pub fn into_text(self) -> Result<String, HttpError> {
        String::from_utf8(self.body).map_err(|e| HttpError::Http(format!("invalid utf-8: {e}")))
    }
}

/// Fetch errors (credential-free: never echo URLs, headers or bodies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// The operation exceeded its time budget.
    Timeout,
    /// Cooperative cancellation was requested.
    Cancelled,
    /// The body exceeded `max_bytes`.
    TooLarge,
    /// The URL or redirect target is unusable.
    InvalidUri(String),
    /// A header name/value is unusable.
    Header(String),
    /// Too many redirects.
    Redirect(String),
    /// Transport or unexpected-status failure (status text only).
    Http(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::Timeout => write!(f, "timeout"),
            HttpError::Cancelled => write!(f, "cancelled"),
            HttpError::TooLarge => write!(f, "body too large"),
            HttpError::InvalidUri(detail) => write!(f, "invalid uri: {detail}"),
            HttpError::Header(detail) => write!(f, "invalid header: {detail}"),
            HttpError::Redirect(detail) => write!(f, "redirect: {detail}"),
            HttpError::Http(detail) => write!(f, "http: {detail}"),
        }
    }
}

impl std::error::Error for HttpError {}

fn origin_differs(a: &Url, b: &Url) -> bool {
    a.scheme() != b.scheme()
        || a.host_str() != b.host_str()
        || a.port_or_known_default() != b.port_or_known_default()
}

fn map_request_error(err: &reqwest::Error) -> HttpError {
    if err.is_timeout() {
        HttpError::Timeout
    } else if let Some(detail) = trust_failure_of(err) {
        HttpError::Http(format!("tls trust rejected: {detail}"))
    } else if err.is_redirect() {
        HttpError::Redirect("redirect".to_string())
    } else {
        HttpError::Http(err.to_string())
    }
}

/// Bounded, cancellable one-shot fetch on top of a [`SharedHttpClient`].
///
/// Redirects are followed manually (auth headers dropped on origin change)
/// up to `options.max_redirects`; the whole operation races `client`
/// policy timeout against `cancellation`.
pub async fn fetch_bytes(
    client: &SharedHttpClient,
    url: &str,
    options: &FetchOptions,
    cancellation: &CancellationToken,
) -> Result<Fetched, HttpError> {
    let parsed = Url::parse(url).map_err(|e| HttpError::InvalidUri(e.to_string()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(HttpError::InvalidUri("scheme must be http(s)".to_string()));
    }
    let mut headers = HeaderMap::new();
    for (name, value) in &options.headers {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| HttpError::Header("name".to_string()))?;
        if value.chars().any(|c| c.is_control() && c != '\t') {
            return Err(HttpError::Header("control character in value".to_string()));
        }
        let value = HeaderValue::from_str(value)
            .map_err(|_| HttpError::Header("invalid value".to_string()))?;
        headers.append(name, value);
    }
    let fetch_agent = match options.user_agent.as_deref().filter(|v| !v.is_empty()) {
        Some(value) => {
            if value.chars().any(|c| c.is_control() && c != '\t') {
                return Err(HttpError::Header(
                    "control character in user agent".to_string(),
                ));
            }
            Some(
                HeaderValue::from_str(value)
                    .map_err(|_| HttpError::Header("user agent".to_string()))?,
            )
        }
        None => client.user_agent.clone(),
    };

    let work = follow(client, parsed, &headers, fetch_agent, options, cancellation);
    tokio::select! {
        result = tokio::time::timeout(client.policy.timeout, work) => match result {
            Ok(inner) => inner,
            Err(_) => Err(HttpError::Timeout),
        },
        _ = wait_for_cancel(cancellation) => Err(HttpError::Cancelled),
    }
}

async fn follow(
    client: &SharedHttpClient,
    start: Url,
    headers: &HeaderMap,
    user_agent: Option<HeaderValue>,
    options: &FetchOptions,
    cancellation: &CancellationToken,
) -> Result<Fetched, HttpError> {
    let mut url = start;
    let mut headers = headers.clone();
    for hop in 0..=options.max_redirects {
        if cancellation.is_cancelled() {
            return Err(HttpError::Cancelled);
        }
        let mut builder = client.client().get(url.clone()).header(ACCEPT, "*/*");
        for (name, value) in headers.iter() {
            builder = builder.header(name, value);
        }
        if let Some(agent) = &user_agent {
            builder = builder.header(USER_AGENT, agent.clone());
        }
        let response = builder.send().await.map_err(|e| map_request_error(&e))?;

        if response.status().is_redirection() {
            if hop == options.max_redirects {
                return Err(HttpError::Redirect("too many redirects".to_string()));
            }
            let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
                return Err(HttpError::Redirect("redirect without Location".to_string()));
            };
            let location = location
                .to_str()
                .map_err(|_| HttpError::Redirect("invalid Location".to_string()))?;
            let next = url
                .join(location)
                .map_err(|e| HttpError::InvalidUri(format!("bad redirect: {e}")))?;
            if next.scheme() != "http" && next.scheme() != "https" {
                return Err(HttpError::InvalidUri(
                    "redirect scheme must be http(s)".to_string(),
                ));
            }
            if origin_differs(&url, &next) {
                headers.remove(AUTHORIZATION);
                headers.remove(COOKIE);
                headers.remove(PROXY_AUTHORIZATION);
            }
            url = next;
            continue;
        }

        if !response.status().is_success() {
            return Err(HttpError::Http(format!(
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
        let body = read_body_capped(response, options.max_bytes, cancellation).await?;
        return Ok(Fetched {
            final_url,
            content_type,
            body,
        });
    }
    Err(HttpError::Redirect("too many redirects".to_string()))
}

async fn read_body_capped(
    mut response: reqwest::Response,
    max_bytes: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, HttpError> {
    if let Some(length) = response.content_length() {
        if length > max_bytes as u64 {
            return Err(HttpError::TooLarge);
        }
    }
    let mut body = Vec::new();
    loop {
        if cancellation.is_cancelled() {
            return Err(HttpError::Cancelled);
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if body.len() + chunk.len() > max_bytes {
                    return Err(HttpError::TooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(err) => return Err(map_request_error(&err)),
        }
    }
    Ok(body)
}

async fn wait_for_cancel(cancellation: &CancellationToken) {
    while !cancellation.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Convenience: fetch and require strict UTF-8 text.
pub async fn fetch_text(
    client: &SharedHttpClient,
    url: &str,
    options: &FetchOptions,
    cancellation: &CancellationToken,
) -> Result<String, HttpError> {
    fetch_bytes(client, url, options, cancellation)
        .await
        .and_then(Fetched::into_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_mapping_matches_upstream_fallback() {
        assert_eq!(
            HttpsTrust::from_provider("system", b"c", b"m"),
            HttpsTrust::System
        );
        assert_eq!(
            HttpsTrust::from_provider("chrome", b"c", b"m"),
            HttpsTrust::BundledPem(b"c".to_vec())
        );
        assert_eq!(
            HttpsTrust::from_provider(" Mozilla ", b"c", b"m"),
            HttpsTrust::BundledPem(b"m".to_vec())
        );
        assert_eq!(
            HttpsTrust::from_provider("bogus", b"c", b"m"),
            HttpsTrust::System
        );
        assert_eq!(
            HttpsTrust::from_provider("", b"c", b"m"),
            HttpsTrust::System
        );
    }

    #[test]
    fn classifier_marks_only_tls_strings() {
        assert!(HttpsTrust::is_trust_failure(
            "invalid peer certificate: UnknownIssuer"
        ));
        assert!(!HttpsTrust::is_trust_failure("status 404"));
        assert!(!HttpsTrust::is_trust_failure("timeout"));
    }

    #[test]
    fn proxy_scheme_is_validated_locally() {
        let policy = HttpPolicy {
            proxy: Some("socks5://127.0.0.1:1080".to_string()),
            ..HttpPolicy::test_direct()
        };
        assert!(SharedHttpClient::new(policy).is_err());
    }

    #[test]
    fn equal_policies_share_one_construction() {
        let before = construction_count();
        let policy = HttpPolicy {
            timeout: std::time::Duration::from_millis(78_001),
            ..HttpPolicy::test_direct()
        };
        let first = SharedHttpClient::shared(policy.clone()).unwrap();
        assert_eq!(construction_count(), before + 1);
        let second = SharedHttpClient::shared(policy).unwrap();
        assert!(first.same_inner(&second));
        assert_eq!(construction_count(), before + 1);
    }
}
