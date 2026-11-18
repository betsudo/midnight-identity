// This file is part of betsudo/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const LIMIT = 1024 * 1024;
const WORKFLOW = ".github/workflows/ci.yml";
const PLAN_KEYS = ["schemaVersion", "deliveryProfile", "mode", "diffAvailable",
  "changedPaths", "areas", "packages", "targets", "audit", "base", "head"];

export function compatibleSourcePlan(prior, ordinary) {
  return prior?.validationClass === "source" && ordinary.diffAvailable === true
    && ["affected", "full"].includes(prior.mode)
    && PLAN_KEYS.every((key) => JSON.stringify(prior[key]) === JSON.stringify(ordinary[key]));
}

function api(endpoint, binary = false) {
  const result = execFileSync("gh", ["api", endpoint], {
    encoding: binary ? undefined : "utf8", maxBuffer: LIMIT, timeout: 10000,
    stdio: ["ignore", "pipe", "ignore"],
  });
  return binary ? result : JSON.parse(result);
}

function artifactPlan(endpoint, request) {
  const archive = request(endpoint, true);
  if (!Buffer.isBuffer(archive) || archive.length > LIMIT) throw new Error("invalid archive");
  const directory = mkdtempSync(path.join(tmpdir(), "factory-source-proof-"));
  try {
    const file = path.join(directory, "plan.zip");
    writeFileSync(file, archive, { mode: 0o600 });
    const options = { encoding: "utf8", maxBuffer: LIMIT, timeout: 10000, stdio: ["ignore", "pipe", "ignore"] };
    if (execFileSync("unzip", ["-Z1", file], options).trim() !== "target-plan.json") {
      throw new Error("unexpected archive members");
    }
    return JSON.parse(execFileSync("unzip", ["-p", file, "target-plan.json"], options));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

// Authenticated Actions API and run-owned artifacts are evidence; comments and
// labels are never consulted. Any unavailable or malformed evidence costs only
// an optimization: the caller retains the ordinary source-validation plan.
export function findSourceValidation({ repository, runId, pr, ordinary }, {
  request = api, readPlan = artifactPlan,
} = {}) {
  try {
    if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/u.test(repository ?? "")
        || !/^[1-9]\d*$/u.test(String(runId)) || !Number.isSafeInteger(pr) || pr < 1
        || !/^[0-9a-f]{40}$/u.test(ordinary.head ?? "")
        || !/^[0-9a-f]{40}$/u.test(ordinary.base ?? "")) return null;
    const root = `repos/${repository}/actions`;
    const current = request(`${root}/runs/${runId}`);
    if (current.id !== Number(runId) || current.path !== WORKFLOW
        || current.repository?.full_name !== repository
        || !Number.isSafeInteger(current.repository.id)
        || !Number.isSafeInteger(current.workflow_id)) return null;
    const runs = request(`${root}/workflows/${current.workflow_id}/runs?event=pull_request&status=completed&head_sha=${ordinary.head}&per_page=30`).workflow_runs;
    if (!Array.isArray(runs)) return null;
    for (const run of runs.slice(0, 5)) {
      if (!Number.isSafeInteger(run.id) || run.id === Number(runId)
          || run.workflow_id !== current.workflow_id || run.path !== WORKFLOW
          || run.repository?.id !== current.repository.id || run.repository?.full_name !== repository
          || run.event !== "pull_request" || run.head_sha !== ordinary.head
          || run.status !== "completed" || run.conclusion !== "success"
          || !Array.isArray(run.pull_requests) || run.pull_requests.length !== 1
          || run.pull_requests[0].number !== pr
          || run.pull_requests[0].head?.sha !== ordinary.head
          || run.pull_requests[0].base?.sha !== ordinary.base) continue;
      const artifacts = request(`${root}/runs/${run.id}/artifacts?per_page=100`).artifacts;
      if (!Array.isArray(artifacts)) return null;
      const matches = artifacts.filter((artifact) => artifact.name === "target-plan" && artifact.expired === false);
      if (matches.length !== 1) continue;
      const artifact = matches[0];
      if (!Number.isSafeInteger(artifact.id) || artifact.id < 1
          || !Number.isSafeInteger(artifact.size_in_bytes) || artifact.size_in_bytes < 1 || artifact.size_in_bytes > LIMIT
          || artifact.workflow_run?.id !== run.id
          || artifact.workflow_run?.repository_id !== current.repository.id
          || artifact.workflow_run?.head_sha !== ordinary.head) continue;
      const prior = readPlan(`${root}/artifacts/${artifact.id}/zip`, request);
      if (compatibleSourcePlan(prior, ordinary)) return { runId: run.id, artifactId: artifact.id };
    }
  } catch { /* Fail closed to ordinary source validation; do not print API/token output. */ }
  return null;
}
