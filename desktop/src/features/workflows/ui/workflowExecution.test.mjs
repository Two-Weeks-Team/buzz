import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  parseExecutionObservation,
  executionPresentation,
} from "../../../shared/api/workflowExecution.ts";
import { WorkflowExecutionNotice } from "./WorkflowExecutionNotice.tsx";

const now = Date.parse("2026-09-07T08:00:00Z");
const raw = {
  state: "leased",
  epoch: 1,
  observed_at: new Date(now).toISOString(),
  lease_expires_at: new Date(now + 120000).toISOString(),
};
test("execution observation rejects missing/invalid state without inventing liveness", () => {
  for (const invalid of [
    null,
    undefined,
    [],
    {},
    { ...raw, state: "success" },
    { ...raw, epoch: -1 },
    { ...raw, epoch: 1.5 },
    { ...raw, observed_at: "bad" },
    { ...raw, lease_expires_at: null },
    { ...raw, lease_expires_at: new Date(now - 1).toISOString() },
  ])
    assert.equal(parseExecutionObservation(invalid), null);
  const parsed = parseExecutionObservation({
    ...raw,
    execution_token: "never-forward",
  });
  assert.equal(parsed.state, "leased");
  assert.equal(JSON.stringify(parsed).includes("never-forward"), false);
  assert.equal(
    executionPresentation({ status: "running", execution: parsed }, now).label,
    "Lease observed",
  );
  assert.equal(
    executionPresentation({ status: "running", execution: parsed }, now + 31000)
      .label,
    "Refresh execution state",
  );
  assert.equal(
    executionPresentation({ status: "running", execution: parsed }, now - 10000)
      .label,
    "Refresh execution state",
  );
  assert.equal(
    executionPresentation({ status: "running" }, now).label,
    "Execution state unavailable",
  );
  assert.equal(
    executionPresentation({ status: "completed" }, now).label,
    "completed",
  );
});
test("unknown and legacy execution notice never equate Running with proven progress", () => {
  let refreshes = 0;
  for (const [state, label] of [
    ["outcome_unknown", "Outcome unknown"],
    ["legacy_unowned", "Execution unverified"],
  ]) {
    const run = {
      status: "running",
      execution: parseExecutionObservation({
        ...raw,
        state,
        lease_expires_at: null,
      }),
    };
    const html = renderToStaticMarkup(
      React.createElement(WorkflowExecutionNotice, {
        run,
        onRefresh: () => refreshes++,
      }),
    );
    assert.ok(html.includes(label));
    assert.ok(html.includes("Refresh history"));
    assert.equal(html.includes("Retry execution"), false);
  }
  assert.equal(refreshes, 0);
});
