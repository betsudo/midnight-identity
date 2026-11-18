//! Native Rust adapter for `@midnight-ntwrk/credential-did-midnight` 1.0.0-rc1.
//!
//! The public adapter surface is ledger-neutral: controller addresses, method
//! identifiers and canonical JWK coordinates are fixed bytes. Generated Compact
//! types remain available under [`contract`] for callers that explicitly need
//! circuit composition.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]
#![allow(missing_docs)] // generated crate-root items carry upstream names

include!("contract/did_midnight.rs");

pub mod adapter;
pub mod contract;

pub use adapter::{
    BindingError, MidnightDidHolderBinding, MidnightDidMethodBinding, VerificationRelationship,
    create_midnight_did_holder_binding, midnight_did_method_id, resolve_midnight_did_method_binding,
};
