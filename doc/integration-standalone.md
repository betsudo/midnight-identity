<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Ledger 9 local integration

The repository can attach to the designated shared `midnight-network` stack
or, when this checkout is the designated owner, operate the same stack through
its repo-owned Compose file. Do not start a second fixed-port project while a
shared owner is active.

## Attach to the shared stack

Run the read-only preflight:

```bash
scripts/midnight-localnet.sh discover
```

It verifies:

- node release `2.1.101`, genesis availability, and block lookup;
- Indexer v4 Ledger 9 protocol state and agreement with the node at the same
  height;
- proof-server release `9.0.2`.

`discover` and `attach` are aliases. On success they print the canonical client
configuration:

| Service | Endpoint |
| --- | --- |
| Node RPC | `http://127.0.0.1:9944` |
| Indexer HTTP | `http://127.0.0.1:8088/api/v4/graphql` |
| Indexer WebSocket | `ws://127.0.0.1:8088/api/v4/graphql/ws` |
| Proof server | `http://127.0.0.1:6300` |

Attach failure is diagnostic only: it does not authorize another session to
start, stop, reset, or replace the shared stack. Coordinate with its owner.
Even the owner-only `up` command performs this discovery first and exits
without invoking Compose when the compatible shared stack is already running.
It refuses to start over incompatible services occupying the shared endpoints.

## Operate as the designated owner

Create private indexer credentials once:

```bash
cp infra/standalone.env.example infra/.midnight-indexer.env
# Replace every value. Generate APP__INFRA__SECRET with: openssl rand -hex 32
```

Keep the same credentials when reusing the Compose-managed indexer volume.
The private file is ignored by Git.

Validate and start the pinned Apple Silicon Ledger 9 cohort:

```bash
scripts/midnight-localnet.sh config
MIDNIGHT_LOCALNET_OWNER=1 scripts/midnight-localnet.sh up
scripts/midnight-localnet.sh status
scripts/midnight-localnet.sh attach
```

Only the designated owner shuts it down. `down` preserves the named volumes:

```bash
MIDNIGHT_LOCALNET_OWNER=1 scripts/midnight-localnet.sh down
```

## Run the Rust service probe

After a successful attach, the environment-gated runtime probe checks the
same node, Indexer v4, and proof-server surfaces:

```bash
MIDNIGHT_DID_LEDGER9_STANDALONE=1 \
MIDNIGHT_DID_LEDGER9_NODE_URL=http://127.0.0.1:9944 \
MIDNIGHT_DID_LEDGER9_INDEXER_URL=http://127.0.0.1:8088 \
MIDNIGHT_DID_LEDGER9_PROOF_URL=http://127.0.0.1:6300 \
cargo test -p midnight-did-runtime --test ledger9_standalone -- --nocapture
```

This is a compatibility probe, not a funded DID lifecycle receipt. A lifecycle
claim additionally requires a fresh deployment, proof, transaction submission,
finality, and independent indexed resolution. When reusing a deployment record
after restart, verify its recorded block hash at that height; genesis alone does
not distinguish independent dev chains.

## Isolated runs

Tests that require a fresh chain, reset, incompatible versions, or a different
topology must use a separate Compose project named
`midnight-midnight-identity-ledger9-<run-id>`, Compose-managed volumes, and
Docker-assigned loopback ports. Discover every mapped endpoint with
`docker compose ... port`; do not point the shared client configuration at an
isolated run. Remove only resources owned by that run.
