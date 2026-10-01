//! Download: streaming, size cap, timeout, truncation, cancellation, sha256.
//!
//! Mock servers bind `127.0.0.1` on ports `>= 11808`; `10808` is never touched.

mod common;

use std::time::Duration;

use common::{pick_spare_port, MockHttp, MockResponse};
use updater::{
    sha256_file, sha256_of, CancellationToken, DownloadRequest, DownloaderOptions, FileDownloader,
    UpdateError,
};

fn downloader(options: DownloaderOptions) -> FileDownloader {
    FileDownloader::new(options).expect("build downloader")
}

#[tokio::test]
async fn streams_file_and_computes_sha256() {
    let payload = b"xray-binary-payload".to_vec();
    let expected = sha256_of(&payload);
    let payload_for_server = payload.clone();
    let mock = MockHttp::spawn(move |_| MockResponse::bytes(payload_for_server.clone()));
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Xray-windows-64.zip");
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), &target);
    let token = CancellationToken::new();
    let result = downloader(DownloaderOptions::default())
        .download(&request, &token)
        .await
        .unwrap();
    assert_eq!(result.sha256, expected);
    assert_eq!(result.bytes, payload.len() as u64);
    assert!(target.exists());
    assert_eq!(sha256_file(&target).await.unwrap(), expected);
}

#[tokio::test]
async fn rejects_oversized_content_length() {
    let mock = MockHttp::spawn(|_| MockResponse::bytes(vec![0u8; 1024]));
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), dir.path().join("a"));
    let token = CancellationToken::new();
    let options = DownloaderOptions {
        max_bytes: 100,
        ..DownloaderOptions::default()
    };
    let err = downloader(options)
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::TooLarge { limit: 100 }));
}

#[tokio::test]
async fn rejects_oversized_streamed_body() {
    // No Content-Length (chunked-ish) still capped while reading.
    let mock = MockHttp::spawn(|_| MockResponse::bytes(vec![7u8; 4096]));
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), dir.path().join("a"));
    let token = CancellationToken::new();
    let options = DownloaderOptions {
        max_bytes: 1024,
        ..DownloaderOptions::default()
    };
    let err = downloader(options)
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::TooLarge { .. }));
    // No partial file left behind.
    assert!(!request.target.exists());
}

#[tokio::test]
async fn zero_byte_response_is_incomplete() {
    let mock = MockHttp::spawn(|_| MockResponse::bytes(Vec::<u8>::new()));
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), dir.path().join("a"));
    let token = CancellationToken::new();
    let err = downloader(DownloaderOptions::default())
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Incomplete));
}

#[tokio::test]
async fn wrong_size_vs_expected_is_incomplete() {
    let mock = MockHttp::spawn(|_| MockResponse::bytes(vec![1u8; 10]));
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), dir.path().join("a"));
    let token = CancellationToken::new();
    let options = DownloaderOptions {
        expected_size: Some(100),
        ..DownloaderOptions::default()
    };
    let err = downloader(options)
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Incomplete));
}

#[tokio::test]
async fn connect_timeout_is_classified() {
    let port = pick_spare_port();
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(
        format!("http://127.0.0.1:{port}/asset"),
        dir.path().join("a"),
    );
    let token = CancellationToken::new();
    let options = DownloaderOptions {
        connect_timeout: Duration::from_millis(200),
        timeout: Duration::from_millis(500),
        ..DownloaderOptions::default()
    };
    let err = downloader(options)
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        UpdateError::Timeout | UpdateError::Download(_)
    ));
}

#[tokio::test]
async fn slow_server_hits_whole_timeout() {
    let mock =
        MockHttp::spawn(|_| MockResponse::bytes(vec![1u8; 8]).delayed(Duration::from_secs(3)));
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), dir.path().join("a"));
    let token = CancellationToken::new();
    let options = DownloaderOptions {
        timeout: Duration::from_millis(300),
        ..DownloaderOptions::default()
    };
    let err = downloader(options)
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Timeout));
}

#[tokio::test]
async fn http_error_status_is_reported() {
    let mock = MockHttp::spawn(|_| MockResponse::json("nope").status(404));
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), dir.path().join("a"));
    let token = CancellationToken::new();
    let err = downloader(DownloaderOptions::default())
        .download(&request, &token)
        .await
        .unwrap_err();
    match err {
        UpdateError::Download(message) => assert!(message.contains("404")),
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn cancellation_aborts_before_completion() {
    let mock = MockHttp::spawn(|_| {
        MockResponse::bytes(vec![0u8; 1024 * 1024]).delayed(Duration::from_secs(2))
    });
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new(format!("{}/asset", mock.base_url()), dir.path().join("a"));
    let token = CancellationToken::new();
    let canceller = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        canceller.cancel();
    });
    let err = downloader(DownloaderOptions::default())
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Cancelled));
    assert!(!request.target.exists());
}

#[tokio::test]
async fn non_http_scheme_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let request = DownloadRequest::new("ftp://example/x", dir.path().join("a"));
    let token = CancellationToken::new();
    let err = downloader(DownloaderOptions::default())
        .download(&request, &token)
        .await
        .unwrap_err();
    assert!(matches!(err, UpdateError::Download(_)));
}
