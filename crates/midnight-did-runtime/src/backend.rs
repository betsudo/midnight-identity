//! Portable I/O boundary for the native Rust DID contract.
//!
//! Ledger 9 contract execution is generated in [`crate::ledger_contract`].
//! Wallet custody, proving, submission, and indexer transport remain behind
//! injected ports. No Ledger 8 transaction or state type crosses this API.

use std::fmt;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

pub use midnight_compact_runtime::ledger::{ChargedState as RawChargedState, DefaultDB as RawDb};
use midnight_compact_runtime::ledger::{ChargedState, DefaultDB, StateValue};

use crate::contract_call::{DidContractCall, DidLedgerSnapshot};

fn empty_charged_state<D: midnight_compact_runtime::ledger::DB>() -> ChargedState<D> {
    ChargedState::new(StateValue::Null)
}

/// A transaction request accepted by a [`Backend`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuiltTx {
    /// Backend-owned bytes. The portable wrapper encodes a validated
    /// [`DidContractCall`]; a live provider must turn it into a real Ledger 9
    /// contract call rather than submit these bytes directly.
    pub bytes: Vec<u8>,
}

/// Durable finality data returned to SDK callers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinalizedTxData {
    /// Ledger transaction hash.
    pub tx_hash: String,
    /// Finalized block height.
    pub block_height: u64,
}

/// Durable Ledger 9 receipt retained for restart reconciliation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LedgerFinalityReceipt {
    /// Ledger transaction hash.
    pub tx_hash: String,
    /// Finalized block height.
    pub block_height: u64,
    /// Provider-specific finality cursor/token, when available.
    pub finality_token: Option<String>,
}

impl From<LedgerFinalityReceipt> for FinalizedTxData {
    fn from(value: LedgerFinalityReceipt) -> Self {
        Self {
            tx_hash: value.tx_hash,
            block_height: value.block_height,
        }
    }
}

/// Errors raised by a backend or provider boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Network or RPC failure.
    Network(String),
    /// Contract envelope or ledger state could not be decoded.
    Decode(String),
    /// The backend is read-only.
    ReadOnly,
    /// A required live provider is absent.
    Unconfigured(&'static str),
    /// The requested operation is unsupported by the provider.
    Unsupported(String),
    /// The operation timed out.
    Timeout(String),
    /// The operation was cancelled.
    Cancelled(String),
    /// Finality could not be reconciled with indexer state.
    Reconciliation(String),
    /// Catch-all provider failure.
    Other(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(message) => write!(f, "backend network failure: {message}"),
            Self::Decode(message) => write!(f, "backend decode failure: {message}"),
            Self::ReadOnly => write!(f, "backend is read-only"),
            Self::Unconfigured(provider) => write!(f, "live backend provider is not configured: {provider}"),
            Self::Unsupported(message) => write!(f, "unsupported live backend operation: {message}"),
            Self::Timeout(message) => write!(f, "backend timeout: {message}"),
            Self::Cancelled(message) => write!(f, "backend operation cancelled: {message}"),
            Self::Reconciliation(message) => write!(f, "backend reconciliation failure: {message}"),
            Self::Other(message) => write!(f, "backend error: {message}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// I/O seam shared by mutable SDK calls and read-only resolution.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Submit a validated contract-call request.
    async fn submit_tx(&self, tx: BuiltTx) -> Result<FinalizedTxData, BackendError>;

    /// Read raw Ledger 9 Compact state.
    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError>;

    /// Read a portable DID snapshot.
    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError>;
}

/// Public, non-secret input for deploying a DID contract.
///
/// Controller and recovery secrets remain inside the injected provider's
/// custody/witness boundary. The generated constructor obtains the contract
/// identifier from `kernel.self()` and enforces distinct controller/recovery
/// public keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DidDeploymentRequest {
    /// Optional client correlation id; never interpreted by the contract.
    pub correlation_id: Option<String>,
}

/// Ledger 9 custody/proof/submission boundary.
///
/// Implementations must invoke the generated Rust constructor or recorded
/// circuit API, build a positive-fee Ledger 9 transaction, prove it, submit it,
/// and wait for durable finality. A serialized [`DidContractCall`] is not a
/// production ledger transaction.
#[async_trait]
pub trait Ledger9DidProvider: Send + Sync {
    /// Deploy a DID using custody-owned controller/recovery state.
    async fn deploy(&self, request: DidDeploymentRequest) -> Result<LedgerFinalityReceipt, BackendError>;

    /// Execute, prove, submit, and finalize one generated DID circuit call.
    async fn execute(&self, call: DidContractCall) -> Result<LedgerFinalityReceipt, BackendError>;
}

/// Indexer/public-data boundary for Ledger 9.
#[async_trait]
pub trait LedgerIndexerProvider: Send + Sync {
    /// Read serialized `ContractState` bytes.
    async fn read_state_bytes(&self) -> Result<Vec<u8>, BackendError>;

    /// Decode raw contract state.
    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        crate::state_decode::charged_state_from_bytes(&self.read_state_bytes().await?)
    }

    /// Decode the portable DID snapshot.
    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        crate::state_decode::decode_ledger_snapshot(&self.read_state().await?)
    }

    /// Confirm the finalized transaction is reflected by the indexer.
    async fn reconcile_finality(&self, receipt: &LedgerFinalityReceipt) -> Result<(), BackendError>;
}

/// Injected Ledger 9 provider bundle.
#[derive(Clone)]
pub struct LiveProviders {
    executor: Arc<dyn Ledger9DidProvider>,
    indexer: Arc<dyn LedgerIndexerProvider>,
}

impl fmt::Debug for LiveProviders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveProviders")
            .field("executor", &"<Ledger9DidProvider>")
            .field("indexer", &"<LedgerIndexerProvider>")
            .finish()
    }
}

impl LiveProviders {
    /// Create a live provider bundle.
    pub fn new(executor: Arc<dyn Ledger9DidProvider>, indexer: Arc<dyn LedgerIndexerProvider>) -> Self {
        Self { executor, indexer }
    }
}

/// Provider-composed production backend for Ledger 9.
#[derive(Debug, Default, Clone)]
pub struct LiveBackend {
    providers: Option<LiveProviders>,
}

impl LiveBackend {
    /// Construct an unconfigured backend. Operations return typed errors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a configured backend.
    pub fn with_providers(providers: LiveProviders) -> Self {
        Self {
            providers: Some(providers),
        }
    }

    fn providers(&self) -> Result<&LiveProviders, BackendError> {
        self.providers
            .as_ref()
            .ok_or(BackendError::Unconfigured("LiveProviders"))
    }

    /// Deploy a generated DID contract and reconcile its final state.
    pub async fn submit_deployment(&self, request: DidDeploymentRequest) -> Result<FinalizedTxData, BackendError> {
        let providers = self.providers()?;
        let receipt = providers.executor.deploy(request).await?;
        providers.indexer.reconcile_finality(&receipt).await?;
        Ok(receipt.into())
    }

    /// Reconcile a previously finalized receipt without resubmission.
    pub async fn reconcile_finality(&self, receipt: &LedgerFinalityReceipt) -> Result<(), BackendError> {
        self.providers()?.indexer.reconcile_finality(receipt).await
    }
}

#[async_trait]
impl Backend for LiveBackend {
    async fn submit_tx(&self, tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        let providers = self.providers()?;
        let call = DidContractCall::decode(&tx.bytes)?;
        let receipt = providers.executor.execute(call).await?;
        providers.indexer.reconcile_finality(&receipt).await?;
        Ok(receipt.into())
    }

    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        self.providers()?.indexer.read_state().await
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        self.providers()?.indexer.read_snapshot().await
    }
}

/// In-memory backend used by portable API tests.
pub struct RecordingBackend {
    calls: Mutex<Vec<DidContractCall>>,
    state: Mutex<ChargedState<DefaultDB>>,
    snapshot: Mutex<DidLedgerSnapshot>,
}

impl fmt::Debug for RecordingBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let calls_len = self.calls.lock().map(|calls| calls.len()).unwrap_or(0);
        f.debug_struct("RecordingBackend")
            .field("recorded_call_count", &calls_len)
            .field("state", &"<ChargedState<DefaultDB>>")
            .field("snapshot", &"<DidLedgerSnapshot>")
            .finish()
    }
}

impl Default for RecordingBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingBackend {
    /// Construct an empty recorder.
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            state: Mutex::new(empty_charged_state()),
            snapshot: Mutex::new(DidLedgerSnapshot::default()),
        }
    }

    /// Construct a recorder with raw public state.
    pub fn with_state(state: ChargedState<DefaultDB>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            state: Mutex::new(state),
            snapshot: Mutex::new(DidLedgerSnapshot::default()),
        }
    }

    /// Construct a recorder with a portable snapshot.
    pub fn with_snapshot(snapshot: DidLedgerSnapshot) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            state: Mutex::new(empty_charged_state()),
            snapshot: Mutex::new(snapshot),
        }
    }

    /// Return recorded calls in submission order.
    pub fn recorded_calls(&self) -> Vec<DidContractCall> {
        self.calls.lock().expect("recording backend lock poisoned").clone()
    }

    /// Replace raw public state.
    pub fn set_state(&self, state: ChargedState<DefaultDB>) {
        *self.state.lock().expect("recording backend lock poisoned") = state;
    }

    /// Replace the portable snapshot.
    pub fn set_snapshot(&self, snapshot: DidLedgerSnapshot) {
        *self.snapshot.lock().expect("recording backend lock poisoned") = snapshot;
    }
}

#[async_trait]
impl Backend for RecordingBackend {
    async fn submit_tx(&self, tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        let call = DidContractCall::decode(&tx.bytes)?;
        let mut calls = self
            .calls
            .lock()
            .map_err(|_| BackendError::Other("recording backend lock poisoned".into()))?;
        calls.push(call);
        Ok(FinalizedTxData {
            tx_hash: synth_tx_hash(&tx.bytes),
            block_height: calls.len() as u64,
        })
    }

    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| BackendError::Other("recording backend lock poisoned".into()))
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        self.calls
            .lock()
            .map_err(|_| BackendError::Other("recording backend lock poisoned".into()))?
            .push(DidContractCall::ReadLedger);
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| BackendError::Other("recording backend lock poisoned".into()))
    }
}

fn synth_tx_hash(bytes: &[u8]) -> String {
    use blake2::{Blake2b512, Digest};
    let mut hasher = Blake2b512::new();
    hasher.update(bytes);
    hex::encode(&hasher.finalize()[..8])
}

/// Read-only resolver backend.
pub struct ResolverBackend {
    /// Raw state returned by [`Backend::read_state`].
    pub state: ChargedState<DefaultDB>,
    /// Portable snapshot returned by [`Backend::read_snapshot`].
    pub snapshot: DidLedgerSnapshot,
}

impl fmt::Debug for ResolverBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolverBackend")
            .field("state", &"<ChargedState<DefaultDB>>")
            .field("snapshot", &"<DidLedgerSnapshot>")
            .finish()
    }
}

impl ResolverBackend {
    /// Construct from raw state.
    pub fn new(state: ChargedState<DefaultDB>) -> Self {
        Self {
            state,
            snapshot: DidLedgerSnapshot::default(),
        }
    }

    /// Construct from a portable snapshot.
    pub fn with_snapshot(snapshot: DidLedgerSnapshot) -> Self {
        Self {
            state: empty_charged_state(),
            snapshot,
        }
    }
}

#[async_trait]
impl Backend for ResolverBackend {
    async fn submit_tx(&self, _tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        Err(BackendError::ReadOnly)
    }

    async fn read_state(&self) -> Result<ChargedState<DefaultDB>, BackendError> {
        Ok(self.state.clone())
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        Ok(self.snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn recording_backend_records_validated_calls() {
        let backend = RecordingBackend::new();
        let receipt = runtime()
            .block_on(backend.submit_tx(BuiltTx {
                bytes: DidContractCall::Deactivate.encode(),
            }))
            .unwrap();
        assert_eq!(receipt.block_height, 1);
        assert_eq!(receipt.tx_hash.len(), 16);
        assert_eq!(backend.recorded_calls(), vec![DidContractCall::Deactivate]);
    }

    #[test]
    fn resolver_is_read_only() {
        let backend = ResolverBackend::with_snapshot(DidLedgerSnapshot::default());
        assert_eq!(
            runtime().block_on(backend.submit_tx(BuiltTx::default())),
            Err(BackendError::ReadOnly)
        );
    }

    #[test]
    fn unconfigured_live_backend_returns_typed_error() {
        let error = runtime().block_on(LiveBackend::new().read_state()).unwrap_err();
        assert_eq!(error, BackendError::Unconfigured("LiveProviders"));
    }
}
