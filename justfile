# Default target lists available recipes.
default:
    @just --list

# Re-generate Rust code from did.compact via the patched compact compiler.
#
# Per compactc --help: `compactc <flag> ... <source-pathname> <target-directory-pathname>`.
# With --rust + --skip-ts the compiler emits:
#   <target>/contract/lib.rs              -- Rust source (consumed)
#   <target>/Cargo.toml                   -- emitted Cargo manifest (NOT consumed; we maintain our own)
#   <target>/compiler/contract-info.json  -- analyzer metadata (not consumed in cycle 1)
#   <target>/zkir/<circuit>.zkir          -- ZKIR per exported circuit
#   <target>/keys/<circuit>.{prover,verifier}  -- proving + verifier keys (emitted by zkir tool)
# Source: third_party compact compiler/passes.ss (generate-everything pass).
did_revision := "750268664278c162e9171406983dc45f24bb5be7"

codegen:
    @mkdir -p target-gen
    @rm -rf target-gen/contract-out
    git submodule update --init third_party/midnight-did
    git -C third_party/midnight-did checkout --detach {{did_revision}}
    compactc-did --target rust --skip-zk \
        third_party/midnight-did/packages/contract/src/did.compact \
        target-gen/contract-out
    @mkdir -p crates/midnight-did-runtime/src/contract
    cp target-gen/contract-out/contract/lib.rs crates/midnight-did-runtime/src/contract/generated.rs
    @mkdir -p crates/midnight-did-runtime/assets/keys
    cp target-gen/contract-out/compiler/rust-compatibility.json crates/midnight-did-runtime/assets/rust-compatibility.json
    # Recursive find picks up keys whether they live in keys/, zkir/, or contract/.
    find target-gen/contract-out -name "*.zkir"     -exec cp {} crates/midnight-did-runtime/assets/keys/ \;
    find target-gen/contract-out -name "*.prover"   -exec cp {} crates/midnight-did-runtime/assets/keys/ \;
    find target-gen/contract-out -name "*.verifier" -exec cp {} crates/midnight-did-runtime/assets/keys/ \;
    cargo fmt -p midnight-did-runtime

# Verify re-running codegen produces no diff (regression signal for CI).
#
# Gate scope: `src/contract/generated.rs` AND the 12 `assets/keys/*.zkir`
# circuit artifacts — both are tracked and byte-reproducible under the
# flake-pinned compactc (verified 2026-08-10), so a change in circuit
# lowering shows up here rather than silently.
#
# NOT gated: `*.prover` / `*.verifier` proving keys. The devshell has no
# `zkir` tool ("Warning: ZKIR not found; skipping final circuit
# compilation"), so `just codegen` never emits them and there is nothing
# to compare. Wire them in only alongside a zkir-providing devshell.
codegen-check:
    #!/usr/bin/env bash
    set -euo pipefail
    before=$(mktemp -d "${TMPDIR:-/tmp}/midnight-did-codegen.XXXXXX")
    trap 'rm -rf "$before"' EXIT
    cp crates/midnight-did-runtime/src/contract/generated.rs "$before/generated.rs"
    cp crates/midnight-did-runtime/assets/rust-compatibility.json "$before/rust-compatibility.json"
    cp -R crates/midnight-did-runtime/assets/keys "$before/keys"
    just codegen
    diff -u "$before/generated.rs" crates/midnight-did-runtime/src/contract/generated.rs
    diff -u "$before/rust-compatibility.json" crates/midnight-did-runtime/assets/rust-compatibility.json
    diff -ru "$before/keys" crates/midnight-did-runtime/assets/keys

# Re-generate the VC Rust bindings from the pinned, released VC packages.
#
# Both inputs are sha256-pinned release tarballs, downloaded into target-gen/:
#
# - `@midnight-ntwrk/credential-compact` (npm) supplies the generic VC/VP core.
#   Its `credentials.compact` lands in midnight-vc-runtime as `credentials`.
# - `@midnight-ntwrk/midnight-vc-passport` supplies the Digital Passport family,
#   which lands in midnight-vc-families as `digital_passport` (npm).
#
# The family `include`s the core from `../core-compact-staging/`, so the recipe
# stages the same pinned core copy there before invoking the compiler.
vc_core_version := "1.0.0-rc1"
vc_core_sha256 := "3aeb66d81540f31d957bcaa54eda1f2ae2aa024c8451b381fbca22d0c2bb61fe"
vc_passport_version := "0.1.0-rc5"
vc_passport_sha256 := "bfb6e0de4c56108e06edd9b700672fc44ac9aa44d688f5fcf82a457fae4e177d"
vc_did_binding_version := "1.0.0-rc1"
vc_did_binding_sha256 := "abfd6ff02fd6284a1c9aa4a5d6a6ae48e957b493d8da042775222d6d0ac9df28"

codegen-vc:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p target-gen crates/midnight-vc-runtime/src/contract crates/midnight-vc-families/src/contract crates/midnight-vc-did-binding/src/contract

    # fetch_package <tarball> <url> <sha256> <extract dir>: download once,
    # verify the checksum on every run, then extract a fresh copy.
    fetch_package() {
        local tgz="$1" url="$2" sha="$3" dir="$4"
        if [ ! -f "$tgz" ]; then
            curl -sfL --output "$tgz" "$url"
        fi
        # `sha256sum -c` is shared by GNU coreutils and the Darwin-compatible
        # implementation in our devshell; GNU-only `--strict` breaks macOS CI.
        echo "$sha  $tgz" | sha256sum -c -
        rm -rf "$dir"
        mkdir -p "$dir"
        tar -xzf "$tgz" -C "$dir"
    }

    core_pkg="@midnight-ntwrk/credential-compact"
    core_ver="{{vc_core_version}}"
    core_dir="target-gen/${core_pkg##*/}-${core_ver}"
    fetch_package "$core_dir.tgz" \
        "https://registry.npmjs.org/${core_pkg}/-/${core_pkg##*/}-${core_ver}.tgz" \
        "{{vc_core_sha256}}" "$core_dir"
    core="$core_dir/package/dist"

    passport_pkg="@midnight-ntwrk/midnight-vc-passport"
    passport_ver="{{vc_passport_version}}"
    passport_dir="target-gen/${passport_pkg##*/}-${passport_ver}"
    fetch_package "$passport_dir.tgz" \
        "https://registry.npmjs.org/${passport_pkg}/-/${passport_pkg##*/}-${passport_ver}.tgz" \
        "{{vc_passport_sha256}}" "$passport_dir"
    passport="$passport_dir/package"

    did_binding_pkg="@midnight-ntwrk/credential-did-midnight"
    did_binding_ver="{{vc_did_binding_version}}"
    did_binding_dir="target-gen/${did_binding_pkg##*/}-${did_binding_ver}"
    fetch_package "$did_binding_dir.tgz" \
        "https://registry.npmjs.org/${did_binding_pkg}/-/${did_binding_pkg##*/}-${did_binding_ver}.tgz" \
        "{{vc_did_binding_sha256}}" "$did_binding_dir"
    did_binding="$did_binding_dir/package"

    staging="$passport/core-compact-staging"
    mkdir -p "$staging/credentials"
    cp "$core/credentials.compact" "$staging/credentials.compact"
    find "$core/credentials" -type f -name '*.compact' \
        -exec cp {} "$staging/credentials/" \;

    mkdir -p "$did_binding/dist/credentials"
    cp "$core/credentials.compact" "$did_binding/dist/credentials.compact"
    find "$core/credentials" -type f -name '*.compact' \
        -exec cp {} "$did_binding/dist/credentials/" \;

    # "<module>:<crate>:<entry point>" — module name is the Rust file under
    # the selected crate's src/contract/ directory.
    contracts=(
        "credentials:midnight-vc-runtime:$core/credentials.compact"
        "digital_passport:midnight-vc-families:$passport/src/digital-passport-credential.compact"
        "did_midnight:midnight-vc-did-binding:$did_binding/dist/did-midnight.compact"
    )
    for entry in "${contracts[@]}"; do
        module="${entry%%:*}"
        rest="${entry#*:}"
        crate="${rest%%:*}"
        entry_point="${rest#*:}"
        out="target-gen/vc-${module}-out"
        rm -rf "$out"
        # The Ledger 9 Rust AST backend is pinned by the `compact-did` flake
        # input. `--skip-zk` keeps this reproducibility gate focused on Rust
        # bindings; proof artifacts are qualified separately.
        compactc-did --target rust --skip-zk "$entry_point" "$out"
        {
            echo "// GENERATED — do not edit; run \`just codegen-vc\`."
            echo "//"
            if [ "$module" = "digital_passport" ]; then
                echo "// Family source: \`src/digital-passport-credential.compact\` in"
                echo "// \`@midnight-ntwrk/midnight-vc-passport@{{vc_passport_version}}\` (npm); core"
                echo "// contract \`@midnight-ntwrk/credential-compact@{{vc_core_version}}\` (npm),"
                echo "// staged into the package's \`core-compact-staging/\` by this recipe."
            elif [ "$module" = "did_midnight" ]; then
                echo "// DID binding source: \`dist/did-midnight.compact\` in"
                echo "// \`@midnight-ntwrk/credential-did-midnight@{{vc_did_binding_version}}\` (npm); core"
                echo "// contract \`@midnight-ntwrk/credential-compact@{{vc_core_version}}\` (npm)."
            else
                echo "// Source: \`dist/credentials.compact\` in"
                echo "// \`@midnight-ntwrk/credential-compact@{{vc_core_version}}\` (npm)."
            fi
            cat "$out/contract/lib.rs"
        } > "crates/${crate}/src/contract/${module}.rs"
    done
    cargo fmt -p midnight-vc-runtime -p midnight-vc-families -p midnight-vc-did-binding

# Verify re-running the VC codegen produces no diff (regression signal for CI).
codegen-vc-check: codegen-vc
    git diff --exit-code -- crates/midnight-vc-runtime/src/contract crates/midnight-vc-families/src/contract crates/midnight-vc-did-binding/src/contract

# Our first-party workspace crates. Bare `cargo fmt/clippy/nextest` also
# sweep the path-mounted third_party crates (cargo absorbs them as
# workspace members), and we don't gate vendored code — so every recipe
# scopes to this list, mirroring CI.
crate_flags := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-uniffi -p midnight-did-cli -p midnight-did-indexer -p midnight-did-resolver -p midnight-did-jubjub-schnorr -p midnight-passport-account-source -p midnight-passport-account -p midnight-passport-vault-source -p midnight-vc-did-binding -p midnight-vc-domain -p midnight-vc-families -p midnight-vc-proof -p midnight-vc-runtime"

# Family bindings are opt-in (`default = []`). Run a separate feature-complete
# pass so `--all-features` does not change unrelated workspace crates.
families_flags := "-p midnight-vc-families --all-features"

build:
    cargo build --all-targets {{crate_flags}}
    cargo build --all-targets {{families_flags}}

test:
    cargo nextest run {{crate_flags}}
    cargo nextest run {{families_flags}}

fmt:
    cargo fmt {{crate_flags}}
    taplo fmt

fmt-check:
    cargo fmt {{crate_flags}} -- --check
    taplo fmt --check

lint:
    cargo clippy --all-targets {{crate_flags}} -- -D warnings
    cargo clippy --all-targets {{families_flags}} -- -D warnings

# Line-coverage floor enforced by `coverage-gate` (and CI). Raise it as
# coverage improves; never lower it to admit a regression.
coverage_floor := "87"

# Coverage scope: every first-party crate; excludes the codegen artifact
# (gated by codegen-check, not tests) and service/demo bin entrypoints.
coverage_crates := "-p midnight-did-domain -p midnight-did-method -p midnight-did-api -p midnight-did -p midnight-did-runtime -p midnight-did-indexer -p midnight-did-jubjub-schnorr -p midnight-did-resolver -p midnight-did-uniffi -p midnight-did-cli -p midnight-passport-account-source -p midnight-passport-account -p midnight-passport-vault-source -p midnight-vc-did-binding -p midnight-vc-domain -p midnight-vc-families -p midnight-vc-proof -p midnight-vc-runtime"
coverage_features := "--features midnight-vc-families/digital-passport"
# `contract/generated\.rs` is the DID codegen artifact;
# `contract/{credentials,digital_passport,did_midnight}\.rs` are the VC ones.
# Generated code is gated by `codegen-check` / `codegen-vc-check`, not by tests;
# every hand-written line in those crates stays in scope.
coverage_exclude := 'contract/generated\.rs|contract/credentials\.rs|contract/digital_passport\.rs|contract/did_midnight\.rs|src/main\.rs|src/bin/'

# HTML coverage report for humans.
coverage:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        {{coverage_features}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --html --open

# Same scope, but fails if line coverage drops below the floor. CI gate.
coverage-gate:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        {{coverage_features}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --summary-only --fail-under-lines {{coverage_floor}}

# LCOV export for CI artifact upload / external services.
coverage-lcov:
    cargo llvm-cov --locked \
        {{coverage_crates}} \
        {{coverage_features}} \
        --ignore-filename-regex '{{coverage_exclude}}' \
        --lcov --output-path target/lcov.info

# Ratchet nag: fail-free warning when measured coverage exceeds the floor
# by >5 points — time to raise `coverage_floor` in the same PR.
coverage-ratchet:
    #!/usr/bin/env bash
    set -euo pipefail
    measured=$(cargo llvm-cov report --summary-only --ignore-filename-regex '{{coverage_exclude}}|third_party/' 2>/dev/null | awk '/^TOTAL/ {print $(NF-3)}' | tr -d '%')
    floor={{coverage_floor}}
    headroom=$(echo "$measured $floor" | awk '{printf "%.2f", $1 - $2}')
    echo "coverage: measured=${measured}% floor=${floor}% headroom=${headroom}"
    if (( $(echo "$headroom > 5" | bc -l) )); then
        echo "::warning::coverage floor is stale (headroom ${headroom} > 5) — raise coverage_floor in justfile"
    fi

ci: fmt-check lint build test coverage-gate coverage-ratchet
