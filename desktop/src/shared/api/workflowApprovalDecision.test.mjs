import assert from "node:assert/strict";
import test from "node:test";
import { denyApproval, grantApproval } from "./tauriWorkflows.ts";

const request = {
  approvalRef: "ab".repeat(32),
  workflowId: "22222222-2222-2222-2222-222222222222",
  runId: "11111111-1111-1111-1111-111111111111",
  expectedRelayUrl: "ws://127.0.0.1:63203",
  expectedSignerPubkey: "cd".repeat(32),
  note: "Only this gate",
};
const ack = {
  approval_ref: request.approvalRef,
  workflow_id: request.workflowId,
  run_id: request.runId,
  event_id: "ef".repeat(32),
  status: "granted",
};

test("approval wrappers preserve scope and consume exact durable decision acknowledgement", async () => {
  const previous = globalThis.window;
  const calls = [];
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: async (command, args) => {
        calls.push({ command, args });
        return {
          ...ack,
          status: command === "grant_approval" ? "granted" : "denied",
        };
      },
    },
  };
  try {
    for (const [call, command, status] of [
      [grantApproval, "grant_approval", "granted"],
      [denyApproval, "deny_approval", "denied"],
    ]) {
      assert.deepEqual(await call(request), {
        approvalRef: request.approvalRef,
        workflowId: request.workflowId,
        runId: request.runId,
        eventId: ack.event_id,
        status,
      });
      assert.deepEqual(calls.at(-1), { command, args: { request } });
    }
  } finally {
    globalThis.window = previous;
  }
});

test("event acceptance, missing fields and foreign decisions are not success and are never retried", async () => {
  const previous = globalThis.window;
  let calls = 0;
  let response;
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: async () => {
        calls++;
        return response;
      },
    },
  };
  try {
    const invalid = [
      null,
      {},
      { event_id: ack.event_id },
      ...["approval_ref", "workflow_id", "run_id", "event_id", "status"].map(
        (field) => ({ ...ack, [field]: "foreign" }),
      ),
      { ...ack, status: "denied" },
      { ...ack, status: "completed" },
    ];
    for (response of invalid) {
      await assert.rejects(
        grantApproval(request),
        /Refresh history before any retry/,
      );
    }
    assert.equal(calls, invalid.length);
  } finally {
    globalThis.window = previous;
  }
});
