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
`05c9ed69b891baa4f15777403c9bdc8bc86c150c` from
`betsudo/compact`. It identifies Compact 0.31.215, language 0.28.0,
TypeScript runtime 0.18.0, runtime ABI 54, IR schema 20, Rust 1.99, and
the Ledger 9.1.0.2 compatibility matrix recorded in
`crates/midnight-did-runtime/assets/rust-compatibility.json`.

## Decision

The migration is DID-only:

- generated DID Rust and all twelve ZKIR sources are regenerated from the
  v1.0.0-rc1 contract source;
- the DID runtime uses `midnight-compact-runtime` 0.2.0 from the exact compiler
  revision and a single coordinated source graph at
  `betsudo/midnight-ledger@f82ac287442fc15c4eb79f02dcb6510373979eab`
  plus `betsudo/midnight-zk@0f9d5c3879f92a8beae8bdb9e7305f39e03a3227`;
- that graph supplies `midnight-ledger-v9=1.0.2`,
  `midnight-storage=3.1.0`, and `midnight-transient-crypto=3.1.0` without
  mixing registry copies of the same Ledger 9 packages;
- ZKIR sources are parsed with `midnight-zkir=3.2.1` from
  `betsudo/midnight-zkir@35e386b7b3f3a76117423ca72883c852181104dc`;
  the Ledger-private ZKIR v2 boundary remains `2.2.0` at the same Ledger
  revision and never crosses the DID crate API;
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
- VC and Passport retain their existing compiler/runtime evidence. Their
  contracts, package pins, and behavior are unchanged; the two committed VC
  Rust bindings receive only the import-order normalization produced by the
  pinned Rust 1.99 formatter so the existing drift gate remains reproducible.

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

The repository's WASM-clean crates compile on `wasm32-unknown-unknown`; the DID
runtime remains a native-only crate. The coordinated Ledger 9.1.0.2 source
graph incorporates the storage correction from `betsudo/midnight-ledger#42`:
`midnight-storage=3.1.0` excludes `sysinfo` from normal builds and confines it
to `stress-test`. Consequently the DID runtime dependency cone is checked for
both `aarch64-apple-ios-sim` and `aarch64-linux-android` using Rust 1.99. The
external-consumer smoke test proves that a fresh Git consumer receives the
same immutable graph; it does not rely on a downstream-invisible local patch.

The 2026-11-16 local arm64 receipt passed exact DID regeneration, workspace
formatting, lint with warnings denied, all repository tests (678 first-party
and 10 feature-complete VC-family tests), the 87% coverage gate at 91.26%, the
WASM-clean package check, and `nix flake check --no-build`. The VC regeneration
comparison remains semantically unchanged. Stable Rust 1.99 moves
`std::marker::PhantomData` below the generated glob import because the
repository's nightly-only import grouping options are ignored; the committed
bindings now include that two-line formatting normalization so local and hosted
drift checks agree.

Native Linux remains a hosted qualification. A fresh proof receipt on the
matching Compact source qualifies `deactivate` with a distinct positive fee,
independent verification, state application, and replay refusal. Of the twelve
exported stateful circuits, all ZKIR v3 sources parse and the repository also
executes `setAlsoKnownAs` through the generated Rust VM; the other circuits are
compile/lifecycle-qualified only and are not claimed as proof-qualified.

## Rollback

Rollback means reverting this DID-only commit and its generated artifacts.
Ledger 9 state must not be decoded or submitted through the former Ledger 8
runtime, and a deployed Ledger 9 DID cannot be downgraded in place. VC and
Passport require no rollback because their graph is unchanged.
