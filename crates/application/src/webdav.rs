//! T16 WebDAV use cases (check / list / upload / download).
//!
//! Credentials and the target directory come explicitly from the configured
//! `WebDavItem`; the endpoint defaults to upstream's `v2rayN_backup/backup.zip`
//! when no directory is configured. Credentials are sent with HTTP Basic auth
//! and are **never** written to logs or error detail.

use std::path::Path;
use std::time::Duration;

use domain::{codes, DomainError};
use updater::tls::HttpsTrust;

/// Upstream default remote directory (`BackupAndRestoreViewModel`).
pub const DEFAULT_DIR: &str = "v2rayN_backup";
/// Upstream default remote file name.
pub const BACKUP_FILE: &str = "backup.zip";

fn webdav_error(code: &'static str, key: &str, detail: impl Into<String>) -> DomainError {
    DomainError::new(code, key).with_detail(detail.into())
}

/// Explicit WebDAV endpoint configuration (mirrors `WebDavItem`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebDavConfig {
    pub url: String,
    pub user_name: String,
    pub password: String,
    pub dir_name: String,
}

impl WebDavConfig {
    pub fn new(
        url: impl Into<String>,
        user_name: impl Into<String>,
        password: impl Into<String>,
        dir_name: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            user_name: user_name.into(),
            password: password.into(),
            dir_name: dir_name.into(),
        }
    }

    fn effective_dir(&self) -> &str {
        let dir = self.dir_name.trim();
        if dir.is_empty() {
            DEFAULT_DIR
        } else {
            dir.trim_matches('/')
        }
    }

    fn base(&self) -> String {
        self.url.trim().trim_end_matches('/').to_string()
    }

    /// The full URL of the remote backup file.
    pub fn backup_url(&self) -> String {
        format!("{}/{}/{}", self.base(), self.effective_dir(), BACKUP_FILE)
    }
}

/// One remote entry returned by `list`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebDavEntry {
    pub href: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

/// Result of a connection check.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebDavCheck {
    pub created_dir: bool,
    pub status: u16,
}

/// A configured WebDAV client (built on the shared `platform::http` facility).
#[derive(Debug, Clone)]
pub struct WebDavClient {
    http: platform::http::SharedHttpClient,
    config: WebDavConfig,
}

impl WebDavClient {
    /// Build a client for `config`. `proxy` is an explicit endpoint (the
    /// running session's local port); `None` stays direct so loopback test
    /// servers are always reachable. Trust is the OS/native store; see
    /// [`Self::new_with_tls`] for the `RootCertProvider` selection.
    pub fn new(
        config: WebDavConfig,
        timeout: Duration,
        proxy: Option<&str>,
    ) -> Result<Self, DomainError> {
        Self::new_with_tls(config, timeout, proxy, &HttpsTrust::System)
    }

    /// Build a client trusting `trust` (SP-25 `RootCertProvider` consumer).
    /// `BundledPem` trusts exactly the bundle, never the OS store.
    pub fn new_with_tls(
        config: WebDavConfig,
        timeout: Duration,
        proxy: Option<&str>,
        trust: &HttpsTrust,
    ) -> Result<Self, DomainError> {
        if config.url.trim().is_empty() {
            return Err(webdav_error(
                codes::FIELD_REQUIRED,
                "error.webdav_url_required",
                "url is empty",
            ));
        }
        if !(config.url.starts_with("http://") || config.url.starts_with("https://")) {
            return Err(webdav_error(
                codes::FIELD_FORMAT,
                "error.webdav_url_invalid",
                "url must be http(s)",
            ));
        }
        let policy = platform::http::HttpPolicy {
            timeout,
            connect_timeout: Duration::from_secs(10),
            user_agent: None,
            proxy: proxy
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            trust: trust.clone(),
            redirect: platform::http::RedirectPolicy::Limited(10),
        };
        let http = platform::http::SharedHttpClient::shared(policy)
            .map_err(|e| webdav_error(codes::INTERNAL, "error.webdav_client", e))?;
        Ok(Self { http, config })
    }

    pub fn config(&self) -> &WebDavConfig {
        &self.config
    }

    fn dir_url(&self) -> String {
        format!("{}/{}/", self.config.base(), self.config.effective_dir())
    }

    fn request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut builder = self.http.client().request(method, url);
        if !self.config.user_name.is_empty() {
            builder = builder.basic_auth(&self.config.user_name, Some(&self.config.password));
        }
        builder
    }

    fn status_error(&self, status: reqwest::StatusCode) -> DomainError {
        match status.as_u16() {
            401 | 403 => webdav_error(
                codes::PERMISSION_DENIED,
                "error.webdav_permission",
                format!("status {}", status.as_u16()),
            ),
            404 => webdav_error(
                codes::NOT_FOUND,
                "error.webdav_not_found",
                format!("status {}", status.as_u16()),
            ),
            other => webdav_error(
                codes::UNAVAILABLE,
                "error.webdav_remote",
                format!("status {other}"),
            )
            .retryable(),
        }
    }

    fn network_error(&self, err: reqwest::Error) -> DomainError {
        if err.is_timeout() {
            DomainError::new(codes::TIMEOUT, "error.webdav_timeout")
                .with_detail(err.to_string())
                .retryable()
        } else if let Some(detail) = updater::tls::trust_failure_of(&err) {
            // A rejected peer certificate under the selected trust roots. The
            // detail carries only the certificate error, never credentials.
            DomainError::new(codes::UNAVAILABLE, "error.webdav_tls")
                .with_detail(format!("tls trust rejected: {detail}"))
                .retryable()
        } else {
            DomainError::new(codes::UNAVAILABLE, "error.webdav_network")
                .with_detail(err.to_string())
                .retryable()
        }
    }

    /// Ensure the remote directory exists and is listable.
    pub async fn check(&self) -> Result<WebDavCheck, DomainError> {
        let dir_url = self.dir_url();
        let probe = self
            .request(propfind(), &dir_url)
            .header("Depth", "0")
            .send()
            .await
            .map_err(|e| self.network_error(e))?;
        if probe.status().is_success() {
            return Ok(WebDavCheck {
                created_dir: false,
                status: probe.status().as_u16(),
            });
        }
        if probe.status().as_u16() != 404 {
            return Err(self.status_error(probe.status()));
        }
        let created = self
            .request(
                reqwest::Method::from_bytes(b"MKCOL").map_err(|e| {
                    webdav_error(codes::INTERNAL, "error.webdav_client", e.to_string())
                })?,
                &dir_url,
            )
            .send()
            .await
            .map_err(|e| self.network_error(e))?;
        let status = created.status();
        if !(status.is_success() || status.as_u16() == 405) {
            return Err(self.status_error(status));
        }
        Ok(WebDavCheck {
            created_dir: status.as_u16() == 201,
            status: status.as_u16(),
        })
    }

    /// List the remote backup directory (Depth: 1).
    pub async fn list(&self) -> Result<Vec<WebDavEntry>, DomainError> {
        let response = self
            .request(propfind(), &self.dir_url())
            .header("Depth", "1")
            .send()
            .await
            .map_err(|e| self.network_error(e))?;
        if !response.status().is_success() {
            return Err(self.status_error(response.status()));
        }
        let body = response.text().await.map_err(|e| self.network_error(e))?;
        Ok(parse_propfind(&body))
    }

    /// Upload bytes to `dir/backup.zip`.
    pub async fn upload(&self, bytes: Vec<u8>) -> Result<u64, DomainError> {
        let length = bytes.len() as u64;
        let response = self
            .request(reqwest::Method::PUT, &self.config.backup_url())
            .header(reqwest::header::CONTENT_LENGTH, length)
            .body(bytes)
            .send()
            .await
            .map_err(|e| self.network_error(e))?;
        if !response.status().is_success() {
            return Err(self.status_error(response.status()));
        }
        Ok(length)
    }

    /// Stream a ZIP file to `dir/backup.zip` without holding it in memory.
    pub async fn upload_file(&self, path: &Path, length: u64) -> Result<u64, DomainError> {
        let file = tokio::fs::File::open(path).await.map_err(|error| {
            webdav_error(codes::INTERNAL, "error.webdav_write", error.to_string())
        })?;
        let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file));
        let response = self
            .request(reqwest::Method::PUT, &self.config.backup_url())
            .header(reqwest::header::CONTENT_LENGTH, length)
            .body(body)
            .send()
            .await
            .map_err(|e| self.network_error(e))?;
        if !response.status().is_success() {
            return Err(self.status_error(response.status()));
        }
        Ok(length)
    }

    /// Download `dir/backup.zip`.
    pub async fn download(&self) -> Result<Vec<u8>, DomainError> {
        let response = self
            .request(reqwest::Method::GET, &self.config.backup_url())
            .send()
            .await
            .map_err(|e| self.network_error(e))?;
        if !response.status().is_success() {
            return Err(self.status_error(response.status()));
        }
        let bytes = response.bytes().await.map_err(|e| self.network_error(e))?;
        Ok(bytes.to_vec())
    }

    /// Stream `dir/backup.zip` to disk without holding it in memory.
    pub async fn download_to_file(&self, path: &Path) -> Result<u64, DomainError> {
        use tokio::io::AsyncWriteExt;

        let mut response = self
            .request(reqwest::Method::GET, &self.config.backup_url())
            .send()
            .await
            .map_err(|e| self.network_error(e))?;
        if !response.status().is_success() {
            return Err(self.status_error(response.status()));
        }
        let mut file = tokio::fs::File::create(path).await.map_err(|error| {
            webdav_error(codes::INTERNAL, "error.webdav_write", error.to_string())
        })?;
        let mut length = 0_u64;
        while let Some(chunk) = response.chunk().await.map_err(|e| self.network_error(e))? {
            file.write_all(&chunk).await.map_err(|error| {
                webdav_error(codes::INTERNAL, "error.webdav_write", error.to_string())
            })?;
            length += chunk.len() as u64;
        }
        file.flush().await.map_err(|error| {
            webdav_error(codes::INTERNAL, "error.webdav_write", error.to_string())
        })?;
        Ok(length)
    }
}

fn propfind() -> reqwest::Method {
    reqwest::Method::from_bytes(b"PROPFIND").unwrap_or(reqwest::Method::GET)
}

/// Minimal WebDAV multistatus parser: enough for the `<D:href>`,
/// `<D:getcontentlength>` and `<D:getlastmodified>` fields every server emits.
fn parse_propfind(xml: &str) -> Vec<WebDavEntry> {
    let mut entries = Vec::new();
    for block in split_href_blocks(xml) {
        let href = extract_tag(&block, "href");
        if href.is_empty() {
            continue;
        }
        let is_dir = href.ends_with('/');
        let size = extract_tag(&block, "getcontentlength")
            .parse::<u64>()
            .unwrap_or(0);
        let modified = extract_tag(&block, "getlastmodified");
        entries.push(WebDavEntry {
            href,
            is_dir,
            size,
            modified,
        });
    }
    entries
}

fn split_href_blocks(xml: &str) -> Vec<String> {
    let lower = xml.to_ascii_lowercase();
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("<d:response") {
        let start = cursor + rel;
        match lower[start..].find("</d:response>") {
            Some(end_rel) => {
                let end = start + end_rel + "</d:response>".len();
                blocks.push(xml[start..end].to_string());
                cursor = end;
            }
            None => break,
        }
    }
    if blocks.is_empty() {
        // Some servers omit the `D:` namespace prefix.
        let mut cursor = 0usize;
        while let Some(rel) = lower[cursor..].find("<response") {
            let start = cursor + rel;
            match lower[start..].find("</response>") {
                Some(end_rel) => {
                    let end = start + end_rel + "</response>".len();
                    blocks.push(xml[start..end].to_string());
                    cursor = end;
                }
                None => break,
            }
        }
    }
    blocks
}

fn extract_tag(block: &str, tag: &str) -> String {
    let lower = block.to_ascii_lowercase();
    let needle = format!(":{tag}>");
    let open = match lower.find(&needle) {
        Some(index) => index + needle.len(),
        None => {
            let plain = format!("<{tag}>");
            match lower.find(&plain) {
                Some(index) => index + plain.len(),
                None => return String::new(),
            }
        }
    };
    let rest = &block[open..];
    let close = rest.find('<').unwrap_or(rest.len());
    decode_entities(rest[..close].trim())
}

fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
