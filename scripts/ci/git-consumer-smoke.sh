#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

compact_revision=05c9ed69b891baa4f15777403c9bdc8bc86c150c
ledger_revision=f82ac287442fc15c4eb79f02dcb6510373979eab
zk_revision=0f9d5c3879f92a8beae8bdb9e7305f39e03a3227
zkir_revision=35e386b7b3f3a76117423ca72883c852181104dc
repository_url=https://github.com/MediaNoxLabs/midnight-identity.git
revision=

while (( $# > 0 )); do
  case "$1" in
    --repo-url)
      repository_url=${2:?--repo-url requires a value}
      shift 2
      ;;
    --rev)
      revision=${2:?--rev requires a value}
      shift 2
      ;;
    -h|--help)
      echo "usage: $0 --rev <40-hex-sha> [--repo-url <git-url>]"
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! "$revision" =~ ^[0-9a-f]{40}$ ]]; then
  echo "--rev must be one immutable 40-character lowercase Git revision" >&2
  exit 2
fi
if [[ "$repository_url" == *'"'* || "$repository_url" == *$'\n'* ]]; then
  echo "--repo-url contains characters that cannot be represented safely in Cargo.toml" >&2
  exit 2
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/midnight-did-git-consumer.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/src"
cat > "$scratch/Cargo.toml" <<EOF
[package]
name = "midnight-did-git-consumer"
version = "0.0.0"
edition = "2024"
rust-version = "1.99"
publish = false

[dependencies]
midnight-did-runtime = { git = "$repository_url", rev = "$revision", features = ["ledger-transaction"] }

# The current published blake2b_halo2/sha3-circuit helpers request the proof
# cohort from crates.io. Cargo patches are scoped to the consumer root, so an
# application must carry these exact pins until those helpers publish the
# coordinated Ledger 9 source graph.
[patch.crates-io]
midnight-circuits-ledger9 = { package = "midnight-circuits", version = "=7.3.0", git = "https://github.com/betsudo/midnight-zk", rev = "$zk_revision" }
midnight-curves-ledger9 = { package = "midnight-curves", version = "=0.3.1", git = "https://github.com/betsudo/midnight-zk", rev = "$zk_revision" }
midnight-proofs-ledger9 = { package = "midnight-proofs", version = "=0.8.2", git = "https://github.com/betsudo/midnight-zk", rev = "$zk_revision" }
midnight-zk-stdlib-ledger9 = { package = "midnight-zk-stdlib", version = "=2.4.0", git = "https://github.com/betsudo/midnight-zk", rev = "$zk_revision" }
EOF
printf 'fn main() {}\n' > "$scratch/src/main.rs"

metadata="$scratch/metadata.json"
consumer_target=${GIT_CONSUMER_TARGET_DIR:-$scratch/target}
CARGO_TARGET_DIR="$consumer_target" cargo metadata \
  --format-version 1 \
  --manifest-path "$scratch/Cargo.toml" > "$metadata"

assert_single_source() {
  local package=$1 repository=$2 expected_revision=$3
  local total matching
  total=$(jq --arg package "$package" '[.packages[] | select(.name == $package)] | length' "$metadata")
  matching=$(jq \
    --arg package "$package" \
    --arg repository "$repository" \
    --arg revision "$expected_revision" \
    '[.packages[] | select(.name == $package and (.source // "" | contains($repository)) and (.source // "" | contains($revision)))] | length' \
    "$metadata")
  if [[ "$total" != 1 || "$matching" != 1 ]]; then
    echo "$package did not resolve exactly once from $repository@$expected_revision" >&2
    jq --arg package "$package" '.packages[] | select(.name == $package) | {name, version, source}' "$metadata" >&2
    exit 1
  fi
}

assert_single_source midnight-compact-runtime betsudo/compact "$compact_revision"

assert_version() {
  local package=$1 expected_version=$2
  local matching
  matching=$(jq \
    --arg package "$package" \
    --arg version "$expected_version" \
    '[.packages[] | select(.name == $package and .version == $version)] | length' \
    "$metadata")
  if [[ "$matching" != 1 ]]; then
    echo "$package did not resolve exactly once at $expected_version" >&2
    jq --arg package "$package" '.packages[] | select(.name == $package) | {name, version, source}' "$metadata" >&2
    exit 1
  fi
}

assert_version_source() {
  local package=$1 expected_version=$2 repository=$3 expected_revision=$4
  local matching
  matching=$(jq \
    --arg package "$package" \
    --arg version "$expected_version" \
    --arg repository "$repository" \
    --arg revision "$expected_revision" \
    '[.packages[] | select(.name == $package and .version == $version and (.source // "" | contains($repository)) and (.source // "" | contains($revision)))] | length' \
    "$metadata")
  if [[ "$matching" != 1 ]]; then
    echo "$package@$expected_version did not resolve exactly once from $repository@$expected_revision" >&2
    jq --arg package "$package" '.packages[] | select(.name == $package) | {name, version, source}' "$metadata" >&2
    exit 1
  fi
}

while read -r package version; do
  assert_version "$package" "$version"
done <<'VERSIONS'
midnight-base-crypto 1.1.1
midnight-circuits 7.3.0
midnight-coin-structure 3.0.1
midnight-curves 0.3.1
midnight-ledger-v9 1.0.2
midnight-onchain-runtime 4.0.1
midnight-onchain-state 4.0.1
midnight-onchain-vm 4.0.1
midnight-proofs 0.8.2
midnight-serialize 1.2.1
midnight-storage 3.1.0
midnight-storage-core 1.2.3
midnight-transient-crypto 2.2.0
midnight-transient-crypto 3.1.0
midnight-zk-stdlib 2.4.0
midnight-zkir 2.2.0
midnight-zswap 9.0.1
VERSIONS

for package in \
  midnight-base-crypto \
  midnight-coin-structure \
  midnight-ledger-v9 \
  midnight-onchain-runtime \
  midnight-onchain-state \
  midnight-onchain-vm \
  midnight-serialize \
  midnight-storage \
  midnight-storage-core \
  midnight-zswap
do
  assert_single_source "$package" betsudo/midnight-ledger "$ledger_revision"
done

# Ledger 9 intentionally retains the v2 transient/ZKIR pair as a private
# implementation boundary while the active runtime uses transient-crypto
# 3.1.0. Both versions must still come from the one Ledger revision.
assert_version_source midnight-transient-crypto 2.2.0 betsudo/midnight-ledger "$ledger_revision"
assert_version_source midnight-transient-crypto 3.1.0 betsudo/midnight-ledger "$ledger_revision"
assert_version_source midnight-zkir 2.2.0 betsudo/midnight-ledger "$ledger_revision"

while read -r package version; do
  # Ledger retains an older private proof cohort for its v2 implementation
  # boundary. Assert the active Ledger 9 versions rather than rejecting those
  # intentional legacy package versions by name alone.
  assert_version_source "$package" "$version" betsudo/midnight-zk "$zk_revision"
done <<'ZK_VERSIONS'
midnight-circuits 7.3.0
midnight-curves 0.3.1
midnight-proofs 0.8.2
midnight-zk-stdlib 2.4.0
ZK_VERSIONS

if jq -e --arg revision "$zkir_revision" \
  '.packages[] | select(.name == "midnight-zkir" and ((.source // "") | contains($revision)))' \
  "$metadata" >/dev/null; then
  echo "runtime consumer unexpectedly includes the compiler-only ZKIR parser" >&2
  exit 1
fi

# `midnight-zkir=3.2.1` is an exact development dependency used to parse the
# generated compiler artifacts. It is intentionally absent from this runtime
# consumer cone; Ledger 9 may retain an older ZKIR version as a private
# implementation dependency.

if jq -e '.packages[] | select(.name == "sysinfo")' "$metadata" >/dev/null; then
  echo "Ledger 9 consumer graph unexpectedly contains sysinfo" >&2
  exit 1
fi

if jq -e '.packages[] | select(.name == "midnight-did-uniffi")' "$metadata" >/dev/null; then
  echo "the supported Git consumer cone must not expose midnight-did-uniffi" >&2
  exit 1
fi

CARGO_TARGET_DIR="$consumer_target" cargo check \
  --manifest-path "$scratch/Cargo.toml"
