//! Integration tests for the API server.
//!
//! Starts the server as a subprocess and sends real HTTP requests.

use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;

fn unused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start_server(port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_email_validator"))
        .arg("api")
        .arg(format!("127.0.0.1:{port}"))
        .spawn()
        .expect("failed to start server")
}

fn wait_for_server(port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if reqwest::get(format!("http://{addr}/health")).await.is_ok() {
                return;
            }
        }
        panic!("server did not start in time");
    });
}

#[test]
fn test_health_endpoint() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let resp = reqwest::get(format!("http://127.0.0.1:{port}/health"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["status"], "ok");
        assert!(json["version"].as_str().unwrap().len() > 0);
    });

    server.kill().ok();
}

#[test]
fn test_validate_single_regex() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let resp = reqwest::get(format!(
            "http://127.0.0.1:{port}/validate?email=test@example.com&method=regex"
        ))
        .await
        .unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["valid_count"], 1);
    });

    server.kill().ok();
}

#[test]
fn test_validate_batch_regex() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/validate"))
            .json(&serde_json::json!({
                "emails": ["a@b.de", "c@d.com"],
                "method": "regex"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["valid_count"], 2);
    });

    server.kill().ok();
}

#[test]
fn test_validate_batch_empty() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/validate"))
            .json(&serde_json::json!({"emails": []}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    });

    server.kill().ok();
}

#[test]
fn test_validate_batch_over_limit() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = reqwest::Client::new();
        let emails: Vec<String> = (0..1001).map(|i| format!("x{i}@y.com")).collect();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/validate"))
            .json(&serde_json::json!({"emails": emails}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 400);
    });

    server.kill().ok();
}

#[test]
fn test_validate_single_regex_invalid() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let resp = reqwest::get(format!(
            "http://127.0.0.1:{port}/validate?email=bogus&method=regex"
        ))
        .await
        .unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["valid_count"], 0);
        assert_eq!(json["invalid_count"], 1);
        assert_eq!(json["results"][0]["valid"], false);
    });

    server.kill().ok();
}

#[test]
fn test_validate_batch_regex_mixed_valid_invalid() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/validate"))
            .json(&serde_json::json!({
                "emails": ["a@b.de", "bogus", "c@d.com"],
                "method": "regex"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["total"], 3);
        assert_eq!(json["valid_count"], 2);
        assert_eq!(json["invalid_count"], 1);
    });

    server.kill().ok();
}

#[test]
fn test_validate_batch_deduplication() {
    let port = unused_port();
    let mut server = start_server(port);
    wait_for_server(port);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("http://127.0.0.1:{port}/validate"))
            .json(&serde_json::json!({
                "emails": ["a@b.de", "a@b.de", "A@B.DE"],
                "method": "regex"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["total"], 1, "duplicates should be collapsed");
    });

    server.kill().ok();
}
