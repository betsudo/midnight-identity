// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const ledgerRevision = "8655615e7c4cbf3a1187b3203bf72e69dc09b8dc";
const compactRevision = "32e314770da10cc62dab30dcfeb340ec4c6bcb64";
const didCompactRevision = "7a4fab772ee34c19053ce293d08052b501ce4b50";
const ledger9Revision = "a2b30128ca4b660b73a068e29b56b48ede8c9a74";
const proofsRevision = "532629b044a88473a7175f4a96c2511c91156136";
const ledgerVersions = new Map([
  ["midnight-base-crypto", "1.0.1"],
  ["midnight-coin-structure", "2.0.2"],
  ["midnight-onchain-runtime", "3.1.1"],
  ["midnight-onchain-state", "3.0.1"],
  ["midnight-onchain-vm", "3.1.1"],
  ["midnight-serialize", "1.1.1"],
  ["midnight-storage", "2.0.3"],
  ["midnight-transient-crypto", "2.1.1"],
  ["midnight-zkir", "2.1.1"],
  ["midnight-zswap", "8.2.0-rc.1"],
  ["midnight-ledger", "8.2.0-rc.1"],
]);
const ledgerCrates = [...ledgerVersions.keys()];

function escaped(value) {
  return value.replaceAll(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function exactGitDependency(name, repository, revision) {
  return new RegExp(`^${escaped(name)}\\s*=\\s*\\{[^}]*git\\s*=\\s*"${escaped(repository)}"[^}]*rev\\s*=\\s*"${revision}"[^}]*\\}`, "mu");
}

test("published workspace dependencies use one immutable Ledger and Compact graph", async () => {
  const manifest = await readFile(path.join(root, "Cargo.toml"), "utf8");
  const ledgerRepository = "https://github.com/MediaNoxLabs/midnight-ledger.git";
  for (const crate of ledgerCrates) {
    const matches = manifest.match(new RegExp(exactGitDependency(crate, ledgerRepository, ledgerRevision), "gmu")) ?? [];
    assert.equal(matches.length, 1, `${crate} must be pinned once in workspace.dependencies`);
  }
  for (const alias of ["compact-runtime", "midnight-compact-runtime"]) {
    assert.match(
      manifest,
      exactGitDependency(alias, "https://github.com/MediaNoxLabs/compact.git", compactRevision),
    );
  }
  assert.match(
    manifest,
    exactGitDependency("midnight-proofs", "https://github.com/MediaNoxLabs/midnight-zk.git", proofsRevision),
  );
  assert.doesNotMatch(manifest, /path\s*=\s*"third_party\/(?:midnight-ledger|compact)/u);
});

test("the obsolete downstream Ledger source-lock shim stays removed", async () => {
  const [rootManifest, runtimeManifest] = await Promise.all([
    readFile(path.join(root, "Cargo.toml"), "utf8"),
    readFile(path.join(root, "crates", "midnight-did-runtime", "Cargo.toml"), "utf8"),
  ]);
  assert.doesNotMatch(rootManifest, /midnight-ledger-source-lock/u);
  assert.doesNotMatch(runtimeManifest, /midnight-ledger-source-lock/u);
});

test("Nix pins the same immutable sources and exposes only opt-in Cargo overrides", async () => {
  const [flake, rustTools, wrapper] = await Promise.all([
    readFile(path.join(root, "flake.nix"), "utf8"),
    readFile(path.join(root, "nix", "rustTools.nix"), "utf8"),
    readFile(path.join(root, "scripts", "cargo-with-nix-overrides.sh"), "utf8"),
  ]);
  assert.match(flake, new RegExp(`rev\\s*=\\s*"${ledgerRevision}"`, "u"));
  assert.match(
    flake,
    new RegExp(`compact-runtime\\s*=\\s*\\{[\\s\\S]*?rev\\s*=\\s*"${compactRevision}"`, "u"),
  );
  assert.match(
    flake,
    new RegExp(`midnight-ledger-v9\\s*=\\s*\\{[\\s\\S]*?rev\\s*=\\s*"${ledger9Revision}"`, "u"),
  );
  for (const input of ["compact-runtime-did", "compact-did"]) {
    assert.match(
      flake,
      new RegExp(`${input}\\s*=\\s*\\{[\\s\\S]*?rev\\s*=\\s*"${didCompactRevision}"`, "u"),
    );
  }
  assert.match(flake, new RegExp(`rev\\s*=\\s*"${proofsRevision}"`, "u"));
  assert.match(rustTools, /"aarch64-apple-ios-sim"/u);
  assert.match(wrapper, /third_party\/compact-did/u);
  assert.match(wrapper, /paths=\[/u);
  assert.doesNotMatch(wrapper, /patch\.crates-io/u);
  assert.doesNotMatch(wrapper, /third_party\/midnight-ledger/u);
});

test("clean external-consumer smoke is wired into CI and documents its supported cone", async () => {
  const [script, workflow, documentation] = await Promise.all([
    readFile(path.join(root, "scripts", "ci", "git-consumer-smoke.sh"), "utf8"),
    readFile(path.join(root, ".github", "workflows", "ci.yml"), "utf8"),
    readFile(path.join(root, "crates", "midnight-did-runtime", "README.md"), "utf8"),
  ]);
  await access(path.join(root, "scripts", "cargo-with-nix-overrides.sh"));
  assert.match(script, /mktemp/u);
  assert.match(script, /cargo metadata/u);
  assert.match(script, /cargo check/u);
  assert.match(script, /midnight-did-runtime/u);
  assert.match(script, /features\s*=\s*\[\s*"ledger-transaction"\s*\]/u);
  assert.match(script, new RegExp(didCompactRevision, "u"));
  for (const [name, version] of [
    ["midnight-ledger-v9", "1.0.1"],
    ["midnight-proofs", "0.8.2"],
    ["midnight-storage", "3.0.0"],
    ["midnight-zk-stdlib", "2.4.0"],
  ]) {
    assert.match(script, new RegExp(`${escaped(name)}\\s+${escaped(version)}`, "u"));
  }
  assert.match(workflow, /bash scripts\/ci\/git-consumer-smoke\.sh/u);
  assert.match(workflow, /--target aarch64-apple-ios-sim/u);
  assert.match(workflow, /xcrun --sdk iphonesimulator --show-sdk-path/u);
  for (const phrase of [
    "Immutable Git consumption",
    "default feature set is empty",
    "`ledger-transaction`",
    "Git resolution is the portable default",
    didCompactRevision,
    "cargo-with-nix-overrides.sh",
  ]) assert.ok(documentation.includes(phrase), phrase);
});
