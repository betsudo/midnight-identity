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
```

The DID runtime pins the signed Rust AST compiler/runtime source to
`betsudo/compact@d6513a24debac14d43a8199460d9dc2c6d9af2b1`.
Its compiler-emitted receipt selects one coordinated Ledger 9.1.0.2 source
graph: `betsudo/midnight-ledger@6fbc2e3900845a858f70eebf8a0deab1d4d3c0d6`
and `betsudo/midnight-zk@bca8db4d9e2c1ecaa5d1db5ba7a201935b6c6c8a`.
That graph contains `midnight-ledger-v9=1.0.2`, `midnight-storage=3.1.0`,
`midnight-proofs=0.8.2`, and `midnight-zk-stdlib=2.4.0`. The repository pins
the compiler-only ZKIR parser to 3.2.1 from
`betsudo/midnight-zkir@3377ad9e48741f60e0aa17010b6f23e34223d906`
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

Immutable Git resolution now works with no application-root ZK patch table.
The patch-free ZK source graph carries `blake2b_halo2` and `sha3-circuit`
alongside the coordinated `midnight-curves` and `midnight-proofs` packages, so
a clean consumer resolves one active proof cohort directly from the pinned
Ledger and Compact dependencies. Inside `nix develop .#rust`, local builds may
opt into already materialised Nix sources without changing any published
manifest:

```console
scripts/cargo-with-nix-overrides.sh check -p midnight-did-runtime --features ledger-transaction
```

The wrapper substitutes only the exact Compact runtime source materialized by
Nix. Ledger and proof dependencies retain their immutable Git source identities.
