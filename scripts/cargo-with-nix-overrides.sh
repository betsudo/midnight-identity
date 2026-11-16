#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

root=$(git rev-parse --show-toplevel)
if [[ "$root" == *"'"* ]]; then
  echo "repository path cannot contain a single quote: $root" >&2
  exit 2
fi

compact_root="$root/third_party/compact-did"

for required in "$compact_root/runtime-rs/Cargo.toml" "$compact_root/runtime-rs-macros/Cargo.toml"; do
  if [[ ! -f "$required" ]]; then
    echo "missing Nix-materialised source: $required" >&2
    echo "enter 'nix develop .#rust' before using this optional wrapper" >&2
    exit 2
  fi
done

cargo_config=(--config "paths=['$compact_root/runtime-rs']")

exec cargo "${cargo_config[@]}" "$@"
