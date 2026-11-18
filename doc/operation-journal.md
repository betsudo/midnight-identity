<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# DID operation journal contract

`midnight_did_runtime::operation_journal` defines a portable durability port,
not a concrete crash-durable adapter. The test-only mutex model demonstrates
state-machine conformance, not crash persistence. Existing backends are not
implicitly journaled by introducing this port.

## Required execution ordering

1. Atomically reserve a globally unique operation ID and the contract before
   signing. Only an acknowledged fresh reservation permits its caller to start
   preparation once. A constructed or loaded record is not that authorization.
2. Persist `prepared` with the public transaction identity once preparation is
   complete. Never persist signed transaction bytes, proofs, witnesses, call
   arguments, keys, signatures, or free-form custody errors in the journal.
3. Durably change `prepared` to `pending` with the SAME transaction identity
   BEFORE attempting submission. Submission is attempted once by that caller.
4. Reconcile pending work by public transaction identity. Timeouts, cancellation,
   missing indexer data, and storage errors are not evidence of rejection and
   never permit resubmission, new signing, or releasing the contract lock.
5. Only confirmed finality for that SAME identity permits `finalized`.

Reserved and prepared work may instead become `rejected` only when it is
certain no submission could have occurred. Loaded reservations authorize
reconciliation only, NOT re-signing. A crash during signing can leave a reserved
record indefinitely; safety takes precedence over automatic recovery/liveness.
No retry/reset/delete/lease-expiry API is provided. Finalized and rejected IDs
remain consumed permanently, although their contract lock is released.

## Adapter obligations

`reserve`, `load`, and full-record compare-and-swap must be linearizable across
all handles/processes using the same durable store. One store namespace is one
network; deployments must not alias contracts on different networks. All
nonterminal work for one contract is serialized. Full-record CAS and the closed
transition validator must run INSIDE the storage transaction; a read/check/write
sequence without atomicity is insufficient. No ABA transition is possible in
the closed forward-only matrix. Refusals must leave records and locks unchanged.

Before acknowledging any successful mutation, the adapter must durably flush
records, consumed-ID history, and contract-lock changes as one atomic commit.
After an ambiguous storage failure or cancellation, reload and reconcile rather
than infer that an ID is absent. Never discard corrupt records, unsupported
schemas, missing history, or inconsistent locks to restore availability. Return
a closed `Corrupt` error and fail closed. A concrete adapter needs independent
crash/fault-injection and multiprocess conformance evidence before making a
durability claim. Reconstructing a set of individual records cannot prove the
completeness of consumed-ID history; the adapter must protect that history too.

Schema version `"1"` accepts only fixed-size public operation/contract/transaction
identifiers, closed states, and closed rejection codes. Unknown versions, states,
and fields are rejected. Debug output redacts all identifiers; serialized
records deliberately retain public identifiers for reconciliation and are NOT
log-safe. Identifier constructors are semantic boundaries: callers must supply
only public identifiers, never repurpose their fixed bytes to store secrets.
No arbitrary metadata, error strings, or opaque payload fields exist.

## Non-goals

Generated-call conversion; custody signing/zeroization; filesystem/database
adapters; DUST, proof, node, indexer, and funded lifecycle integration; ledger
graph changes; VC/Passport; consumer/UI/release work. This slice does not claim
to enforce journal usage by existing signing or submission APIs. Integration
must preserve these ordering rules and must not treat load as a signing grant.
