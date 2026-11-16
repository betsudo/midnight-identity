# ADR 0012: Ledger 9 DID Rust AST cohort

- Status: accepted for the DID runtime trial
- Date: 2026-11-16

## Context

`midnight-did` v1.0.0-rc1 moves the TypeScript contract distribution to
Ledger 9 and Compact 0.36.0. The Rust contract is produced by the current Rust
AST compatibility line rather than the released TypeScript compiler. Those are
two different compiler products and their version banners must not be treated
as interchangeable.

The exact Rust source used here is signed commit
`7a4fab772ee34c19053ce293d08052b501ce4b50` from
`MediaNoxLabs/compact`. It emits runtime ABI 54, IR schema 20, Rust 1.99, and
the Ledger 9.1.0.1 compatibility matrix recorded in
`crates/midnight-did-runtime/assets/rust-compatibility.json`.

## Decision

The migration is DID-only:

- generated DID Rust and all twelve ZKIR sources are regenerated from the
  v1.0.0-rc1 contract source;
- the DID runtime uses `midnight-compact-runtime` 0.2.0 from the exact compiler
  revision and released `midnight-ledger-v9=1.0.1` dependencies;
- ZKIR sources are parsed with `midnight-zkir=3.2.1`; the release cohort was
  qualified with the compatible ZKIR 3.2 line;
- generated code is included at crate root because its references are
  intentionally crate-root-relative, while `contract` preserves a namespaced
  re-export;
- controller/recovery authorization signing is ported onto Ledger 9 curve
  types inside the DID runtime; the shared Ledger 8 helper remains unchanged
  for VC, and curve objects never cross the revision boundary;
- indexer and resolver fixtures are regenerated from two signed state changes
  and serialized with the Ledger 9 `contract-state[v8]` tag;
- `LiveBackend` accepts provider ports that must execute generated code and
  construct a real positive-fee Ledger 9 transaction. A `DidContractCall`
  envelope is never itself a production transaction;
- VC and Passport retain their existing compiler/runtime evidence. No VC code,
  contract, or artifact is regenerated in this change.

The official Ledger 9 package may retain older proof/transient packages as
private compatibility implementation details. They are permitted only inside
that package's dependency cone; first-party DID APIs expose the Ledger 9
runtime types and do not exchange objects across ledger eras.

## DID deployment semantics

Ledger 9 DIDs are new deployments at new contract addresses. There is no
in-place Ledger 8 state upgrade. The generated constructor preserves the
TypeScript rules: controller and recovery authority keys are custody-owned,
must be distinct, timestamps are witnessed, and the identifier comes from
`kernel.self()`.

## Qualification and limits

Local arm64 qualification covers constructor layout, controller/recovery
separation refusal, a signed stateful `setAlsoKnownAs` operation, replayable
public VM recording, stale replay refusal, state decoding, compatibility
receipt assertions, and parsing all twelve ZKIR v3 sources.

This does not by itself qualify a proof server, funded standalone transaction,
or native Linux release. A concrete `Ledger9DidProvider` must still supply
custody, proving keys, positive-fee balancing, proof generation, node finality,
and independent indexer reconciliation before live support is claimed.

The repository's WASM-clean crates compile on `wasm32-unknown-unknown`. The DID
runtime itself does not yet qualify for `aarch64-apple-ios-sim`: released
`midnight-storage=3.0.0` unconditionally enables `sysinfo=0.39.6`, whose Apple
backend imports macOS-only libc symbols when built for iOS. This is an upstream
dependency-cone blocker, not a generated DID contract failure. Do not restore
mobile claims until the Ledger 9 storage/runtime release makes that dependency
target-aware (or removes it from the mobile cone).

The 2026-11-16 local arm64 receipt passed exact DID regeneration, workspace
formatting, lint with warnings denied, all repository tests (678 first-party
and 10 feature-complete VC-family tests), the 87% coverage gate at 91.26%, the
WASM-clean package check, and `nix flake check --no-build`. The VC regeneration
comparison remains unchanged semantically but is not byte-stable under Rust
1.99: stable rustfmt moves `std::marker::PhantomData` below the generated glob
import because the repository's nightly-only import grouping options are
ignored. This DID migration deliberately does not accept that unrelated VC
generated-source churn.

Native Linux and a fresh proof-backed, positive-fee standalone transaction are
still required release qualifications. Of the twelve exported stateful
circuits, all ZKIR v3 sources parse, while only `setAlsoKnownAs` has been
executed end-to-end through the generated Rust VM in this trial.

## Rollback

Rollback means reverting this DID-only commit and its generated artifacts.
Ledger 9 state must not be decoded or submitted through the former Ledger 8
runtime, and a deployed Ledger 9 DID cannot be downgraded in place. VC and
Passport require no rollback because their graph is unchanged.
