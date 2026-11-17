<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-did-runtime

Codegen target and contract-transport layer for the Midnight DID Method
Rust port. This is the only crate in the workspace that touches
`midnight-compact-runtime` and the Ledger 9 contract state directly.

## What lives here

- `contract/generated.rs` — **generated** by the Rust-codegen branch of
  the [Compact compiler](https://github.com/MediaNoxLabs/compact)
  (`compactc --target rust --skip-zk`) from the vendored
  [`midnight-did` v1.0.0-rc1](https://github.com/midnightntwrk/midnight-did/releases/tag/v1.0.0-rc1)
  contract. Never hand-edit it; regenerate with `just codegen` and gate
  with `just codegen-check` (CI does).
- `contract_wrapper` — the concrete `Contract<B: Backend>` wrapper: one
  inherent method per exported circuit (create / rotate / recover /
  set/remove verification methods, relations, services, alsoKnownAs /
  deactivate), each encoding a typed call for its backend
  ([ADR 0008](../../doc/adr/0008-contract-abstraction-reform.md)).
- `contract_call` — `DidContractCall` enum + `DidLedgerSnapshot`: the
  typed wire representation of a contract invocation and the decoded
  ledger state.
- `backend` — the 3-method `Backend` trait and its implementations:
  - `RecordingBackend` — in-memory mock; every `submit_tx` is decoded
    back into a typed `DidContractCall` for assertion in tests.
  - `ResolverBackend` — serves a stored `DidLedgerSnapshot` for
    read-only resolution flows.
  - `LiveBackend` — provider-composed Ledger 9 boundary. A
    `Ledger9DidProvider` owns custody, generated-circuit execution, positive-fee
    transaction construction, proving, submission, and finality; the indexer
    provider supplies independently decoded state and restart reconciliation.
- `assets/rust-compatibility.json` — compiler-emitted ABI and exact dependency
  receipt for Ledger 9.1.0.2 / runtime ABI 54.
- `ledger9_signing` — the TypeScript-compatible controller/recovery signer on
  Ledger 9 curve types. The Ledger 8 helper remains isolated for VC consumers.
- `examples/generate_ledger9_fixture.rs` — deterministic regeneration of the
  v8 tagged state capture used by indexer and resolver integration tests.

## Position in the workspace

`midnight-did-api` builds operations *on top of* `Contract<B>`; this
crate owns the contract surface itself. Consumers that only need DID
parsing or document types should depend on `midnight-did-method` /
`midnight-did-domain` instead — this crate drags in halo2/arkworks
transitively and is not wasm-clean.

## Immutable Git consumption

External Cargo workspaces consume this crate from a full 40-character revision
of this repository:

```toml
[dependencies]
midnight-did-runtime = {
  git = "https://github.com/MediaNoxLabs/midnight-identity.git",
  rev = "<immutable-midnight-identity-revision>",
  features = ["ledger-transaction"]
}

# Temporary application-root source contract for the Ledger 9 proof cohort.
# Cargo does not propagate a library's [patch] table to its consumers.
[patch.crates-io]
midnight-circuits-ledger9 = { package = "midnight-circuits", version = "=7.3.0", git = "https://github.com/betsudo/midnight-zk", rev = "0f9d5c3879f92a8beae8bdb9e7305f39e03a3227" }
midnight-curves-ledger9 = { package = "midnight-curves", version = "=0.3.1", git = "https://github.com/betsudo/midnight-zk", rev = "0f9d5c3879f92a8beae8bdb9e7305f39e03a3227" }
midnight-proofs-ledger9 = { package = "midnight-proofs", version = "=0.8.2", git = "https://github.com/betsudo/midnight-zk", rev = "0f9d5c3879f92a8beae8bdb9e7305f39e03a3227" }
midnight-zk-stdlib-ledger9 = { package = "midnight-zk-stdlib", version = "=2.4.0", git = "https://github.com/betsudo/midnight-zk", rev = "0f9d5c3879f92a8beae8bdb9e7305f39e03a3227" }
```

The DID runtime pins the signed Rust AST compiler/runtime source to
`betsudo/compact@05c9ed69b891baa4f15777403c9bdc8bc86c150c`.
Its compiler-emitted receipt selects one coordinated Ledger 9.1.0.2 source
graph: `betsudo/midnight-ledger@f82ac287442fc15c4eb79f02dcb6510373979eab`
and `betsudo/midnight-zk@0f9d5c3879f92a8beae8bdb9e7305f39e03a3227`.
That graph contains `midnight-ledger-v9=1.0.2`, `midnight-storage=3.1.0`,
`midnight-proofs=0.8.2`, and `midnight-zk-stdlib=2.4.0`. The repository pins
the compiler-only ZKIR parser to 3.2.1 from
`betsudo/midnight-zkir@35e386b7b3f3a76117423ca72883c852181104dc`
and tests all twelve emitted ZKIR v3 sources. VC and Passport remain on their
existing Compact/Ledger 8 evidence
graph; their types never cross the DID runtime boundary.

The supported external-consumer feature cone is native Rust only:

- the default feature set is empty;
- `ledger-transaction` enables the generated observed-call transaction helpers
  and is exercised by the clean temporary-consumer check;
- `http` and `node-subxt` are retained compatibility feature names while
  concrete transport remains an injected provider concern.

`midnight-did-uniffi` is not in this cone and its mock handle is not a
production API. This runtime cone is also not advertised as wasm-clean. The
patched `midnight-storage=3.1.0` boundary removes `sysinfo` from the normal
dependency cone, so the exact graph is checked for iOS simulator and Android
without introducing a target-specific fork.

The standalone probe discovers an already-running shared Ledger 9 stack via
`scripts/midnight-localnet.sh attach` and uses the indexer's `/api/v4/graphql`
endpoint. Only its designated owner may invoke `up` or `down`; owner `up`
attaches first and will not start a duplicate compatible stack.

Immutable Git resolution plus the explicit application-root ZK patch table is
the portable external-consumer contract. The patch is required because the
currently published `blake2b_halo2` and `sha3-circuit` helpers request
`midnight-curves` and `midnight-proofs` from crates.io, while the Ledger 9
runtime must use the coordinated Git cohort. Remove it only after those helper
releases carry the same source graph. Inside `nix develop .#rust`, local builds
may opt into already materialised Nix sources without changing any published
manifest:

```console
scripts/cargo-with-nix-overrides.sh check -p midnight-did-runtime --features ledger-transaction
```

The wrapper substitutes only the exact Compact runtime source materialized by
Nix. Ledger and proof dependencies retain their immutable Git source identities.
