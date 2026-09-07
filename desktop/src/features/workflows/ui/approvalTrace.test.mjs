import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { WorkflowRunTrace } from "./WorkflowRunTrace.tsx";
import { approvalForTrace, approvalTraceStatus } from "./approvalTrace.ts";

const run = { id: "run-a", workflowId: "workflow-a" };
const step = {
  stepId: "gate",
  approvalRef: "ref-a",
  status: "waiting_approval",
};
const decision = {
  runId: run.id,
  workflowId: run.workflowId,
  stepId: step.stepId,
  approvalRef: step.approvalRef,
  status: "granted",
};

test("historical request uses only its exact persisted decision", () => {
  assert.equal(
    approvalTraceStatus(step, approvalForTrace(run, step, [decision])),
    "granted",
  );
  for (const field of ["runId", "workflowId", "stepId", "approvalRef"]) {
    assert.equal(
      approvalForTrace(run, step, [{ ...decision, [field]: "foreign" }]),
      undefined,
    );
  }
  assert.equal(
    approvalForTrace(run, { ...step, approvalRef: null }, [decision]),
    undefined,
  );
});

test("missing decision is unavailable, never current waiting or inferred completion", () => {
  assert.equal(approvalTraceStatus(step), "approval_requested");
  for (const status of ["pending", "denied", "expired", "granted"]) {
    assert.equal(approvalTraceStatus(step, { ...decision, status }), status);
  }
  assert.equal(
    approvalTraceStatus({ ...step, status: "completed" }),
    "completed",
  );
});

test("real trace component renders question and decision without stale pending badge", () => {
  const trace = {
    ...step,
    message: "Review <script>request</script>",
    output: {},
    startedAt: null,
    completedAt: null,
    error: null,
  };
  const render = (approvals) =>
    renderToStaticMarkup(
      React.createElement(WorkflowRunTrace, {
        run: { ...run, status: "completed", executionTrace: [trace] },
        approvals,
      }),
    );
  const html = render([
    { ...decision, note: "Approved scope only", approverPubkey: "owner" },
  ]);
  assert.match(html, /Review &lt;script&gt;request&lt;\/script&gt;/);
  assert.match(html, /Approved scope only/);
  assert.match(html, /workflow-approval-decision/);
  assert.doesNotMatch(
    html,
    /waiting approval|Pending approval|workflow-approval-card/,
  );
  assert.match(render([]), /Current decision is unavailable/);
  assert.doesNotMatch(
    render([{ ...decision, approvalRef: "foreign" }]),
    /workflow-approval-decision/,
  );
});
