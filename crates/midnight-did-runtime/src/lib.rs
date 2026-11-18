//! Native Rust implementation of the Midnight DID Method.

#![forbid(unsafe_code)]
// The Rust AST backend emits a public generated surface without rustdoc.
// Hand-written public APIs remain documented, but the crate-level lint cannot
// distinguish them from the verbatim generated artifact included below.
#![allow(missing_docs)]
#![warn(rust_2018_idioms, clippy::all)]

/// Crate version reported by the build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Compact's Rust AST backend emits a crate root: generated references use
// `crate::types`, `crate::pure_circuits`, and `crate::ledger_contract`.
// Keep the generated artifact verbatim and include it at this crate's root;
// `contract` below provides the stable namespaced re-export used by consumers.
include!("contract/generated.rs");

pub mod backend;
pub mod contract;
pub mod contract_call;
pub mod contract_wrapper;
pub mod identifier_bridge;
pub mod ledger9_signing;
pub mod operation_journal;
pub mod state_decode;

pub use backend::{
    Backend, BackendError, BuiltTx, DidDeploymentRequest, FinalizedTxData, Ledger9DidProvider, LedgerFinalityReceipt,
    LedgerIndexerProvider, LiveBackend, LiveProviders, RecordingBackend, ResolverBackend,
};
pub use contract_call::{
    DidContractCall, DidLedgerSnapshot, JubjubPointHex, LedgerPublicKeyJwk, LedgerSchnorrJubjubVerificationMethod,
    LedgerService, LedgerVerificationMethod, LedgerVerificationMethodRelation, MapMutation, NewJubjubPointHex,
    SchnorrJubjubDigest, SchnorrJubjubSignature, SetMutation, ValidationError,
};
pub use contract_wrapper::Contract;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_compiles() {
        assert!(!VERSION.is_empty());
    }
}
