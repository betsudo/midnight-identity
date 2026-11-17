// SPDX-License-Identifier: Apache-2.0

//! Environment-gated Ledger 9 standalone protocol probe.
//!
//! This verifies service compatibility before a custody implementation is
//! allowed to claim a funded DID lifecycle. Transaction construction remains
//! behind `Ledger9DidProvider`; this probe never submits a serialized SDK DTO.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn split_http_url(url: &str) -> (&str, &str) {
    let rest = url
        .strip_prefix("http://")
        .unwrap_or_else(|| panic!("only http:// standalone URLs are supported by this probe: {url}"));
    rest.split_once('/')
        .map_or((rest, "/"), |(host, path)| (host, &url[url.find(path).unwrap() - 1..]))
}

fn http_request(method: &str, url: &str, body: Option<&str>, content_type: &str) -> (u16, String) {
    let (host, path) = split_http_url(url);
    let mut stream = TcpStream::connect(host).unwrap_or_else(|error| panic!("connect {host}: {error}"));
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    let payload = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("malformed HTTP response from {url}: {response:?}"));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    (status, body.to_owned())
}

fn rpc(node_url: &str, method: &str, params: serde_json::Value) -> serde_json::Value {
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    let (status, response) = http_request("POST", node_url, Some(&body), "application/json");
    assert_eq!(status, 200, "{method} HTTP status {status}: {response}");
    let value: serde_json::Value = serde_json::from_str(&response).expect("JSON-RPC response");
    assert!(value.get("error").is_none(), "{method} returned error: {value}");
    value["result"].clone()
}

#[test]
fn ledger9_standalone_protocol_probe() {
    if env("MIDNIGHT_DID_LEDGER9_STANDALONE").as_deref() != Some("1") {
        eprintln!("skip: set MIDNIGHT_DID_LEDGER9_STANDALONE=1 with node/indexer/proof URLs");
        return;
    }

    let node = env("MIDNIGHT_DID_LEDGER9_NODE_URL").expect("MIDNIGHT_DID_LEDGER9_NODE_URL");
    let indexer = env("MIDNIGHT_DID_LEDGER9_INDEXER_URL").expect("MIDNIGHT_DID_LEDGER9_INDEXER_URL");
    let proof = env("MIDNIGHT_DID_LEDGER9_PROOF_URL").expect("MIDNIGHT_DID_LEDGER9_PROOF_URL");

    let health = rpc(&node, "system_health", serde_json::json!([]));
    assert_eq!(health["isSyncing"], false, "node is syncing: {health}");
    let ledger_version = rpc(&node, "midnight_ledgerVersion", serde_json::json!([]));
    assert!(
        ledger_version.to_string().contains('9'),
        "expected Ledger 9, got {ledger_version}"
    );

    let indexer_url = format!("{}/api/v4/graphql", indexer.trim_end_matches('/'));
    let schema_body = serde_json::json!({"query":"{ __schema { queryType { fields { name } } } }"}).to_string();
    let (indexer_status, indexer_response) = http_request("POST", &indexer_url, Some(&schema_body), "application/json");
    assert_eq!(indexer_status, 200, "indexer HTTP {indexer_status}: {indexer_response}");
    assert!(
        indexer_response.contains("contractAction"),
        "indexer lacks contractAction"
    );

    let (proof_status, proof_body) = http_request(
        "GET",
        &format!("{}/health", proof.trim_end_matches('/')),
        None,
        "application/json",
    );
    assert_eq!(proof_status, 200, "proof health HTTP {proof_status}: {proof_body}");
}
