#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ts_root="$repo_root/third_party/midnight-did"
expected_ts_revision="750268664278c162e9171406983dc45f24bb5be7"
pnpm_cmd=(corepack pnpm@10.34.5)
run_rust=true
run_live=false

for argument in "$@"; do
  case "$argument" in
    --cross-language-only) run_rust=false ;;
    --live) run_live=true ;;
    *)
      echo "usage: $0 [--cross-language-only] [--live]" >&2
      exit 2
      ;;
  esac
done

git -C "$repo_root" submodule update --init third_party/midnight-did
actual_ts_revision="$(git -C "$ts_root" rev-parse HEAD)"
if [[ "$actual_ts_revision" != "$expected_ts_revision" ]]; then
  echo "midnight-did revision mismatch: expected $expected_ts_revision, got $actual_ts_revision" >&2
  exit 1
fi

"${pnpm_cmd[@]}" --dir "$ts_root" install --frozen-lockfile
"${pnpm_cmd[@]}" --dir "$ts_root" --filter @midnight-ntwrk/midnight-did-jubjub-schnorr build
"${pnpm_cmd[@]}" --dir "$ts_root" --filter @midnight-ntwrk/midnight-did-contract build:prepared
"${pnpm_cmd[@]}" --dir "$ts_root" --filter @midnight-ntwrk/midnight-did build
node "$repo_root/scripts/ci/did-cross-language-conformance.mjs"

if [[ "$run_rust" == true ]]; then
  cargo +1.99.1 test --manifest-path "$repo_root/Cargo.toml" -p midnight-did-domain -p midnight-did-api -p midnight-did-jubjub-schnorr --locked
fi

if [[ "$run_live" == true ]]; then
  "${pnpm_cmd[@]}" --dir "$ts_root" --filter @midnight-ntwrk/midnight-did-api test-api
fi
