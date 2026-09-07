import assert from "node:assert/strict";
import test from "node:test";
import { parseAttemptPage, getWorkflowAttempts } from "./workflowAttempts.ts";
const scope = {
  workflowId: "22222222-2222-4222-8222-222222222222",
  runId: "11111111-1111-4111-8111-111111111111",
  expectedRelayUrl: "ws://127.0.0.1:63203",
  expectedSignerPubkey: "ab".repeat(32),
};
const attempt = {
  step_index: 0,
  execution_epoch: 1,
  step_id: "pause",
  action_digest: "ab".repeat(32),
  started_at: "2026-09-07T09:00:00Z",
  returned_at: null,
  result: null,
};
const raw = {
  workflow_id: scope.workflowId,
  run_id: scope.runId,
  attempts: [attempt],
  next: null,
};
test("journal pages reject malformed, foreign, duplicate and unsafe cursors without empty fallback", () => {
  for (const invalid of [
    null,
    {},
    { ...raw, run_id: "foreign" },
    { ...raw, attempts: null },
    { ...raw, attempts: Array(17).fill(attempt) },
    { ...raw, next: undefined },
    { ...raw, next: { after_index: 2 } },
    { ...raw, attempts: [attempt, attempt] },
    ...[
      { step_index: -1 },
      { execution_epoch: 0 },
      { action_digest: "bad" },
      { started_at: "0" },
      { returned_at: "2026-09-07T09:00:00Z" },
      { result: { status: "completed" } },
    ].map((change) => ({ ...raw, attempts: [{ ...attempt, ...change }] })),
  ])
    assert.throws(() => parseAttemptPage(invalid, scope, null));
  assert.throws(() => parseAttemptPage(raw, scope, 0));
  const page = parseAttemptPage(
    {
      ...raw,
      attempts: [{ ...attempt, execution_token: "must-not-forward" }],
      next: { after_index: 0 },
    },
    scope,
    null,
  );
  assert.equal(page.attempts[0].result, null);
  assert.equal(page.next, 0);
  assert.equal(JSON.stringify(page).includes("must-not-forward"), false);
  assert.deepEqual(parseAttemptPage({ ...raw, attempts: [] }, scope, null), {
    attempts: [],
    next: null,
  });
});
test("journal transport preserves captured scope and performs only the requested read", async () => {
  const previous = globalThis.window;
  const calls = [];
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: async (command, args) => {
        calls.push({ command, args });
        return raw;
      },
    },
  };
  try {
    await getWorkflowAttempts(scope, null);
    assert.deepEqual(calls, [
      {
        command: "get_run_attempts",
        args: { request: { ...scope, afterIndex: null } },
      },
    ]);
  } finally {
    globalThis.window = previous;
  }
});
