import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { canDecideApproval } from "./approvalAction.ts";
import { WorkflowApprovalCard } from "./WorkflowApprovalCard.tsx";

const scope = { relayUrl: "ws://127.0.0.1:63203", pubkey: "ab".repeat(32) };
const run = {
  id: "run",
  workflowId: "wf",
  currentStep: 1,
  status: "waiting_approval",
};
const approval = {
  runId: "run",
  workflowId: "wf",
  stepId: "gate",
  stepIndex: 1,
  status: "pending",
  approvalRef: "cd".repeat(32),
  approverSpec: scope.pubkey,
  expiresAt: "2099-01-01T00:00:00Z",
};

test("only the exact active pending gate and eligible captured signer can be reviewed", () => {
  const check = (r = run, a = approval, s = scope) =>
    canDecideApproval(r, a, s, Date.now());
  assert.equal(check(), true);
  assert.equal(
    check(run, { ...approval, approverSpec: scope.pubkey.toUpperCase() }),
    true,
  );
  for (const spec of ["any", ""])
    assert.equal(check(run, { ...approval, approverSpec: spec }), true);
  for (const status of [
    "running",
    "pending",
    "completed",
    "failed",
    "cancelled",
  ])
    assert.equal(check({ ...run, status }), false);
  for (const patch of [
    { runId: "foreign" },
    { workflowId: "foreign" },
    { stepIndex: 0 },
    { status: "granted" },
    { approvalRef: "token" },
    { approverSpec: "owner" },
    { approverSpec: "ef".repeat(32) },
    { expiresAt: "bad" },
    { expiresAt: "2000-01-01T00:00:00Z" },
  ])
    assert.equal(check(run, { ...approval, ...patch }), false);
  for (const s of [
    { ...scope, relayUrl: "" },
    { ...scope, pubkey: "" },
  ])
    assert.equal(check(run, approval, s), false);
  assert.equal(canDecideApproval(run, approval, undefined, Date.now()), false);
});

test("card starts at review, does not submit on render, and fails closed without scope or deadline", () => {
  let calls = 0;
  const render = (extra = {}) =>
    renderToStaticMarkup(
      React.createElement(WorkflowApprovalCard, {
        approval,
        question: "Review scoped hiring",
        scope,
        onDecision: async () => {
          calls++;
        },
        ...extra,
      }),
    );
  assert.match(render(), /Review approval/);
  assert.match(render(), /Review denial/);
  assert.doesNotMatch(render(), /Confirm approve/);
  assert.equal(calls, 0);
  assert.doesNotMatch(render({ scope: undefined }), /Review approval/);
  assert.doesNotMatch(render({ question: null }), /Review approval/);
  assert.doesNotMatch(
    render({ approval: { ...approval, expiresAt: "bad" } }),
    /Review approval/,
  );
  assert.match(
    render({ approval: { ...approval, expiresAt: "bad" } }),
    /deadline passed or is unavailable/,
  );
});
