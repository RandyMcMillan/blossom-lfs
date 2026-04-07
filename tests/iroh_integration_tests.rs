//! Iroh transport integration tests for blossom-lfs daemon.
//!
//! Tests iroh-only mode: the daemon talks directly to a blossom-rs iroh
//! QUIC server without any HTTP intermediary.

#![cfg(feature = "iroh")]

use base64::Engine;
use blossom_rs::access::OpenAccess;
use blossom_rs::auth::Signer;
use blossom_rs::db::MemoryDatabase;
use blossom_rs::locks::MemoryLockDatabase;
use blossom_rs::storage::MemoryBackend;
use blossom_rs::transport::{BlossomProtocol, IrohState, BLOSSOM_ALPN};
use blossom_rs::MemoryLfsVersionDatabase;
use iroh::endpoint::presets::N0;
use iroh::protocol::Router;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::Mutex;

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn repo_b64(repo_path: &std::path::Path) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(repo_path.to_string_lossy().as_bytes())
}

fn setup_git_repo_iroh_only(endpoint_id_str: &str, nsec_hex: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init failed");

    std::process::Command::new("git")
        .args(["remote", "add", "origin", "https://myrepo"])
        .current_dir(repo_path)
        .output()
        .expect("git remote add failed");

    let config = format!(
        "iroh-endpoint={}\ntransport=iroh\nprivate-key={}",
        endpoint_id_str, nsec_hex
    );
    std::fs::write(repo_path.join(".lfsdalconfig"), config).unwrap();

    dir
}

async fn spawn_iroh_lfs_server() -> (iroh::EndpointAddr, Router) {
    let state = Arc::new(Mutex::new(IrohState {
        backend: Box::new(MemoryBackend::new()),
        database: Box::new(MemoryDatabase::new()),
        access: Box::new(OpenAccess),
        base_url: "iroh://test".to_string(),
        max_upload_size: None,
        require_auth: false,
        lock_db: Some(Box::new(MemoryLockDatabase::new())),
        lfs_version_db: Some(Box::new(MemoryLfsVersionDatabase::new())),
    }));

    let endpoint = iroh::Endpoint::builder(N0)
        .bind()
        .await
        .expect("bind server endpoint");

    let addr = endpoint.addr();

    let router = Router::builder(endpoint)
        .accept(BLOSSOM_ALPN, Arc::new(BlossomProtocol::new(state)))
        .spawn();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    (addr, router)
}

async fn spawn_lfs_daemon(port: u16) {
    tokio::spawn(blossom_lfs::daemon::run_daemon(port));
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}

async fn find_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_iroh_upload_and_download() {
    let (server_addr, _router) = spawn_iroh_lfs_server().await;
    let signer = Signer::generate();

    let endpoint_id_str = server_addr.id.to_string();
    let repo_dir = setup_git_repo_iroh_only(&endpoint_id_str, &signer.secret_key_hex());
    let repo_b64 = repo_b64(repo_dir.path());

    let daemon_port = find_port().await;
    spawn_lfs_daemon(daemon_port).await;

    let daemon_url = format!("http://127.0.0.1:{}", daemon_port);
    let http = reqwest::Client::new();

    let data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
    let oid = sha256_hex(&data);

    let resp = http
        .put(format!("{}/lfs/{}/objects/{}", daemon_url, repo_b64, oid))
        .body(data.clone())
        .send()
        .await
        .unwrap();
    if resp.status() != 200 {
        let body = resp.text().await.unwrap();
        eprintln!("UPLOAD FAILED: status body = {}", body);
        panic!("iroh upload should succeed");
    }

    let resp = http
        .get(format!("{}/lfs/{}/objects/{}", daemon_url, repo_b64, oid))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "iroh download should succeed");
    let downloaded = resp.bytes().await.unwrap();
    assert_eq!(
        &downloaded[..],
        &data[..],
        "iroh round-trip content mismatch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_iroh_batch_upload_download() {
    let (server_addr, _router) = spawn_iroh_lfs_server().await;
    let signer = Signer::generate();

    let endpoint_id_str = server_addr.id.to_string();
    let repo_dir = setup_git_repo_iroh_only(&endpoint_id_str, &signer.secret_key_hex());
    let repo_b64 = repo_b64(repo_dir.path());

    let daemon_port = find_port().await;
    spawn_lfs_daemon(daemon_port).await;

    let daemon_url = format!("http://127.0.0.1:{}", daemon_port);
    let http = reqwest::Client::new();

    let data: Vec<u8> = (0..8000).map(|i| (i % 128) as u8).collect();
    let oid = sha256_hex(&data);

    let resp = http
        .post(format!("{}/lfs/{}/objects/batch", daemon_url, repo_b64))
        .json(&serde_json::json!({
            "operation": "upload",
            "objects": [{"oid": oid, "size": data.len()}]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let batch: serde_json::Value = resp.json().await.unwrap();
    let upload_url = batch["objects"][0]["actions"]["upload"]["href"]
        .as_str()
        .unwrap()
        .to_string();
    let verify_url = batch["objects"][0]["actions"]["verify"]["href"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = http
        .put(&upload_url)
        .body(data.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "batch upload via iroh should succeed");

    let resp = http.post(&verify_url).send().await.unwrap();
    assert_eq!(resp.status(), 200, "verify via iroh should succeed");

    let resp = http
        .post(format!("{}/lfs/{}/objects/batch", daemon_url, repo_b64))
        .json(&serde_json::json!({
            "operation": "download",
            "objects": [{"oid": oid, "size": data.len()}]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let download_url = resp.json::<serde_json::Value>().await.unwrap()["objects"][0]["actions"]
        ["download"]["href"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = http.get(&download_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let downloaded = resp.bytes().await.unwrap();
    assert_eq!(
        &downloaded[..],
        &data[..],
        "batch download via iroh mismatch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_iroh_lock_lifecycle() {
    let (server_addr, _router) = spawn_iroh_lfs_server().await;
    let signer = Signer::generate();

    let endpoint_id_str = server_addr.id.to_string();
    let repo_dir = setup_git_repo_iroh_only(&endpoint_id_str, &signer.secret_key_hex());
    let repo_b64 = repo_b64(repo_dir.path());

    let daemon_port = find_port().await;
    spawn_lfs_daemon(daemon_port).await;

    let daemon_url = format!("http://127.0.0.1:{}", daemon_port);
    let http = reqwest::Client::new();

    let resp = http
        .post(format!("{}/lfs/{}/locks", daemon_url, repo_b64))
        .json(&serde_json::json!({"path": "big-file.bin"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "lock create via iroh should succeed");
    let lock_resp: serde_json::Value = resp.json().await.unwrap();
    let lock_id = lock_resp["lock"]["id"].as_str().unwrap().to_string();
    assert!(!lock_id.is_empty());

    let resp = http
        .get(format!("{}/lfs/{}/locks", daemon_url, repo_b64))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let locks: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(locks["locks"].as_array().unwrap().len(), 1);

    let resp = http
        .post(format!("{}/lfs/{}/locks/verify", daemon_url, repo_b64))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let verify: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(verify["ours"].as_array().unwrap().len(), 1);
    assert_eq!(verify["theirs"].as_array().unwrap().len(), 0);

    let resp = http
        .post(format!(
            "{}/lfs/{}/locks/{}/unlock",
            daemon_url, repo_b64, lock_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "unlock via iroh should succeed");

    let resp = http
        .get(format!("{}/lfs/{}/locks", daemon_url, repo_b64))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let locks: serde_json::Value = resp.json().await.unwrap();
    assert!(
        locks["locks"].as_array().unwrap().is_empty(),
        "lock should be gone after unlock"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_iroh_multiple_blobs() {
    let (server_addr, _router) = spawn_iroh_lfs_server().await;
    let signer = Signer::generate();

    let endpoint_id_str = server_addr.id.to_string();
    let repo_dir = setup_git_repo_iroh_only(&endpoint_id_str, &signer.secret_key_hex());
    let repo_b64 = repo_b64(repo_dir.path());

    let daemon_port = find_port().await;
    spawn_lfs_daemon(daemon_port).await;

    let daemon_url = format!("http://127.0.0.1:{}", daemon_port);
    let http = reqwest::Client::new();

    let blobs: Vec<Vec<u8>> = vec![vec![0x11; 6000], vec![0x22; 8000], vec![0x33; 4000]];

    let mut oids = Vec::new();
    for data in &blobs {
        let oid = sha256_hex(data);
        oids.push(oid.clone());

        let resp = http
            .put(format!("{}/lfs/{}/objects/{}", daemon_url, repo_b64, oid))
            .body(data.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "upload blob via iroh should succeed");
    }

    for (i, oid) in oids.iter().enumerate() {
        let resp = http
            .get(format!("{}/lfs/{}/objects/{}", daemon_url, repo_b64, oid))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let downloaded = resp.bytes().await.unwrap();
        assert_eq!(
            &downloaded[..],
            &blobs[i][..],
            "blob {} content mismatch via iroh",
            i
        );
    }
}
