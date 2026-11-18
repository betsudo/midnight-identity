// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

use super::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Conformance model ONLY: no disk durability claim. The map itself retains
// terminal IDs and derives contract locks, avoiding a second source of truth.
#[derive(Default)]
struct ModelStore(Mutex<HashMap<OperationId, OperationRecord>>);

#[async_trait::async_trait]
impl OperationStore for ModelStore {
    async fn reserve(&self, operation: OperationId, contract: ContractId) -> Result<OperationRecord, StoreError> {
        let mut records = self.0.lock().unwrap();
        if records.contains_key(&operation) {
            return Err(StoreError::Consumed);
        }
        if records
            .values()
            .any(|r| r.contract() == contract && !r.state().is_terminal())
        {
            return Err(StoreError::ContractBusy);
        }
        let record = OperationRecord::reservation(operation, contract);
        records.insert(operation, record.clone());
        Ok(record)
    }

    async fn load(&self, operation: OperationId) -> Result<Option<OperationRecord>, StoreError> {
        Ok(self.0.lock().unwrap().get(&operation).cloned())
    }

    async fn compare_and_swap(
        &self,
        expected: &OperationRecord,
        next: OperationState,
    ) -> Result<OperationRecord, StoreError> {
        let mut records = self.0.lock().unwrap();
        if records.get(&expected.operation()) != Some(expected) {
            return Err(StoreError::Stale);
        }
        let record = expected.transition(next)?;
        records.insert(record.operation(), record.clone());
        Ok(record)
    }
}

const OP: OperationId = OperationId::new([1; 32]);
const CONTRACT: ContractId = ContractId::new([2; 32]);
const TX: TransactionId = TransactionId::new([3; 32]);
const OTHER: TransactionId = TransactionId::new([4; 32]);

#[test]
fn serialization_reload_and_debug_redaction() {
    let mut record = OperationRecord::reservation(OP, CONTRACT);
    for state in [
        OperationState::Reserved,
        OperationState::Prepared { transaction: TX },
        OperationState::Pending { transaction: TX },
        OperationState::Finalized { transaction: TX },
        OperationState::Rejected {
            reason: RejectionReason::Cancelled,
        },
    ] {
        record.state = state;
        let encoded = serde_json::to_string(&record).unwrap();
        assert_eq!(serde_json::from_str::<OperationRecord>(&encoded).unwrap(), record);
        let debug = format!("{record:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("[1, 1"));
        assert!(!debug.contains("[2, 2"));
        assert!(!debug.contains("[3, 3"));
    }
}

#[test]
fn unknown_schema_fields_and_state_are_refused() {
    let record = OperationRecord::reservation(OP, CONTRACT);
    let value = serde_json::to_value(record).unwrap();
    for (field, replacement) in [
        ("schema_version", serde_json::json!("2")),
        ("secret", serde_json::json!("forbidden")),
        ("state", serde_json::json!({"status":"retry"})),
        ("state", serde_json::json!({"status":"reserved", "signature": []})),
        (
            "state",
            serde_json::json!({"status":"prepared", "transaction": vec![3; 32], "proof": []}),
        ),
    ] {
        let mut corrupt = value.clone();
        corrupt[field] = replacement;
        assert!(serde_json::from_value::<OperationRecord>(corrupt).is_err());
    }
}

#[test]
fn nested_payload_fields_and_identifier_shapes_are_closed() {
    for state in [
        OperationState::Prepared { transaction: TX },
        OperationState::Pending { transaction: TX },
        OperationState::Finalized { transaction: TX },
        OperationState::Rejected {
            reason: RejectionReason::Cancelled,
        },
    ] {
        let mut record = OperationRecord::reservation(OP, CONTRACT);
        record.state = state;
        let valid = serde_json::to_value(&record).unwrap();
        // Start with a VALID payload so failure proves nested field rejection,
        // not merely a missing required transaction/reason.
        let mut corrupt = valid.clone();
        corrupt["state"]["data"]["custody_error"] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<OperationRecord>(corrupt).is_err());
        let mut corrupt = valid.clone();
        corrupt["state"]["extra"] = serde_json::json!([]);
        assert!(serde_json::from_value::<OperationRecord>(corrupt).is_err());
        for field in ["operation", "contract"] {
            let mut corrupt = valid.clone();
            corrupt[field] = serde_json::json!([1, 2]);
            assert!(serde_json::from_value::<OperationRecord>(corrupt).is_err());
        }
        if state
            != (OperationState::Rejected {
                reason: RejectionReason::Cancelled,
            })
        {
            let mut corrupt = valid;
            corrupt["state"]["data"]["transaction"] = serde_json::json!("not an identifier");
            assert!(serde_json::from_value::<OperationRecord>(corrupt).is_err());
        }
    }
    assert_eq!(OP.as_bytes(), &[1; 32]);
    assert_eq!(CONTRACT.as_bytes(), &[2; 32]);
    assert_eq!(TX.as_bytes(), &[3; 32]);
    for error in [
        StoreError::Consumed,
        StoreError::ContractBusy,
        StoreError::Stale,
        StoreError::InvalidTransition,
        StoreError::Corrupt,
        StoreError::Unavailable,
    ] {
        assert!(!format!("{error:?}: {error}").contains("[1, 1"));
    }
}

#[test]
fn exhaustive_matrix_refuses_rollback_and_identity_changes() {
    use OperationState::*;
    let states = [
        Reserved,
        Prepared { transaction: TX },
        Pending { transaction: TX },
        Finalized { transaction: TX },
        Rejected {
            reason: RejectionReason::Cancelled,
        },
    ];
    for (from, state) in states.iter().enumerate() {
        let mut record = OperationRecord::reservation(OP, CONTRACT);
        record.state = *state;
        for (to, next) in states.iter().enumerate() {
            let allowed = matches!((from, to), (0, 1 | 4) | (1, 2 | 4) | (2, 3));
            assert_eq!(record.transition(*next).is_ok(), allowed, "{from}->{to}");
        }
    }
    let prepared = OperationRecord::reservation(OP, CONTRACT)
        .transition(Prepared { transaction: TX })
        .unwrap();
    assert_eq!(
        prepared.transition(Pending { transaction: OTHER }),
        Err(StoreError::InvalidTransition)
    );
    let pending = prepared.transition(Pending { transaction: TX }).unwrap();
    assert_eq!(
        pending.transition(Finalized { transaction: OTHER }),
        Err(StoreError::InvalidTransition)
    );
}

#[tokio::test]
async fn restart_duplicates_stale_cas_ambiguity_and_terminal_release() {
    use OperationState::*;
    let store = ModelStore::default();
    let reserved = store.reserve(OP, CONTRACT).await.unwrap();
    // Simulate a restart by reconstructing the model solely from serialized records.
    let bytes = serde_json::to_vec(&vec![reserved.clone()]).unwrap();
    let reloaded: Vec<OperationRecord> = serde_json::from_slice(&bytes).unwrap();
    *store.0.lock().unwrap() = reloaded.into_iter().map(|r| (r.operation(), r)).collect();
    assert_eq!(store.reserve(OP, CONTRACT).await, Err(StoreError::Consumed));
    let second = OperationId::new([5; 32]);
    assert_eq!(store.reserve(second, CONTRACT).await, Err(StoreError::ContractBusy));
    let prepared = store
        .compare_and_swap(&reserved, Prepared { transaction: TX })
        .await
        .unwrap();
    assert_eq!(
        store
            .compare_and_swap(
                &reserved,
                Rejected {
                    reason: RejectionReason::Cancelled
                }
            )
            .await,
        Err(StoreError::Stale)
    );
    let pending = store
        .compare_and_swap(&prepared, Pending { transaction: TX })
        .await
        .unwrap();
    // A restart after an ambiguous submission preserves the pending lock.
    let records: Vec<_> = store.0.lock().unwrap().values().cloned().collect();
    let reloaded: Vec<OperationRecord> = serde_json::from_slice(&serde_json::to_vec(&records).unwrap()).unwrap();
    *store.0.lock().unwrap() = reloaded.into_iter().map(|r| (r.operation(), r)).collect();
    assert_eq!(store.reserve(OP, CONTRACT).await, Err(StoreError::Consumed));
    for next in [
        Reserved,
        Prepared { transaction: TX },
        Pending { transaction: TX },
        Rejected {
            reason: RejectionReason::PreparationFailed,
        },
        Finalized { transaction: OTHER },
    ] {
        assert_eq!(
            store.compare_and_swap(&pending, next).await,
            Err(StoreError::InvalidTransition)
        );
        assert_eq!(store.load(OP).await.unwrap(), Some(pending.clone()));
        assert_eq!(store.reserve(second, CONTRACT).await, Err(StoreError::ContractBusy));
    }
    let finalized = store
        .compare_and_swap(&pending, Finalized { transaction: TX })
        .await
        .unwrap();
    assert_eq!(
        store.reserve(OP, ContractId::new([6; 32])).await,
        Err(StoreError::Consumed)
    );
    assert_eq!(store.load(OP).await.unwrap(), Some(finalized));
    let next = store.reserve(second, CONTRACT).await.unwrap();
    store
        .compare_and_swap(
            &next,
            Rejected {
                reason: RejectionReason::Cancelled,
            },
        )
        .await
        .unwrap();
    assert_eq!(store.reserve(second, CONTRACT).await, Err(StoreError::Consumed));
    assert!(store.reserve(OperationId::new([7; 32]), CONTRACT).await.is_ok());
}

#[tokio::test]
async fn rejection_from_prepared_releases_contract_not_id() {
    let store = ModelStore::default();
    let reserved = store.reserve(OP, CONTRACT).await.unwrap();
    let prepared = store
        .compare_and_swap(&reserved, OperationState::Prepared { transaction: TX })
        .await
        .unwrap();
    store
        .compare_and_swap(
            &prepared,
            OperationState::Rejected {
                reason: RejectionReason::PreparationFailed,
            },
        )
        .await
        .unwrap();
    assert_eq!(store.reserve(OP, CONTRACT).await, Err(StoreError::Consumed));
    assert!(store.reserve(OperationId::new([8; 32]), CONTRACT).await.is_ok());
}

#[test]
fn concurrent_reservation_has_one_winner_for_id_and_contract() {
    for same_id in [true, false] {
        let store = Arc::new(ModelStore::default());
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let store = Arc::clone(&store);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
                    barrier.wait();
                    runtime.block_on(store.reserve(if same_id { OP } else { OperationId::new([i; 32]) }, CONTRACT))
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(Result::is_ok)
                .count(),
            1
        );
        assert_eq!(store.0.lock().unwrap().len(), 1);
    }
}
