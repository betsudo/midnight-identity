# VC Ledger 9 and DigitalPassport Rust compatibility note

Issue: [#108](https://github.com/MediaNoxLabs/midnight-identity/issues/108)
Date: 2026-11-18

## Decision

Use the published VC 1.0 release-candidate cohort as the authoritative Ledger 9
core and DID-binding source. Keep DigitalPassport rc5 source semantics intact,
but stage it against that exact core and generate Rust with the same AST backend
and Ledger graph used by the DID runtime.

This is a downstream compatibility qualification, not a claim that
`midnight-vc-passport` has released Ledger 9 support. No public Ledger 9 branch
or migration PR exists in that repository as of this date.

## Exact source graph

| Surface | Selected source |
|---|---|
| VC Core Compact/model | `@midnight-ntwrk/credential-compact@1.0.0-rc1`, npm tarball SHA-256 `3aeb66d81540f31d957bcaa54eda1f2ae2aa024c8451b381fbca22d0c2bb61fe`; upstream tag commit `62b21892652957d681cd3d11d1d3c0ade502d98e` |
| Midnight DID binding | `@midnight-ntwrk/credential-did-midnight@1.0.0-rc1`, npm tarball SHA-256 `abfd6ff02fd6284a1c9aa4a5d6a6ae48e957b493d8da042775222d6d0ac9df28` |
| DigitalPassport | `@midnight-ntwrk/midnight-vc-passport@0.1.0-rc5`, npm tarball SHA-256 `bfb6e0de4c56108e06edd9b700672fc44ac9aa44d688f5fcf82a457fae4e177d`; public `develop` inspected at `2e13b029` |
| Rust AST compiler/runtime | `betsudo/compact@d6513a24debac14d43a8199460d9dc2c6d9af2b1` |
| Ledger | `betsudo/midnight-ledger@6fbc2e3900845a858f70eebf8a0deab1d4d3c0d6` |
| ZK | `betsudo/midnight-zk@bca8db4d9e2c1ecaa5d1db5ba7a201935b6c6c8a` |
| ZKIR selector | `3377ad9e48741f60e0aa17010b6f23e34223d906` |

The Passport family is pure Compact (its generated ledger is empty). Its rc5
entry point compiles without source edits when the bundled core staging folder
is replaced with VC Core 1.0.0-rc1. This makes a bounded compatibility overlay
possible without forking the family contract.

## Crate ownership

| Crate | Responsibility | Boundary |
|---|---|---|
| `midnight-vc-domain` | Pure Rust schema descriptors and verifier status vocabulary | No Midnight runtime dependencies; wasm-clean |
| `midnight-vc-runtime` | Generated VC/VP, holder, status-binding, signer-authorization, and proof circuits | Exact Ledger 9 generated-code cone |
| `midnight-vc-did-binding` | Generated DID-binding circuits plus resolution-to-binding adapter | Public adapter uses fixed bytes and domain DID types; generated Ledger types remain under `contract` |
| `midnight-vc-families` | DigitalPassport claim, protocol, disclosure, predicate, VC and VP circuits | Feature-gated family overlay; no product policy |
| `midnight-vc-proof` | Issuance signing, bounded wire parsing, and authenticated family verification | Custody supplies key material; resolver/trust/status policy stays outside |

The new AST generator emits crate-root references. Handwritten wrappers mount
each generated unit at crate root and re-export its previous
`contract::<surface>` namespace, preserving existing callers while keeping the
generated files reproducible and untouched.

## Parity and refusals

- Existing DigitalPassport claim commitments, claim roots, VC/VP body roots,
  issuance/presentation challenges, validation decisions, and detached proof
  fixtures remain byte-for-byte unchanged under Ledger 9.
- The DID adapter refuses off-chain DIDs, subject mismatch, deactivated DIDs,
  absent/non-positive state versions, missing relationship authorization,
  foreign controllers, non-`JsonWebKey` methods, and non-EC/Jubjub keys.
- The 1.0 DID method identifier is raw `SHA-256(UTF8(canonical #fragment))`.
  It must not be replaced by the older off-chain holder-binding domain tag.
- VC Core 1.0 provides `RegistryBoundStatusBinding` and registry-reference root
  primitives. Live registry evaluation and freshness/trust policy are external
  ports. DigitalPassport rc5 still selects `NoStatusBinding`; this slice does
  not invent Passport revocation semantics.

## IR and proof qualification

The exact compiler compatibility report selects runtime ABI 54, IR schema 20,
Ledger `9.1.0.2`, and `midnight-zkir=3.2.1` at
`3377ad9e48741f60e0aa17010b6f23e34223d906`. The selected ZKIR binary was built
from that revision and reports `midnight-zkir 3.2.1`. A compiler run with the
binary on `PATH` produced byte-identical Rust to the reproducible `--skip-zk`
run.

No ZKIR source, proving key, or verifier key is expected for this cohort. The
three contracts export 148 circuits in total, and their authoritative
`contract-info.json` files classify every circuit as pure, witness-free, and
`proof: false`:

| Contract | Exported circuits | Proof-required | Witnesses | Ledger entries |
|---|---:|---:|---:|---:|
| VC Core | 34 | 0 | 0 | 0 |
| DID binding | 39 | 0 | 0 | 0 |
| DigitalPassport | 75 | 0 | 0 | 0 |

The generated Rust IR SHA-256 digests are
`164cbf27bc543a13c477017dfdc5b69d99227bd9409c3d767fee74a06317aa99`
(VC Core),
`5783007e39b9861e0fec40479b6d09a91e89164b1c945ac018d9d587884c0c93`
(DID binding), and
`05cc460f45c528d4e67e9c4ad133476f369c631b5d4a39e9d4edb538c460df84`
(DigitalPassport).

The `Proof` values in this API are detached Jubjub signatures checked by pure
circuits; they are not Ledger transaction proofs. Consequently, detached-proof
signing, verification, tamper refusal, and upstream vector parity are the
applicable proof gates. A proof-server or funded Ledger transaction would not
exercise additional behavior in these empty-ledger contracts.

## Qualification and rollback

Local qualification covers exact-tarball regeneration, focused Rust tests,
golden vectors, malformed/tampered proofs, DID-binding refusal tests, clippy,
formatting, codegen drift, metadata source inspection, and applicable factory
gates. No standalone chain is required because every migrated contract in this
slice is pure, witness-free, and has an empty ledger. Native Linux remains a
hosted-CI qualification; local results are from Apple Silicon macOS.

Rollback is a coordinated reversion of the three npm pins, generated files,
crate-local Ledger 9 pins, AST wrapper layout, and lockfile. Do not independently
downgrade only the family or proof crate: Rust types from separate Ledger
revisions are intentionally incompatible.
