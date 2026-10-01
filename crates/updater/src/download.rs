//! Streaming download to a staging directory.
//!
//! Mirrors the observable behaviour of upstream `Services/DownloadService.cs`
//! but adds explicit guards the project requires: a hard byte cap, a
//! connect/total timeout and an `Incomplete` classification when the peer
//! closes early. The client never reads the environment proxy; callers pass an
//! explicit [`DownloaderOptions::proxy`] when one is wanted.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::error::UpdateError;

/// Download behaviour with safe defaults.
#[derive(Debug, Clone)]
pub struct DownloaderOptions {
    /// Extra request headers.
    pub headers: Vec<(String, String)>,
    /// User-Agent override.
    pub user_agent: Option<String>,
    /// Explicit proxy URL (`http://host:port`), otherwise direct.
    pub proxy: Option<String>,
    /// TCP/TLS connect timeout.
    pub connect_timeout: Duration,
    /// Whole-stream timeout, excludes the caller's own cancellation.
    pub timeout: Duration,
    /// Hard maximum body size in bytes.
    pub max_bytes: u64,
    /// Expected Content-Length, when the metadata provides one.
    pub expected_size: Option<u64>,
}

impl Default for DownloaderOptions {
    fn default() -> Self {
        Self {
            headers: Vec::new(),
            user_agent: Some("v2rayN-updater".to_string()),
            proxy: None,
            connect_timeout: Duration::from_secs(10),
            timeout: Duration::from_secs(180),
            max_bytes: 512 * 1024 * 1024,
            expected_size: None,
        }
    }
}

/// A single download request.
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    /// Destination file path (inside the staging directory).
    pub target: PathBuf,
}

impl DownloadRequest {
    pub fn new(url: impl Into<String>, target: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            target: target.into(),
        }
    }
}

/// Result of a completed download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedFile {
    pub path: PathBuf,
    pub bytes: u64,
    /// Lowercase hex SHA-256 computed while streaming.
    pub sha256: String,
    pub final_url: String,
}

/// Reusable streaming downloader.
#[derive(Debug, Clone)]
pub struct FileDownloader {
    client: reqwest::Client,
    headers: reqwest::header::HeaderMap,
    user_agent: Option<reqwest::header::HeaderValue>,
    options: DownloaderOptions,
}

impl FileDownloader {
    pub fn new(options: DownloaderOptions) -> Result<Self, UpdateError> {
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value) in &options.headers {
            let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| UpdateError::InvalidMetadata("invalid header name".into()))?;
            let value = reqwest::header::HeaderValue::from_str(value)
                .map_err(|_| UpdateError::InvalidMetadata("invalid header value".into()))?;
            headers.append(name, value);
        }
        let user_agent = match options.user_agent.as_deref() {
            Some(value) => Some(
                reqwest::header::HeaderValue::from_str(value)
                    .map_err(|_| UpdateError::InvalidMetadata("invalid user agent".into()))?,
            ),
            None => None,
        };

        let mut builder = reqwest::Client::builder()
            .connect_timeout(options.connect_timeout)
            .timeout(options.timeout)
            .redirect(reqwest::redirect::Policy::limited(10));
        if let Some(proxy) = &options.proxy {
            let parsed = reqwest::Proxy::all(proxy)
                .map_err(|e| UpdateError::Download(format!("proxy: {e}")))?;
            builder = builder.proxy(parsed);
        } else {
            builder = builder.no_proxy();
        }
        let client = builder
            .build()
            .map_err(|e| UpdateError::Download(format!("client build: {e}")))?;

        Ok(Self {
            client,
            headers,
            user_agent,
            options,
        })
    }

    /// Stream `request` to disk, computing the SHA-256 as it goes.
    ///
    /// The whole operation is wrapped in a cancellation race, including the
    /// initial `send()`, so a cancel request is honoured even while waiting for
    /// the response headers.
    pub async fn download(
        &self,
        request: &DownloadRequest,
        cancellation: &domain::CancellationToken,
    ) -> Result<DownloadedFile, UpdateError> {
        let work = self.download_inner(request);
        tokio::pin!(work);
        tokio::select! {
            result = &mut work => result,
            _ = wait_for_cancel(cancellation) => {
                // Best-effort cleanup of any partially written staging file.
                cleanup_partial(&request.target).await;
                Err(UpdateError::Cancelled)
            }
        }
    }

    async fn download_inner(
        &self,
        request: &DownloadRequest,
    ) -> Result<DownloadedFile, UpdateError> {
        let url = reqwest::Url::parse(&request.url)
            .map_err(|e| UpdateError::Download(format!("invalid url: {e}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(UpdateError::Download("scheme must be http(s)".into()));
        }
        if let Some(parent) = request.target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| UpdateError::Io(e.to_string()))?;
        }

        let mut builder = self.client.get(url).header(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("*/*"),
        );
        for (name, value) in self.headers.iter() {
            builder = builder.header(name, value);
        }
        if let Some(user_agent) = &self.user_agent {
            builder = builder.header(reqwest::header::USER_AGENT, user_agent.clone());
        }

        let response = builder.send().await.map_err(|e| classify(&e))?;
        if !response.status().is_success() {
            return Err(UpdateError::Download(format!(
                "status {}",
                response.status().as_u16()
            )));
        }
        if let Some(length) = response.content_length() {
            if length > self.options.max_bytes {
                return Err(UpdateError::TooLarge {
                    limit: self.options.max_bytes,
                });
            }
            if let Some(expected) = self.options.expected_size {
                if length < expected {
                    return Err(UpdateError::Incomplete);
                }
            }
        }

        let final_url = response.url().to_string();
        // Staging write: never overwrite an existing file with a partial body.
        let partial = partial_path(&request.target);
        let _ = tokio::fs::remove_file(&partial).await;
        let mut file = tokio::fs::File::create(&partial)
            .await
            .map_err(|e| UpdateError::Io(e.to_string()))?;

        let mut hasher = Sha256::new();
        let mut written: u64 = 0;
        let mut response = response;
        let body_result: Result<(), UpdateError> = loop {
            let chunk = response.chunk().await;
            match chunk {
                Ok(Some(chunk)) => {
                    written += chunk.len() as u64;
                    if written > self.options.max_bytes {
                        break Err(UpdateError::TooLarge {
                            limit: self.options.max_bytes,
                        });
                    }
                    hasher.update(&chunk);
                    if let Err(e) = file.write_all(&chunk).await {
                        break Err(UpdateError::Io(e.to_string()));
                    }
                }
                Ok(None) => break Ok(()),
                Err(e) => break Err(classify(&e)),
            }
        };

        if let Err(error) = body_result {
            let _ = file.shutdown().await;
            drop(file);
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(error);
        }
        file.flush()
            .await
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        file.shutdown()
            .await
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        drop(file);

        if written == 0 {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(UpdateError::Incomplete);
        }
        if let Some(expected) = self.options.expected_size {
            if written != expected {
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(UpdateError::Incomplete);
            }
        }

        tokio::fs::rename(&partial, &request.target)
            .await
            .map_err(|e| UpdateError::Io(e.to_string()))?;

        Ok(DownloadedFile {
            path: request.target.clone(),
            bytes: written,
            sha256: hex::encode(hasher.finalize()),
            final_url,
        })
    }
}

async fn wait_for_cancel(cancellation: &domain::CancellationToken) {
    while !cancellation.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Remove a staging file and its `.partial` sibling (best effort).
async fn cleanup_partial(target: &Path) {
    let partial = partial_path(target);
    let _ = tokio::fs::remove_file(&partial).await;
    let _ = tokio::fs::remove_file(target).await;
}

/// The temporary path a streaming write uses before the final rename.
fn partial_path(target: &Path) -> PathBuf {
    target.with_extension(format!(
        "{}partial",
        target
            .extension()
            .map(|e| format!("{}.", e.to_string_lossy()))
            .unwrap_or_default()
    ))
}

fn classify(err: &reqwest::Error) -> UpdateError {
    if err.is_timeout() {
        UpdateError::Timeout
    } else if err.is_body() || err.is_decode() {
        UpdateError::Incomplete
    } else {
        UpdateError::Download(err.to_string())
    }
}

/// Public helper: does a file's bytes hash to `expected_hex`?
pub fn sha256_matches(bytes: &[u8], expected_hex: &str) -> bool {
    let digest = hex::encode(Sha256::digest(bytes));
    digest.eq_ignore_ascii_case(expected_hex)
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_of(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Hash a file on disk (used to verify an already-staged artifact).
pub async fn sha256_file(path: &Path) -> Result<String, UpdateError> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| UpdateError::Io(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buf)
            .await
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_helper_matches() {
        // "abc" -> known digest.
        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(sha256_matches(b"abc", digest));
        assert!(sha256_matches(b"abc", &digest.to_uppercase()));
        assert!(!sha256_matches(b"abcd", digest));
    }

    #[test]
    fn rejects_non_http_scheme() {
        let downloader = FileDownloader::new(DownloaderOptions::default()).unwrap();
        let request = DownloadRequest::new("file:///etc/passwd", "x");
        let token = domain::CancellationToken::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = rt.block_on(downloader.download(&request, &token));
        assert!(matches!(result, Err(UpdateError::Download(_))));
    }
}
