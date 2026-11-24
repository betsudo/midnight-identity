<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Ledger 9 Midnight DID TypeScript/Rust E2E conformance

Date: 2026-11-24

## Qualified source graph

- Rust: `betsudo/midnight-identity@8456b88c2de636e81a27f7da6ba447145740eaff`
  plus the changes described here.
- TypeScript: `midnightntwrk/midnight-did@750268664278c162e9171406983dc45f24bb5be7`
  (`v1.0.0-rc1`), pinned at `third_party/midnight-did`.
- Ledger 9 standalone cohort used by the upstream suite: node `2.1.101`,
  indexer `4.4.101`, proof server `9.0.1`.
- Shared local stack observed during this qualification: node `2.1.101`,
  indexer `4.4.101`, proof server `9.0.2`, API v4, network `undeployed`.

`midnight-did` `main` is not the Ledger 9 reference. The pinned develop/tag
revision above is authoritative for this comparison.

## Conformance contract

The language-neutral fixture
`crates/midnight-did-api/tests/fixtures/ledger9-cross-language-conformance.json`
is consumed independently by Rust and TypeScript. It fixes:

- an Ed25519 verification method in `authentication`;
- a path-form P-256 verification method in `capabilityInvocation`;
- a native Schnorr-Jubjub method in `assertionMethod`;
- canonical unsigned big-endian base64url JWK coordinates while retaining
  little-endian Compact field storage internally;
- absolute DID URL relationship values;
- omitted absent `alsoKnownAs` and `service` fields;
- aliases, subject-bound and legacy foreign-DID services, service endpoints,
  metadata and versioning;
- deterministic Ed25519 and Jubjub signatures, including negative payload
  verification.

Run the fast two-language gate with:

```console
./scripts/ci/did-e2e-conformance.sh
```

Run the original TypeScript standalone lifecycle suite as well with:

```console
./scripts/ci/did-e2e-conformance.sh --live
```

The live mode intentionally uses the upstream suite unchanged from the pinned
submodule. It creates an isolated chain because the suite assumes a fresh
genesis-funded wallet and owns destructive lifecycle state.

## Upstream scenario mapping

| TypeScript `did.api.test.ts` scenario | Rust counterpart | Qualification |
| --- | --- | --- |
| deploy empty DID; join deployed DID | `create_did_seeds_active_slot_and_resolves_empty_document`, `initial_state_matches_ts_fixture` | portable API + resolver |
| rotate controller; subsequent update | `rotate_controller_key_then_add_and_remove_aka`, controller operation tests | portable API |
| recover controller; subsequent update | `controller_operations::tests::recovers_and_promotes_private_state` | portable API |
| stale version / stale controller refusal | `generated_ledger9::recorded_stateful_call_replays_public_program_and_refuses_stale_replay` | generated Rust contract, representative mutation |
| DID contexts, id and metadata | `resolved_document_carries_w3c_contexts`, `resolved_document_id_uses_canonical_midnight_did_format`, `resolved_document_metadata_has_version_and_timestamp` | resolver |
| Ed25519, P-256, X25519, secp256k1, BLS and opaque JWK mapping | mapper/domain tests; upstream live suite | domain/mapping; all-profile live qualification remains TS-owned |
| native Jubjub insert, resolve, signature/challenge | `cross_language_conformance`, `ts_parity`, `after_set_schnorr_jubjub_vm_insert_matches_ts_fixture` | byte-identical in both languages |
| reject Jubjub through opaque JWK map | `ledger_mappers::tests::rejects_jubjub_in_jwk_path` | portable API |
| add relation; reject duplicate; remove; reject missing | `verification_methods` integration suite | portable API |
| relation/curve compatibility | `relation_curve_rules_match_typescript` | portable API |
| refuse method removal while referenced | `remove_verification_method_refuses_referenced_method` and Schnorr equivalent | portable API |
| add/update/remove services | `did_api_end_to_end` service mutation fixtures | portable API + resolver |
| all DID Core endpoint variations | `resolves_all_did_core_service_endpoint_variations` | resolver |
| add/remove alias and reject invalid URI | `did_api_end_to_end` alias cases | portable API |
| wallet snapshot/restore | private-state restart and pending-finality recovery tests | custody-neutral equivalent; wallet-SDK history remains TS-specific |
| deactivate | `after_deactivate_matches_ts_fixture` | portable API + resolver |

## Behavioral corrections made by this parity pass

Rust now follows the current Ledger 9 TypeScript behavior for:

1. absolute relationship identifiers in resolved DID documents, including
   RFC 3986 path/query/fragment resolution and dot-segment removal;
2. omission of absent optional document properties while accepting legacy
   explicit `null` on input;
3. duplicate verification-method detection across both physical ledger maps;
4. required method existence before update, removal or relation insertion;
5. explicit relationship removal before verification-method removal;
6. X25519-only `keyAgreement` and non-X25519 signing relationships;
7. add/update/remove service existence rules;
8. big-endian JWK projection at the ledger/domain boundary;
9. canonical absolute DID URL writes for verification methods and services,
   with exact reads of the legacy `#fragment` key only when the canonical id
   is exactly `<current-subject>#fragment`;
10. refusal of ambiguous canonical-plus-legacy records, while keeping path,
    query, foreign-DID, and external service identities distinct;
11. VC DID-binding consumption of canonical absolute resolver identifiers while
    preserving the v1 fragment-only proof preimage and refusing path/query
    identifiers that would previously have collapsed onto the same fragment.

The portable gate also runs the pinned TypeScript domain, DID, and API unit
suites (717 tests) and consumes the same language-neutral resolver and
signature fixture from Rust and TypeScript. The Rust operation suite contains
explicit regressions for canonical writes, legacy reads, ambiguous records,
and path identities that must not collapse to a fragment. The repository
coverage gate reports 91.28% line coverage (90.05% region coverage) for this
qualified source graph.

## Live qualification boundary

The Rust `LiveBackend` is provider-injected. This repository does not ship a
concrete wallet funding/proving/node/indexer provider that can execute the
complete standalone lifecycle by itself. Consequently, recording-backend and
generated-contract tests must not be described as a live Rust lifecycle.

The original TypeScript suite was invoked on 2026-11-24. No scenario executed:
Docker failed while creating the isolated Compose network with `all predefined
address pools have been fully subnetted`; all 31 cases were skipped after the
suite-level setup failure. The existing shared stack was healthy, but the
upstream harness has no supported attach-to-existing-undeployed-stack mode.
No unrelated Docker networks were deleted to manufacture a pass.

Closing full live cross-language qualification therefore requires one of:

- a concrete Rust standalone provider implementation, followed by create →
  resolve → update → rotate/recover → deactivate on the same chain; and
- sufficient Docker network capacity (or an upstream external-stack mode) to
  execute the original TypeScript suite.

These deployment gaps do not invalidate the shared resolver/key/signature
vectors, but they do prevent claiming a complete live Rust/TypeScript E2E pass.
