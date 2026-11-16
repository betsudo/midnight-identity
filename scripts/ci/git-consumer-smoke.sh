#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

compact_revision=7a4fab772ee34c19053ce293d08052b501ce4b50
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

assert_single_source midnight-compact-runtime MediaNoxLabs/compact "$compact_revision"

assert_registry_version() {
  local package=$1 expected_version=$2
  local matching
  matching=$(jq \
    --arg package "$package" \
    --arg version "$expected_version" \
    '[.packages[] | select(.name == $package and .version == $version and (.source // "" | startswith("registry+")))] | length' \
    "$metadata")
  if [[ "$matching" != 1 ]]; then
    echo "$package did not resolve exactly once from the registry at $expected_version" >&2
    jq --arg package "$package" '.packages[] | select(.name == $package) | {name, version, source}' "$metadata" >&2
    exit 1
  fi
}

while read -r package version; do
  assert_registry_version "$package" "$version"
done <<'VERSIONS'
midnight-base-crypto 1.1.1
midnight-coin-structure 3.0.1
midnight-ledger-v9 1.0.1
midnight-onchain-runtime 4.0.1
midnight-onchain-state 4.0.1
midnight-onchain-vm 4.0.1
midnight-proofs 0.8.2
midnight-serialize 1.2.1
midnight-storage 3.0.0
midnight-transient-crypto 3.0.1
midnight-zk-stdlib 2.4.0
midnight-zswap 9.0.1
VERSIONS

# `midnight-zkir=3.2.1` is an exact development dependency used to parse the
# generated compiler artifacts. It is intentionally absent from this runtime
# consumer cone; Ledger 9 may retain an older ZKIR version as a private
# implementation dependency.

if jq -e '.packages[] | select((.source // "") | contains("midnight-ledger.git") or contains("midnight-zk.git"))' "$metadata" >/dev/null; then
  echo "Ledger 9 consumer graph unexpectedly contains a Git-sourced ledger/proof package" >&2
  exit 1
fi

if jq -e '.packages[] | select(.name == "midnight-did-uniffi")' "$metadata" >/dev/null; then
  echo "the supported Git consumer cone must not expose midnight-did-uniffi" >&2
  exit 1
fi

CARGO_TARGET_DIR="$consumer_target" cargo check \
  --manifest-path "$scratch/Cargo.toml"
