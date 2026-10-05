//! R4-29 full application-release chain over synthetic inputs:
//!
//! download (loopback asset) -> OpenPGP detached verification (synthetic trust
//! root) -> safe unpack/stage -> flat overlay (keep `app.previous`) -> rollback
//! -> restart-command construction.
//!
//! The external runner is a stub file that is never executed and no host state
//! (system proxy, routes, 10808 port, registry) is touched. Ports are
//! OS-assigned ephemeral ports, never 10808.
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use pgp::composed::{KeyType, SecretKeyParamsBuilder, StandaloneSignature};
use pgp::packet::{SignatureConfig, SignatureType};
use pgp::types::Password;
use rand::thread_rng;
use tiny_http::{Response, Server};
use updater::{
    apply_app_upgrade, rollback_app_upgrade, safe_unpack_zip, AppInstallLayout, CancellationToken,
    DownloadRequest, DownloaderOptions, FileDownloader, PgpDetachedVerifier, SignatureVerifier,
    UnpackLimits,
};

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        writer
            .start_file((*name).to_string(), options)
            .expect("start");
        writer.write_all(bytes).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

/// Synthetic OpenPGP release key + detached-signature trust root.
fn release_key() -> (pgp::composed::SignedSecretKey, PgpDetachedVerifier) {
    let mut rng = thread_rng();
    let params = SecretKeyParamsBuilder::default()
        .key_type(KeyType::Ed25519Legacy)
        .can_certify(true)
        .can_sign(true)
        .primary_user_id("r4-29 synthetic release <r4-29@example.invalid>".into())
        .passphrase(None)
        .build()
        .expect("key params");
    let secret = params.generate(&mut rng).expect("generate");
    let signed = secret.sign(&mut rng, &Password::empty()).expect("sign key");
    let armored = signed
        .signed_public_key()
        .to_armored_bytes(None.into())
        .expect("armor key");
    let verifier =
        PgpDetachedVerifier::from_armored_public_key(&armored).expect("parse public key");
    (signed, verifier)
}

fn detached_signature(secret: &pgp::composed::SignedSecretKey, data: &[u8]) -> Vec<u8> {
    let mut rng = thread_rng();
    let config = SignatureConfig::from_key(&mut rng, &secret.primary_key, SignatureType::Binary)
        .expect("config");
    let signature = config
        .sign(&secret.primary_key, &Password::empty(), data)
        .expect("sign");
    StandaloneSignature::new(signature)
        .to_armored_bytes(None.into())
        .expect("armor signature")
}

/// Serve `bytes` on an OS-assigned loopback port and return its URL.
fn serve_asset(bytes: Vec<u8>) -> String {
    let server = Server::http("127.0.0.1:0").expect("bind mock");
    let port = server.server_addr().to_ip().expect("ip listen addr").port();
    let url = format!("http://127.0.0.1:{port}/asset.zip");
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            if request.url() == "/asset.zip" {
                let _ = request.respond(Response::from_data(bytes.clone()).with_status_code(200));
            } else {
                let _ = request.respond(Response::empty(404));
            }
        }
    });
    url
}

fn stage_zip(root: &Path, version: &str, zip: &Path) -> std::path::PathBuf {
    let dest = root
        .join(".staging")
        .join(format!("app-{version}"))
        .join("unpacked");
    safe_unpack_zip(zip, &dest, UnpackLimits::default()).expect("unpack");
    dest
}

#[test]
fn verified_release_chain_download_stage_replace_rollback_restart() {
    let (secret, verifier) = release_key();
    let zip = make_zip(&[
        ("v2rayn_desktop.exe", b"NEW-APP".as_slice()),
        ("data/seed.txt", b"seed".as_slice()),
    ]);
    let signature = detached_signature(&secret, &zip);

    // PGP verification of the authentic asset succeeds; a tampered asset and a
    // missing/garbage signature fail closed (FIX-12B semantics).
    verifier.verify(&zip, &signature).expect("valid signature");
    assert!(verifier.verify(b"tampered", &signature).is_err());
    assert!(verifier.verify(&zip, b"not-a-signature").is_err());

    let url = serve_asset(zip.clone());
    let work = tempfile::tempdir().expect("work");
    let downloaded = work.path().join("asset.zip");

    // Download the release asset from the mock.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async {
        let downloader = FileDownloader::new(DownloaderOptions {
            timeout: Duration::from_secs(10),
            max_bytes: 16 * 1024 * 1024,
            ..DownloaderOptions::default()
        })
        .expect("downloader");
        downloader
            .download(
                &DownloadRequest::new(url, downloaded.clone()),
                &CancellationToken::new(),
            )
            .await
            .expect("download");
    });
    let bytes = std::fs::read(&downloaded).expect("read downloaded");
    assert_eq!(bytes, zip);
    // The downloaded bytes are the ones verified above.
    verifier
        .verify(&bytes, &signature)
        .expect("verify download");

    // Flat install root with a stub runner that must never be created/executed.
    let root = tempfile::tempdir().expect("root");
    let layout = AppInstallLayout::new(root.path().to_path_buf(), "v2rayn_desktop.exe");
    std::fs::write(layout.runner_exe(), b"stub").expect("runner stub");
    std::fs::write(layout.app_exe(), b"old-app").expect("old app");
    std::fs::write(root.path().join("keep.txt"), b"keep").expect("keep");

    // Stage, then overlay; the predecessor lands under app.previous.
    let staged = stage_zip(root.path(), "7.99.0", &downloaded);
    let outcome = apply_app_upgrade(&layout, "7.99.0", &staged).expect("apply");
    assert_eq!(
        std::fs::read(layout.app_exe()).expect("new app"),
        b"NEW-APP"
    );
    assert_eq!(
        outcome.kept_previous.as_deref(),
        Some(layout.previous_dir().as_path())
    );
    // Restart command is constructed only (flat exe, install root as cwd).
    assert_eq!(outcome.restart.program, layout.app_exe());
    assert_eq!(outcome.restart.working_dir, root.path());
    assert!(outcome.restart.args.is_empty());
    // Unmanaged user file survives; the stub runner is untouched.
    assert!(root.path().join("keep.txt").is_file());
    assert_eq!(std::fs::read(layout.runner_exe()).expect("stub"), b"stub");

    // Rollback restores the previous install byte-for-byte.
    rollback_app_upgrade(&layout).expect("rollback");
    assert_eq!(
        std::fs::read(layout.app_exe()).expect("restored"),
        b"old-app"
    );
    assert!(root.path().join("keep.txt").is_file());
}
