// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Indexer-backed **read-only** [`Backend`] for the Midnight DID method.
//!
//! This is "Approach A" of the LiveBackend track (issue #4): resolution
//! goes live by reading contract state from the Midnight indexer's
//! GraphQL API — the same `contractAction(address){state}` query the TS
//! `indexer-public-data-provider` issues — and decoding it through
//! [`midnight_did_runtime::state_decode`]. Writes stay unsupported
//! (`submit_tx` → [`BackendError::ReadOnly`]); the wallet + proof-server
//! write path is tracked separately (issue #9).
//!
//! ```no_run
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use midnight_did_indexer::{IndexerBackend, IndexerClient};
//!
//! let client = IndexerClient::new("http://127.0.0.1:8088/api/v4/graphql")?;
//! let backend = IndexerBackend::new(client, "00aabb…contract-address-hex…");
//! // hand `backend` to `Contract::new(backend, addr, network)` and resolve.
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

use async_trait::async_trait;
use midnight_did_runtime::backend::{Backend, BackendError, BuiltTx, FinalizedTxData};
use midnight_did_runtime::contract_call::DidLedgerSnapshot;
use midnight_did_runtime::state_decode::{charged_state_from_bytes, snapshot_from_bytes};
use serde::Deserialize;
use serde_json::json;

/// Fetch the state and the address of the indexer action that supplied it.
const CONTRACT_STATE_QUERY: &str = "query CONTRACT_STATE_QUERY($address: HexEncoded!, $offset: ContractActionOffset) {\n  contractAction(address: $address, offset: $offset) {\n    address\n    state\n  }\n}";

/// Errors from the indexer HTTP/GraphQL exchange.
#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    /// Transport-level failure (connect, timeout, non-2xx).
    #[error("indexer transport: {0}")]
    Transport(String),
    /// The indexer answered, but with GraphQL errors.
    #[error("indexer GraphQL: {0}")]
    GraphQl(String),
    /// The response body didn't match the expected shape.
    #[error("indexer response: {0}")]
    Malformed(String),
    /// The indexer returned an action for a different contract.
    #[error("indexed action address {actual} does not match requested address {expected}")]
    AddressMismatch {
        /// Address returned by the indexer.
        actual: String,
        /// Address requested by the caller.
        expected: String,
    },
}

impl From<IndexerError> for BackendError {
    fn from(e: IndexerError) -> Self {
        if matches!(e, IndexerError::AddressMismatch { .. }) {
            BackendError::Decode(e.to_string())
        } else {
            BackendError::Network(e.to_string())
        }
    }
}

/// Minimal GraphQL-over-HTTP client for the Midnight indexer.
#[derive(Debug, Clone)]
pub struct IndexerClient {
    http: reqwest::Client,
    graphql_url: String,
}

impl IndexerClient {
    /// Build a client for the indexer's GraphQL endpoint
    /// (e.g. `http://127.0.0.1:8088/api/v4/graphql`).
    pub fn new(graphql_url: impl Into<String>) -> Result<Self, IndexerError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        Ok(Self {
            http,
            graphql_url: graphql_url.into(),
        })
    }

    /// The configured GraphQL endpoint URL.
    pub fn graphql_url(&self) -> &str {
        &self.graphql_url
    }

    /// Fetch the latest serialized contract state for `address_hex`.
    ///
    /// Returns `Ok(None)` when the indexer knows no contract action for
    /// the address (the TS provider's `null` — maps to *notFound*).
    pub async fn contract_state_bytes(&self, address_hex: &str) -> Result<Option<Vec<u8>>, IndexerError> {
        let body = json!({
            "query": CONTRACT_STATE_QUERY,
            "variables": { "address": address_hex, "offset": null },
        });
        let response = self
            .http
            .post(&self.graphql_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(IndexerError::Transport(format!("HTTP {status}")));
        }
        let payload: GraphQlResponse = response
            .json()
            .await
            .map_err(|e| IndexerError::Malformed(e.to_string()))?;
        parse_contract_state_response(payload, address_hex)
    }

    /// Fetch and decode a DID snapshot bound to the requested contract action.
    ///
    /// RC1 constructors can leave the stored ID unset. In that case the
    /// indexer's action address binds the snapshot to the requested DID.
    pub async fn did_snapshot(&self, address_hex: &str) -> Result<Option<DidLedgerSnapshot>, BackendError> {
        let Some(bytes) = self.contract_state_bytes(address_hex).await? else {
            return Ok(None);
        };
        let snapshot = snapshot_from_bytes(&bytes)?;
        validate_snapshot_subject(&snapshot.id_hex, address_hex)?;
        Ok(Some(snapshot))
    }
}

fn validate_snapshot_subject(stored: &str, address: &str) -> Result<(), BackendError> {
    const ZERO_ID: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    if stored != ZERO_ID && !stored.eq_ignore_ascii_case(address) {
        return Err(BackendError::Decode(format!(
            "indexed contract id {stored} does not match requested address {address}"
        )));
    }
    Ok(())
}

/// Read-only [`Backend`] over an [`IndexerClient`] + contract address.
#[derive(Debug, Clone)]
pub struct IndexerBackend {
    client: IndexerClient,
    address_hex: String,
}

impl IndexerBackend {
    /// Bind a client to one contract address (hex, no `0x` prefix).
    pub fn new(client: IndexerClient, address_hex: impl Into<String>) -> Self {
        Self {
            client,
            address_hex: address_hex.into(),
        }
    }

    /// The bound contract address (hex).
    pub fn address_hex(&self) -> &str {
        &self.address_hex
    }

    async fn fetch_state_bytes(&self) -> Result<Vec<u8>, BackendError> {
        self.client
            .contract_state_bytes(&self.address_hex)
            .await
            .map_err(BackendError::from)?
            .ok_or_else(|| {
                BackendError::Other(format!(
                    "no contract state for address {} (not deployed?)",
                    self.address_hex
                ))
            })
    }
}

#[async_trait]
impl Backend for IndexerBackend {
    async fn submit_tx(&self, _tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        Err(BackendError::ReadOnly)
    }

    async fn read_state(
        &self,
    ) -> Result<midnight_did_runtime::backend::RawChargedState<midnight_did_runtime::backend::RawDb>, BackendError>
    {
        charged_state_from_bytes(&self.fetch_state_bytes().await?)
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        self.client.did_snapshot(&self.address_hex).await?.ok_or_else(|| {
            BackendError::Other(format!(
                "no contract state for address {} (not deployed?)",
                self.address_hex
            ))
        })
    }
}

// ─────────────────────────────────────────────────────────────────────
// Response parsing (pure — unit-tested without HTTP)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GraphQlResponse {
    #[serde(default)]
    data: Option<ContractActionData>,
    #[serde(default)]
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct ContractActionData {
    #[serde(rename = "contractAction")]
    contract_action: Option<ContractAction>,
}

#[derive(Debug, Deserialize)]
struct ContractAction {
    address: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

fn parse_contract_state_response(
    payload: GraphQlResponse,
    expected_address: &str,
) -> Result<Option<Vec<u8>>, IndexerError> {
    if let Some(errors) = payload.errors
        && !errors.is_empty()
    {
        let joined = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("; ");
        return Err(IndexerError::GraphQl(joined));
    }
    let Some(data) = payload.data else {
        return Err(IndexerError::Malformed("response has neither data nor errors".into()));
    };
    let Some(action) = data.contract_action else {
        return Ok(None); // unknown address → notFound
    };
    if !action
        .address
        .trim_start_matches("0x")
        .eq_ignore_ascii_case(expected_address)
    {
        return Err(IndexerError::AddressMismatch {
            actual: action.address,
            expected: expected_address.into(),
        });
    }
    let bytes = hex::decode(action.state.trim_start_matches("0x"))
        .map_err(|e| IndexerError::Malformed(format!("state field is not hex: {e}")))?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json_str: &str) -> Result<Option<Vec<u8>>, IndexerError> {
        let payload: GraphQlResponse = serde_json::from_str(json_str).expect("test JSON parses");
        parse_contract_state_response(payload, "aa")
    }

    #[test]
    fn parses_state_hex() {
        let got = parse(r#"{"data":{"contractAction":{"address":"aa","state":"00ff10"}}}"#).unwrap();
        assert_eq!(got, Some(vec![0x00, 0xff, 0x10]));
    }

    #[test]
    fn a_present_action_requires_the_requested_address() {
        assert!(matches!(
            parse(r#"{"data":{"contractAction":{"address":"bb","state":"00"}}}"#),
            Err(IndexerError::AddressMismatch { .. })
        ));
        assert!(serde_json::from_str::<GraphQlResponse>(r#"{"data":{"contractAction":{"state":"00"}}}"#).is_err());
    }

    #[test]
    fn nonzero_state_id_must_match_the_action() {
        assert!(validate_snapshot_subject(&"aa".repeat(32), &"aa".repeat(32)).is_ok());
        assert!(validate_snapshot_subject(&"bb".repeat(32), &"aa".repeat(32)).is_err());
    }

    #[test]
    fn null_contract_action_is_none() {
        let got = parse(r#"{"data":{"contractAction":null}}"#).unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn graphql_errors_are_surfaced() {
        let err = parse(r#"{"errors":[{"message":"boom"},{"message":"bang"}]}"#).unwrap_err();
        assert!(matches!(err, IndexerError::GraphQl(m) if m == "boom; bang"));
    }

    #[test]
    fn non_hex_state_is_malformed() {
        let err = parse(r#"{"data":{"contractAction":{"address":"aa","state":"zz"}}}"#).unwrap_err();
        assert!(matches!(err, IndexerError::Malformed(_)));
    }

    #[test]
    fn empty_body_is_malformed() {
        let err = parse(r#"{}"#).unwrap_err();
        assert!(matches!(err, IndexerError::Malformed(_)));
    }

    #[tokio::test]
    async fn submit_tx_is_read_only() {
        let client = IndexerClient::new("http://127.0.0.1:1/api/v4/graphql").unwrap();
        let backend = IndexerBackend::new(client, "00");
        let err = backend.submit_tx(BuiltTx { bytes: vec![] }).await.unwrap_err();
        assert!(matches!(err, BackendError::ReadOnly));
    }
}
