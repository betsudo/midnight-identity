// This file is part of betsudo/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
import assert from "node:assert/strict";
import test from "node:test";
import { makeTargetPlan } from "../../scripts/ci/target-plan.mjs";
import { findSourceValidation } from "../../scripts/ci/source-validation-proof.mjs";

function fixture() {
  const ordinary = { ...makeTargetPlan(["crates/midnight-did-domain/src/lib.rs"]),
    base: "a".repeat(40), head: "b".repeat(40), validationClass: "source" };
  const repository = { id: 7, full_name: "betsudo/midnight-identity" };
  const current = { id: 100, workflow_id: 42, path: ".github/workflows/ci.yml", repository };
  const prior = { ...current, id: 99, event: "pull_request", head_sha: ordinary.head,
    status: "completed", conclusion: "success",
    pull_requests: [{ number: 117, head: { sha: ordinary.head }, base: { sha: ordinary.base } }] };
  const artifact = { id: 5, name: "target-plan", expired: false, size_in_bytes: 1024,
    workflow_run: { id: 99, repository_id: 7, head_sha: ordinary.head } };
  return { ordinary, current, prior, artifact, plan: structuredClone(ordinary), runs: null };
}

function proof(state, options = {}) {
  const root = "repos/betsudo/midnight-identity/actions";
  const request = (endpoint) => {
    if (endpoint === `${root}/runs/100`) return state.current;
    if (endpoint === `${root}/workflows/42/runs?event=pull_request&status=completed&head_sha=${state.ordinary.head}&per_page=30`) {
      return { workflow_runs: state.runs ?? [state.prior] };
    }
    if (endpoint === `${root}/runs/99/artifacts?per_page=100`) return { artifacts: [state.artifact] };
    if (endpoint === `${root}/artifacts/5/zip`) return Buffer.from("malformed ZIP");
    throw new Error("unexpected API request");
  };
  return findSourceValidation({ repository: "betsudo/midnight-identity", runId: "100", pr: 117,
    ordinary: state.ordinary }, { request, readPlan: (endpoint) => {
      assert.equal(endpoint, `${root}/artifacts/5/zip`);
      return state.plan;
    }, ...options });
}

test("same PR/head/base successful source run and compatible immutable plan permit reuse", () => {
  assert.deepEqual(proof(fixture()), { runId: 99, artifactId: 5 });
});

test("missing, failed, canceled, mismatched and recursively reused evidence fail closed", () => {
  for (const mutate of [
    (s) => { s.runs = []; },
    (s) => { s.prior.conclusion = "failure"; },
    (s) => { s.prior.conclusion = "cancelled"; },
    (s) => { s.prior.status = "in_progress"; },
    (s) => { s.prior.head_sha = "c".repeat(40); },
    (s) => { s.prior.id = 100; },
    (s) => { s.prior.repository = { id: 8, full_name: "other/repo" }; },
    (s) => { s.prior.workflow_id = 43; },
    (s) => { s.prior.path = ".github/workflows/other.yml"; },
    (s) => { s.prior.pull_requests[0].number = 118; },
    (s) => { s.prior.pull_requests[0].base.sha = "c".repeat(40); },
    (s) => { s.prior.pull_requests[0].head.sha = "c".repeat(40); },
    (s) => { s.plan.base = "c".repeat(40); },
    (s) => { s.plan.targets = ["policy"]; },
    (s) => { s.plan.changedPaths = []; },
    (s) => { s.plan.validationClass = "metadata-reuse"; },
    (s) => { s.plan.mode = "policy-only"; },
    (s) => { delete s.plan.validationClass; },
    (s) => { s.artifact.expired = true; },
    (s) => { s.artifact.workflow_run.id = 98; },
    (s) => { s.artifact.size_in_bytes = 2 ** 30; },
    (s) => { delete s.artifact.size_in_bytes; },
    (s) => { s.current.repository.full_name = "other/repo"; },
    (s) => { s.plan = null; },
  ]) {
    const state = fixture();
    mutate(state);
    assert.equal(proof(state), null, String(mutate));
  }
});

test("API and malformed artifact failures preserve ordinary validation", () => {
  const state = fixture();
  assert.equal(proof(state, { readPlan: () => { throw new Error("malformed artifact"); } }), null);
  // Exercise the real bounded archive reader with an invalid API response.
  assert.equal(proof(state, { readPlan: undefined }), null);
  for (const request of [() => { throw new Error("unavailable"); }, () => null, () => ({})]) {
    assert.equal(findSourceValidation({ repository: "betsudo/midnight-identity", runId: "100", pr: 117,
      ordinary: state.ordinary }, { request }), null);
  }
});
