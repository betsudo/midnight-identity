// This file is part of betsudo/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { metadataOnlyEdit, run } from "../../scripts/ci/target-plan.mjs";

test("only known body/title edits are eligible for proven source-run reuse", () => {
  for (const changes of [{ body: {} }, { title: {} }, { body: {}, title: {} }]) {
    assert.equal(metadataOnlyEdit("pull_request", { action: "edited", changes }), true);
  }
  for (const changes of [{}, { base: {} }, { title: {}, unknown: {} }]) {
    assert.equal(metadataOnlyEdit("pull_request", { action: "edited", changes }), false);
  }
  for (const action of ["synchronize", "reopened", "opened"]) {
    assert.equal(metadataOnlyEdit("pull_request", { action, changes: { body: {} } }), false);
  }
  assert.equal(metadataOnlyEdit("push", { action: "edited", changes: { body: {} } }), false);
});

test("event-backed immutable plans preserve source correctness and exact identities", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "issue117-plan-"));
  try {
    const eventFile = path.join(directory, "event.json");
    const base = "a".repeat(40);
    const head = "b".repeat(40);
    for (const [eventName, action, changes, policyOnly] of [
      ["pull_request", "edited", { body: {} }, false],
      ["pull_request", "edited", { title: {} }, false],
      ["pull_request", "edited", { base: {} }, false],
      ["pull_request", "edited", {}, false],
      ["pull_request", "edited", { unknown: {} }, false],
      ["pull_request", "synchronize", { body: {} }, false],
      ["pull_request", "reopened", { body: {} }, false],
      ["push", "edited", { body: {} }, false],
      ["schedule", undefined, undefined, false],
      ["workflow_dispatch", undefined, undefined, false],
    ]) {
      writeFileSync(eventFile, JSON.stringify({ action, changes }));
      let output = "";
      run(["--paths", "Cargo.toml", "--base", base, "--head", head,
        "--event-name", eventName, "--event-file", eventFile, "--full", "--format", "json"], {
        stdout: { write: (text) => { output += text; } },
      });
      const plan = JSON.parse(output);
      assert.equal(plan.base, base);
      assert.equal(plan.head, head);
      assert.equal(plan.mode, policyOnly ? "policy-only" : "full");
      if (policyOnly) assert.deepEqual(plan.targets, ["policy"]);
      else assert.ok(plan.targets.includes("rust"));
      assert.ok(plan.reason.includes(eventName));

      let lookups = 0;
      const proven = run(["--paths", "Cargo.toml", "--base", base, "--head", head,
        "--event-name", eventName, "--event-file", eventFile, "--full", "--format", "json",
        "--github-source-proof"], {
        stdout: { write() {} },
        sourceProof: () => { lookups++; return { runId: 99, artifactId: 5 }; },
      });
      const eligible = eventName === "pull_request" && action === "edited"
        && ("body" in (changes ?? {}) || "title" in (changes ?? {}));
      assert.equal(lookups, eligible ? 1 : 0);
      assert.equal(proven.validationClass, eligible ? "metadata-reuse" : "source");
      assert.equal(proven.mode, eligible ? "policy-only" : "full");
      const unavailable = run(["--paths", "Cargo.toml", "--event-name", eventName,
        "--event-file", eventFile, "--github-source-proof", "--format", "json"], {
        stdout: { write() {} }, sourceProof: () => { throw new Error("API unavailable"); },
      });
      assert.equal(unavailable.validationClass, "source");
      assert.ok(unavailable.targets.includes("rust"));
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("all expensive jobs require policy admission and use event identity", () => {
  const workflow = readFileSync(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  for (const lane of ["rust", "unit", "wasm", "coverage", "did_codegen", "vc_codegen", "audit"]) {
    const block = workflow.split(`\n  ${lane}:\n`)[1].split(/\n  [a-z_]+:\n/u)[0];
    assert.match(block, /needs: \[plan, policy\]/u);
    assert.doesNotMatch(block, /always\(\)/u);
  }
  assert.doesNotMatch(workflow, /MediaNoxLabs\/midnight-identity/u);
  assert.match(workflow, /EVENT_REPOSITORY: \$\{\{ github.repository \}\}/u);
  assert.match(workflow, /EVENT_REPOSITORY_URL: \$\{\{ github.server_url \}\}/u);
  assert.match(workflow, /Policy admission failed/u);
});
