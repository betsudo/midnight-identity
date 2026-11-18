// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Non-secret reconciliation records and the durable operation-store contract.
//! See `doc/operation-journal.md` for crash boundaries and adapter obligations.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! identifier {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from a public identifier, never key or custody material.
            pub const fn new(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
            /// Public bytes for storage and reconciliation, not logging.
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    };
}

identifier!(
    OperationId,
    "Globally unique, permanently consumed operation identifier."
);
identifier!(
    ContractId,
    "Public contract identity in the adapter's single network namespace."
);
identifier!(
    TransactionId,
    "Public transaction identity used for finality reconciliation."
);

/// Only supported on-disk schema. Unknown versions fail deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaVersion {
    /// First journal schema.
    #[serde(rename = "1")]
    V1,
}

/// Closed rejection codes; never attach free-form custody errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    /// Work was cancelled before any possible submission.
    Cancelled,
    /// Local preparation failed before any possible submission.
    PreparationFailed,
}

/// Reconciliation state. Prepared work already has its final transaction identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationState {
    /// Durably reserved before signing; reload does not authorize signing.
    Reserved,
    /// Preparation completed, but submission has not started.
    Prepared {
        /// Public prepared transaction identity.
        transaction: TransactionId,
    },
    /// Persisted BEFORE attempting submission, including ambiguous outcomes.
    Pending {
        /// The only transaction that may be reconciled.
        transaction: TransactionId,
    },
    /// Finality has been independently confirmed for this same transaction.
    Finalized {
        /// Confirmed transaction identity.
        transaction: TransactionId,
    },
    /// Definitively abandoned before submission; ID remains consumed.
    Rejected {
        /// Closed, non-secret reason.
        reason: RejectionReason,
    },
}

impl OperationState {
    /// Whether this state releases the contract lock, never the operation ID.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Finalized { .. } | Self::Rejected { .. })
    }
}

/// Closed persisted record. No call arguments, signatures, proofs, or secret data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationRecord {
    schema_version: SchemaVersion,
    operation: OperationId,
    contract: ContractId,
    state: OperationState,
}

impl OperationRecord {
    /// Candidate for atomic insertion only; constructing this is NOT a signing grant.
    pub const fn reservation(operation: OperationId, contract: ContractId) -> Self {
        Self {
            schema_version: SchemaVersion::V1,
            operation,
            contract,
            state: OperationState::Reserved,
        }
    }

    /// Permanent operation identity.
    pub const fn operation(&self) -> OperationId {
        self.operation
    }
    /// Contract whose nonterminal work is serialized.
    pub const fn contract(&self) -> ContractId {
        self.contract
    }
    /// Current reconciliation state.
    pub const fn state(&self) -> OperationState {
        self.state
    }

    /// Build a successor without mutating this record. Stores must independently
    /// enforce this check within the atomic compare-and-swap transaction.
    pub fn transition(&self, next: OperationState) -> Result<Self, StoreError> {
        use OperationState::{Finalized, Pending, Prepared, Rejected, Reserved};
        let valid = match (self.state, next) {
            (Reserved, Prepared { .. } | Rejected { .. }) | (Prepared { .. }, Rejected { .. }) => true,
            (Prepared { transaction: old }, Pending { transaction: new })
            | (Pending { transaction: old }, Finalized { transaction: new }) => old == new,
            _ => false,
        };
        if !valid {
            return Err(StoreError::InvalidTransition);
        }
        Ok(Self {
            state: next,
            ..self.clone()
        })
    }
}

/// Closed store failures with no secret-bearing error strings or identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// Operation IDs are never reusable, even after terminal completion.
    #[error("operation ID already consumed")]
    Consumed,
    /// Another nonterminal operation owns the contract.
    #[error("contract has unresolved work")]
    ContractBusy,
    /// The expected record is absent or no longer current.
    #[error("stale compare-and-swap")]
    Stale,
    /// The proposed state is outside the closed transition matrix.
    #[error("invalid journal transition")]
    InvalidTransition,
    /// Corrupt, unsupported, or inconsistent durable state; fail closed.
    #[error("journal corruption")]
    Corrupt,
    /// Storage unavailable; outcome may be ambiguous. Reload, never blindly retry signing.
    #[error("journal storage unavailable")]
    Unavailable,
}

/// Portable durability port, NOT an implementation or crash-durability claim.
///
/// Implementations MUST provide linearizable transactions across ALL handles and
/// processes in one network namespace. Successful writes require durable flush
/// of records, permanently consumed IDs, and contract-lock changes together.
/// Acknowledging a buffered write is forbidden. Cancellation/I/O failure can be
/// ambiguous: callers must reload and reconcile, never infer permission to sign.
/// Corrupt records, unknown schemas, missing tombstones, or inconsistent locks
/// must fail closed, not be skipped or treated as an empty store.
///
/// No delete/reset/retry API exists. Only a successful fresh `reserve` authorizes
/// its caller to begin preparation ONCE; loaded Reserved records authorize only
/// reconciliation or definitive pre-submission rejection. A pending record is
/// persisted before the first submission attempt and must never be resubmitted
/// automatically, rejected, or unlocked merely because finality is unavailable.
#[async_trait::async_trait]
pub trait OperationStore: Send + Sync {
    /// Atomically consume a fresh ID and acquire its contract lock, returning
    /// only after durable flush. Duplicate IDs and occupied contracts leave
    /// storage unchanged. All history, including terminal IDs, must be checked.
    async fn reserve(&self, operation: OperationId, contract: ContractId) -> Result<OperationRecord, StoreError>;

    /// Load for reconciliation, NEVER as a fresh signing/preparation grant.
    /// `None` means a positively verified absent ID, not a read/decode failure.
    async fn load(&self, operation: OperationId) -> Result<Option<OperationRecord>, StoreError>;

    /// Atomically compare the complete stored record to `expected`, validate
    /// `expected.transition(next)`, persist the successor and flush before
    /// success. Refusals leave storage unchanged. Terminal transitions release
    /// ONLY the contract lock; the operation ID remains consumed forever.
    async fn compare_and_swap(
        &self,
        expected: &OperationRecord,
        next: OperationState,
    ) -> Result<OperationRecord, StoreError>;
}

#[cfg(test)]
#[path = "operation_journal_tests.rs"]
mod tests;
